use samsynk::sensor::{SensorRead, SensorTypes, REGISTRY};

use samsynk::sensor_definitions::*;

use core::time::Duration;
use tokio::time::interval;
use tokio_modbus::client::Reader;
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
static DATA_BITS: DataBits = DataBits::Eight;
static STOP_BITS: StopBits = StopBits::One;

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
        .stop_bits(STOP_BITS)
        .data_bits(DATA_BITS)
        .timeout(Duration::new(TIMEOUT, 0));
    let port = SerialStream::open(&builder)
        .unwrap_or_else(|_| panic!("Could not open port {}.", TTY_PATH));

    let mut ctx: Box<dyn Reader> = Box::new(rtu::connect_slave(port, slave).await.unwrap());

    let mut all_sensors: Vec<SensorTypes<'static>> = vec![];

    for sensor in SENSORS.clone().into_iter() {
        all_sensors.push(SensorTypes::Basic(sensor.clone()));
    }

    for sensor in TEMP_SENSORS.clone().into_iter() {
        all_sensors.push(SensorTypes::Temperature(sensor.clone()));
    }

    for sensor in COMPOUND_SENSORS.clone().into_iter() {
        all_sensors.push(SensorTypes::Compound(sensor.clone()));
    }

    let mut collect_interval = interval(Duration::from_millis(5000));
    loop {
        collect_interval.tick().await;

        for sensor in all_sensors.clone().into_iter() {
            (ctx, _) = match sensor {
                SensorTypes::Basic(s) => s.read(ctx).await.unwrap(),
                SensorTypes::Temperature(s) => s.read(ctx).await.unwrap(),
                SensorTypes::Compound(s) => s.read(ctx).await.unwrap(),
                SensorTypes::Serial(_) => (ctx, String::new()),
                SensorTypes::Fault(_) => (ctx, String::new()),
            }
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
