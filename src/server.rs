use crate::sensor::{SensorTypes, SensorWrite, REGISTRY};
use bytes::Bytes;
use prometheus::Encoder;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::AtomicU16;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration, Instant};
use tokio_modbus::client::Context;
use warp::{Filter, Rejection, Reply};

const START_TIMEOUT: Duration = Duration::from_secs(5);
const COLLECT_INTERVAL: Duration = Duration::from_secs(10);

type Address = ([u8; 4], u16);

async fn data_collector(
    all_sensors: HashMap<String, SensorTypes<'static>>,
    ctx: Arc<Mutex<Context>>,
) {
    let mut collect_interval = interval(COLLECT_INTERVAL);
    loop {
        collect_interval.tick().await;
        let ctx = ctx.clone();

        for (_, sensor) in all_sensors.clone().iter() {
            sensor.read(ctx.clone()).await.unwrap();
        }
    }
}

pub fn origin_url(addr: ([u8; 4], u16)) -> String {
    let host = addr.0.map(|i| i.to_string()).join(".");
    format!("http://{}:{}", host, addr.1)
}

async fn metrics_handler() -> Result<impl Reply, Rejection> {
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {}", e);
    };
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);
    Ok(res)
}

async fn healthcheck_handler() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::html("Everything is OK!"))
}

pub async fn sensor_get_handler(
    sensor_name: String,
    ctx: Arc<Mutex<Context>>,
    sensors: HashMap<String, SensorTypes<'_>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(sensor) = sensors.get(&sensor_name) {
        let result = sensor.read(ctx).await;
        match result {
            Ok(res) => Ok(warp::reply::with_status(res, warp::http::StatusCode::OK)),
            Err(_) => Ok(warp::reply::with_status(
                "INTERNAL_SERVER_ERROR".to_string(),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            )),
        }
    } else {
        Ok(warp::reply::with_status(
            "NOT FOUND".to_string(),
            warp::http::StatusCode::NOT_FOUND,
        ))
    }
}

pub async fn sensor_post_handler(
    sensor_name: String,
    val: Bytes,
    ctx: Arc<Mutex<Context>>,
    sensors: HashMap<String, SensorTypes<'_>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let Some(sensor) = sensors.get(&sensor_name) else {
        return Err(warp::reject());
    };
    if let SensorTypes::Basic(s) = sensor {
        s.write(
            ctx.clone(),
            AtomicU16::new(std::str::from_utf8(&val).unwrap().parse::<u16>().unwrap()),
        )
        .await
        .expect("Error with writing to modbus.");
    }
    Ok(warp::reply::reply())
}

pub struct Server {
    pub(crate) _join_handle: tokio::task::JoinHandle<()>,
}

pub async fn wait_for_healthcheck(address: Address) {
    let deadline = Instant::now() + START_TIMEOUT;

    while Instant::now() <= deadline {
        if let Ok(res) = reqwest::get(origin_url(address) + "/api/healthcheck").await {
            if res.status() == StatusCode::OK {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("Server did not become available.");
}

impl Server {
    pub async fn new(
        ctx: Arc<Mutex<Context>>,
        address: Address,
        sensors: HashMap<String, SensorTypes<'static>>,
    ) -> Result<Server, Box<dyn Error>> {
        tokio::task::spawn(data_collector(sensors.clone(), ctx.clone()));

        let sensors_filter = warp::any().map(move || sensors.clone());
        let modbus_client_ctx_filter = warp::any().map(move || ctx.clone());

        let unstable_api_read = warp::path!("api" / "unstable" / String)
            .and(warp::get())
            .and(modbus_client_ctx_filter.clone())
            .and(sensors_filter.clone())
            .and_then(sensor_get_handler);

        let unstable_api_write = warp::path!("api" / "unstable" / String)
            .and(warp::post())
            .and(warp::body::bytes())
            .and(modbus_client_ctx_filter.clone())
            .and(sensors_filter.clone())
            .and_then(sensor_post_handler);

        let healthcheck_api_route = warp::path!("api" / "healthcheck")
            .and(warp::get())
            .and_then(healthcheck_handler);

        let metrics = warp::path!("metrics").and_then(metrics_handler);

        let routes = healthcheck_api_route
            .or(unstable_api_read)
            .or(unstable_api_write)
            .or(metrics);

        let server = Server {
            _join_handle: tokio::spawn(async move { warp::serve(routes).run(address).await }),
        };
        wait_for_healthcheck(address).await;

        Ok(server)
    }
}
