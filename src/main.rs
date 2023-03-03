mod sensor;

use sensor::{SerialSensor, RatedPowerSensor, Sensor};

use core::time::Duration;
use tokio_modbus::prelude::*;
use tokio_serial::{DataBits, SerialStream, StopBits};


//async fn read_serial_number(context: ctx)

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tty_path = "/dev/ttyUSB0";
    let timeout = 10;
    let slave = Slave(1);

    let builder = tokio_serial::new(tty_path, 9600)
        .stop_bits(StopBits::One)
        .data_bits(DataBits::Eight)
        .timeout(Duration::new(timeout, 0));
    let port = SerialStream::open(&builder).expect(&format!("Could not open port {}.", tty_path));

    let mut ctx = rtu::connect_slave(port, slave).await?;

    let serial_regs = vec![3, 4, 5, 6, 7];
    let rated_power_regs = vec![16, 17];
    let mut serial_sensor = SerialSensor::new("Serial".to_string(), serial_regs);
    let mut rated_power = RatedPowerSensor::new("Rated Power".to_string(), rated_power_regs);
    println!("Reading a sensor value");

    let (ctx, value) = serial_sensor.read(ctx).await?;
    let (ctx, _value) = rated_power.read(ctx).await?;
    println!("Sensor value is: {}", value);

    Ok(())
}
