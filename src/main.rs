pub mod helpers;
pub mod sensor;
pub mod sensor_definitions;
pub mod server;

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tokio_modbus::prelude::*;
use tokio_serial::{DataBits, SerialStream, StopBits};

const IP_ADDR: [u8; 4] = [127, 0, 0, 1];
pub const TTY_PATH: &str = "/dev/ttyUSB0";
const PORT: u16 = 8080;
const BAUD_RATE: u32 = 9600;

const SLAVE: Slave = Slave(1);
const TIMEOUT: Duration = Duration::from_secs(2);
const DATA_BITS: DataBits = DataBits::Eight;
const STOP_BITS: StopBits = StopBits::One;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let builder = tokio_serial::new(TTY_PATH, BAUD_RATE)
        .stop_bits(STOP_BITS)
        .data_bits(DATA_BITS)
        .timeout(TIMEOUT);
    let client_serial = SerialStream::open(&builder)
        .unwrap_or_else(|_| panic!("Could not open port {}.", TTY_PATH));

    let addr = (IP_ADDR, PORT);

    let ctx = Arc::new(Mutex::new(rtu::attach_slave(client_serial, SLAVE)));

    let server = server::Server::new(ctx.clone(), addr).await.unwrap();
    server._join_handle.await.unwrap();
}
