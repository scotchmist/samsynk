use std::env;
use std::time::Duration;

use tokio_modbus::prelude::*;
use tokio_serial::{DataBits, SerialStream, StopBits};

static TTY_PATH: &str = "/dev/ttyUSB0";
static TIMEOUT: u64 = 10;
static SLAVE: u8 = 1;
static BAUD: u32 = 9600;
static DATA_BITS: DataBits = DataBits::Eight;
static STOP_BITS: StopBits = StopBits::One;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let slave = Slave(SLAVE);
    let builder = tokio_serial::new(TTY_PATH, BAUD)
        .stop_bits(STOP_BITS)
        .data_bits(DATA_BITS)
        .timeout(Duration::new(TIMEOUT, 0));
    let port = SerialStream::open(&builder)
        .unwrap_or_else(|_| panic!("Could not open port {}.", TTY_PATH));

    let mut ctx = rtu::attach_slave(port, slave);

    for arg in env::args().skip(1) {
        let int_arg = arg.parse::<u16>().unwrap();
        let rsp = ctx.read_holding_registers(int_arg, 1).await.unwrap();
        println!("Sensor value is: {rsp:?}");
    }

    Ok(())
}
