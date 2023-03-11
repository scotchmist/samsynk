use crate::helpers::{signed, slug_name};

use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::{IntGauge, Registry};
use tokio_modbus::client::Context;
use tokio_modbus::prelude::*;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
}

#[async_trait]
pub trait SensorRead {
    async fn read(&self, ctx: Context) -> Result<(Context, String), Box<dyn std::error::Error>>;
}

#[derive(Clone)]
pub struct Sensor<'a> {
    pub name: &'a str,
    registers: &'a [u16],
    factor: u32,
    is_signed: bool,
    metric: IntGauge,
}

impl Sensor<'_> {
    pub fn new<'a>(
        name: &'a str,
        registers: &'a [u16],
        factor: u32,
        is_signed: bool,
    ) -> Sensor<'a> {
        let metric = IntGauge::new(&slug_name(name), name).unwrap();
        REGISTRY.register(Box::new(metric.clone())).unwrap();

        Sensor {
            name,
            registers,
            factor,
            is_signed,
            metric,
        }
    }
}

#[async_trait]
impl SensorRead for Sensor<'_> {
    async fn read(
        &self,
        mut ctx: Context,
    ) -> Result<(Context, String), Box<dyn std::error::Error>> {
        let raw_output = ctx
            .read_holding_registers(self.registers[0], self.registers.len() as u16)
            .await?;
        let mut output = raw_output[0] as i64;

        if raw_output.len() > 1 {
            output += (raw_output[1] as i64) << 16
        } else if self.is_signed {
            output = signed(output)
        }
        output /= i64::pow(self.factor as i64, 1);

        self.metric.set(output.into());

        Ok((ctx, format!("{}", output)))
    }
}

#[derive(Clone)]
pub struct TemperatureSensor<'a>(pub Sensor<'a>);

#[async_trait]
impl SensorRead for TemperatureSensor<'_> {
    async fn read(
        &self,
        mut ctx: Context,
    ) -> Result<(Context, String), Box<dyn std::error::Error>> {
        let raw_output = ctx
            .read_holding_registers(self.0.registers[0], self.0.registers.len() as u16)
            .await?;
        let mut output = raw_output[0] as i64;

        if raw_output.len() > 1 {
            output += (raw_output[1] as i64) << 16
        } else if self.0.is_signed {
            output = signed(output)
        }
        output /= i64::pow(self.0.factor as i64, 1);

        self.0.metric.set(output - 100_i64);

        Ok((ctx, format!("{}", output)))
    }
}

#[derive(Clone)]
pub struct SerialSensor<'a> {
    pub name: &'a str,
    registers: [u16; 5],
}

#[async_trait]
impl SensorRead for SerialSensor<'_> {
    async fn read(
        &self,
        mut ctx: Context,
    ) -> Result<(Context, String), Box<dyn std::error::Error>> {
        let raw_value = ctx
            .read_holding_registers(self.registers[0], self.registers.len() as u16)
            .await?;
        let mut output = "".to_owned();
        for b16 in raw_value {
            let first_char = &((b16 >> 8) as u8 as char).to_string();
            let second_char = &((b16 & 0xFF) as u8 as char).to_string();
            output.push_str(first_char);
            output.push_str(second_char);
        }

        Ok((ctx, output))
    }
}

pub static SERIAL: SerialSensor = SerialSensor {
    name: "Serial Number",
    registers: [3, 4, 5, 6, 7],
};

//pub static RATED_POWER: Sensor = BaseSensor {
//    name: "RatedPower",
//    registers: &[16, 17],
//   factor: 10,
//};

pub enum SensorTypes<'a> {
    Basic(Sensor<'a>),
    Temperature(TemperatureSensor<'a>),
}
