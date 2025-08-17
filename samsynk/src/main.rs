use log::info;
use samsynk_lib::sensor::{SensorTypes, register_sensors};
use std::collections::HashMap;
use syslog::{BasicLogger, Facility, Formatter3164};
use tokio::time::Duration;
use tokio_modbus::prelude::*;
use tokio_serial::{DataBits, SerialStream, StopBits};

const IP_ADDR: [u8; 4] = [127, 0, 0, 1];
pub const TTY_PATH: &str = "/dev/ttyUSB0";
const PORT: u16 = 8082;
const BAUD_RATE: u32 = 9600;

const SLAVE: Slave = Slave(1);
const TIMEOUT: Duration = Duration::from_secs(2);
const DATA_BITS: DataBits = DataBits::Eight;
const STOP_BITS: StopBits = StopBits::One;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let syslog_formatter = Formatter3164 {
        facility: Facility::LOG_USER,
        hostname: None,
        process: env!("CARGO_PKG_NAME").into(),
        pid: 0,
    };

    let syslog_writer = syslog::unix(syslog_formatter).expect("Could not connect to syslog.");

    log::set_boxed_logger(Box::new(BasicLogger::new(syslog_writer))).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    info!("Starting samsynk.");

    let sensors: HashMap<String, SensorTypes> = register_sensors();

    let builder = tokio_serial::new(TTY_PATH, BAUD_RATE)
        .stop_bits(STOP_BITS)
        .data_bits(DATA_BITS)
        .timeout(TIMEOUT);
    let client_serial = SerialStream::open(&builder)
        .unwrap_or_else(|_| panic!("Could not open port {}.", TTY_PATH));

    let addr = (IP_ADDR, PORT);

    let ctx = rtu::attach_slave(client_serial, SLAVE);

    let server = samsynk_lib::server::Server::new(ctx, addr, sensors)
        .await
        .unwrap();
    server._join_handle.await.unwrap();
}
