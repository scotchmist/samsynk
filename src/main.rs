use samsynk::sensor::{SensorRead, SensorTypes, REGISTRY};
use samsynk::sensor_definitions::*;
use samsynk::server;

use core::time::Duration;
use tokio::time::interval;
use tokio_modbus::client::{Context, Reader};
use tokio_modbus::prelude::*;
use tokio_serial::{DataBits, SerialStream, StopBits};

use warp::{Rejection, Reply};

use prometheus::Encoder;

const IP_ADDR: [u8; 4] = [127, 0, 0, 1];
const PORT: u16 = 8080;

const TTY_PATH: &str = "/dev/ttyUSB0";
const TIMEOUT: u64 = 10;
const SLAVE: u8 = 1;
const BAUD: u32 = 9600;
const DATA_BITS: DataBits = DataBits::Eight;
const STOP_BITS: StopBits = StopBits::One;
const COLLECT_INTERVAL: u64 = 5000; //ms

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

async fn data_collector(all_sensors: Vec<SensorTypes<'static>>, mut ctx: Box<dyn Reader>) {
    let mut collect_interval = interval(Duration::from_millis(COLLECT_INTERVAL));
    loop {
        collect_interval.tick().await;

        for sensor in all_sensors.clone().into_iter() {
            (ctx, _) = match sensor {
                SensorTypes::Basic(s) => s.read(ctx).await.unwrap(),
                SensorTypes::Temperature(s) => s.read(ctx).await.unwrap(),
                SensorTypes::Compound(s) => s.read(ctx).await.unwrap(),
                SensorTypes::Fault(s) => s.read(ctx).await.unwrap(),
                SensorTypes::Serial(_) => (ctx, String::new()),
            }
        }
    }
}

fn register_sensors() -> Vec<SensorTypes<'static>> {
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
    all_sensors.push(SensorTypes::Fault(FAULTS.clone()));
    all_sensors
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let slave = Slave(SLAVE);

    let builder = tokio_serial::new(TTY_PATH, BAUD)
        .stop_bits(STOP_BITS)
        .data_bits(DATA_BITS)
        .timeout(Duration::new(TIMEOUT, 0));
    let port = SerialStream::open(&builder)
        .unwrap_or_else(|_| panic!("Could not open port {}.", TTY_PATH));

    let ctx: Box<Context> = Box::new(rtu::attach_slave(port, slave));

    let all_sensors = register_sensors();

    tokio::task::spawn(data_collector(all_sensors, ctx));
    let server = server::Server::start((IP_ADDR, PORT));
    server.await;
}
