mod helpers;
mod sensor;
mod sensor_definitions;

use sensor::{SensorRead, SensorTypes, REGISTRY};

use sensor_definitions::*;

use core::time::Duration;
use tokio::time::interval;
use tokio_modbus::prelude::*;
use tokio_serial::{DataBits, SerialStream, StopBits};

use warp::{Filter, Rejection, Reply};

use prometheus::Encoder;

static IP_ADDR: [u8; 4] = [127, 0, 0, 1];
static PORT: u16 = 8080;

static TTY_PATH: &str = "/dev/ttyUSB0";
static TIMEOUT: u64 = 10;
static SLAVE: u8 = 1;
static BAUD: u32 = 9600;

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

async fn data_collector() {
    let slave = Slave(SLAVE);

    let builder = tokio_serial::new(TTY_PATH, BAUD)
        .stop_bits(StopBits::One)
        .data_bits(DataBits::Eight)
        .timeout(Duration::new(TIMEOUT, 0));
    let port = SerialStream::open(&builder).expect(&format!("Could not open port {}.", TTY_PATH));

    let mut ctx = rtu::connect_slave(port, slave).await.unwrap();

    let mut all_sensors: Vec<SensorTypes<'static>> = vec![];

    for sensor in SENSORS.clone().into_iter() {
        all_sensors.push(SensorTypes::Basic(sensor.clone()));
    }

    for sensor in TEMP_SENSORS.clone().into_iter() {
        all_sensors.push(SensorTypes::Temperature(sensor.clone()));
    }

    let mut collect_interval = interval(Duration::from_millis(5000));
    loop {
        collect_interval.tick().await;

        for sensor in SENSORS.clone().into_iter() {
            (ctx, _) = sensor.read(ctx).await.unwrap();
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metrics_route = warp::path!("metrics").and_then(metrics_handler);

    tokio::task::spawn(data_collector());
    warp::serve(metrics_route).run((IP_ADDR, PORT)).await;
    Ok(())
}
