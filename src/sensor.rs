use crate::helpers::{signed, slug_name};

use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::{IntGauge, Registry};
pub use tokio_modbus::client::Context;
use tokio_modbus::prelude::*;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
}

#[async_trait]
pub trait SensorRead {
    async fn read(
        &self,
        ctx: Box<dyn Reader>,
    ) -> Result<(Box<dyn Reader>, String), Box<dyn std::error::Error>>;
}

#[derive(Clone)]
pub struct Sensor<'a> {
    pub name: &'a str,
    registers: &'a [u16],
    factor: i64,
    is_signed: bool,
    metric: IntGauge,
}

impl Sensor<'_> {
    pub fn new<'a>(
        name: &'a str,
        registers: &'a [u16],
        factor: i64,
        is_signed: bool,
    ) -> Sensor<'a> {
        let metric = IntGauge::new(slug_name(name), name).unwrap();
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
        mut ctx: Box<dyn Reader>,
    ) -> Result<(Box<dyn Reader>, String), Box<dyn std::error::Error>> {
        let raw_output = ctx
            .read_holding_registers(self.registers[0], self.registers.len() as u16)
            .await?;
        let mut output = raw_output[0] as i64;

        if raw_output.len() > 1 {
            output += (raw_output[1] as i64) << 16
        } else if self.is_signed {
            output = signed(output)
        }
        output /= self.factor;

        self.metric.set(output);

        Ok((ctx, format!("{}", output)))
    }
}

#[derive(Clone)]
pub struct TemperatureSensor<'a>(pub Sensor<'a>);

#[async_trait]
impl SensorRead for TemperatureSensor<'_> {
    async fn read(
        &self,
        mut ctx: Box<dyn Reader>,
    ) -> Result<(Box<dyn Reader>, String), Box<dyn std::error::Error>> {
        let raw_output = ctx
            .read_holding_registers(self.0.registers[0], self.0.registers.len() as u16)
            .await?;

        let mut output = raw_output[0] as i64;

        output /= self.0.factor;
        output -= 100_i64;
        self.0.metric.set(output as i64);

        Ok((ctx, format!("{}", output)))
    }
}

#[derive(Clone)]
pub struct CumulativeSensor<'a>{
    pub name: &'a str,
    registers: &'a [u16],
    factors: &'a [i64],
    is_signed: &'a [bool],
    no_negative: bool,

    metric: IntGauge,
}

impl CumulativeSensor<'_> {
    pub fn new<'a>(
        name: &'a str,
        registers: &'a [u16],
        factors: &'a [i64],
        is_signed: &'a [bool],
        no_negative: bool
    ) -> CumulativeSensor<'a> {
        let metric = IntGauge::new(slug_name(name), name).unwrap();
        REGISTRY.register(Box::new(metric.clone())).unwrap();

        CumulativeSensor {
            name,
            registers,
            factors,
            is_signed,
            no_negative,
            metric,
        }
    }
}

#[async_trait]
impl SensorRead for CumulativeSensor<'_> {
    async fn read(
        &self,
        mut ctx: Box<dyn Reader>,
    ) -> Result<(Box<dyn Reader>, String), Box<dyn std::error::Error>> {
        let mut output: i64 = 0;
        for (i, reg) in self.registers.iter().enumerate() {
            let raw_output = ctx
                .read_holding_registers(*reg, 1)
                .await?;
            let signed = match self.is_signed[i] {
                true => signed(raw_output[0] as i64),
                false => raw_output[0] as i64
            };
            output += signed as i64 * self.factors[i];
        }
        if self.no_negative && output < 0 {
            output = 0;
        }

        Ok((ctx, format!("{}", output)))
    }
}

#[derive(Clone)]
pub struct SerialSensor<'a> {
    pub name: &'a str,
    pub(crate) registers: [u16; 5],
}

#[async_trait]
impl SensorRead for SerialSensor<'_> {
    async fn read(
        &self,
        mut ctx: Box<dyn Reader>,
    ) -> Result<(Box<dyn Reader>, String), Box<dyn std::error::Error>> {
        let raw_value = ctx
            .read_holding_registers(self.registers[0], self.registers.len() as u16)
            .await?;
        let mut output = "".to_owned();
        for b16 in raw_value {
            let first_char = format!("{}", (b16 >> 8) as u8);
            let second_char = format!("{}", (b16 & 0xFF) as u8);
            output.push_str(&first_char);
            output.push_str(&second_char);
        }

        Ok((ctx, output))
    }
}

#[derive(Clone)]
pub enum SensorTypes<'a> {
    Basic(Sensor<'a>),
    Temperature(TemperatureSensor<'a>),
    Serial(SerialSensor<'a>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_modbus::prelude::Response::ReadHoldingRegisters;

    use std::{
        fmt::Debug,
        io::{Error, ErrorKind},
    };

    use std::sync::Mutex;

    #[derive(Debug)]
    struct Context {
        client: Box<dyn Client>,
    }

    #[async_trait]
    impl Client for Context {
        async fn call<'a>(&'a mut self, request: Request) -> Result<Response, Error> {
            self.client.call(request).await
        }
    }

    #[derive(Default, Debug)]
    pub(crate) struct ClientMock {
        slave: Option<Slave>,
        last_request: Mutex<Option<Request>>,
        next_response: Option<Result<Response, Error>>,
    }

    #[allow(dead_code)]
    impl ClientMock {
        pub(crate) fn slave(&self) -> Option<Slave> {
            self.slave
        }

        pub(crate) fn last_request(&self) -> &Mutex<Option<Request>> {
            &self.last_request
        }

        pub(crate) fn set_next_response(&mut self, next_response: Result<Response, Error>) {
            self.next_response = Some(next_response);
        }
    }

    #[async_trait]
    impl Client for ClientMock {
        async fn call<'a>(&'a mut self, request: Request) -> Result<Response, Error> {
            *self.last_request.lock().unwrap() = Some(request);
            match self.next_response.as_ref().unwrap() {
                Ok(response) => Ok(response.clone()),
                Err(err) => Err(Error::new(err.kind(), format!("{err}"))),
            }
        }
    }

    impl SlaveContext for ClientMock {
        fn set_slave(&mut self, slave: Slave) {
            self.slave = Some(slave);
        }
    }

    impl SlaveContext for Context {
        fn set_slave(&mut self, slave: Slave) {
            self.client.set_slave(slave);
        }
    }

    #[async_trait]
    impl Reader for Context {
        async fn read_holding_registers<'a>(
            &'a mut self,
            addr: u16,
            cnt: u16,
        ) -> Result<Vec<u16>, Error> {
            let rsp = self
                .client
                .call(Request::ReadHoldingRegisters(addr, cnt))
                .await?;
            if let Response::ReadHoldingRegisters(rsp) = rsp {
                if rsp.len() as u16 != cnt {
                    return Err(Error::new(ErrorKind::InvalidData, "invalid response"));
                }
                Ok(rsp)
            } else {
                Err(Error::new(ErrorKind::InvalidData, "unexpected response"))
            }
        }

        async fn read_discrete_inputs(&mut self, _: u16, _: u16) -> Result<Vec<bool>, Error> {
            Ok(vec![true])
        }

        async fn read_coils(&mut self, _: u16, _: u16) -> Result<Vec<bool>, Error> {
            Ok(vec![true])
        }

        async fn read_input_registers(&mut self, _: u16, _: u16) -> Result<Vec<u16>, Error> {
            Ok(vec![2])
        }

        async fn read_write_multiple_registers(
            &mut self,
            _: u16,
            _: u16,
            _: u16,
            _: &[u16],
        ) -> Result<Vec<u16>, Error> {
            Ok(vec![1])
        }
    }

    #[tokio::test]
    async fn read_data_from_modbus_over_serial() {
        let mock_out = vec![240];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Box::new(Context { client });

        let sensor = Sensor::new("Battery Voltage", &[183], 1, false);

        let value: String;
        (_, value) = sensor.read(ctx).await.unwrap();

        assert_eq!("240", value);
    }

    /// Check that the Temperature read method works as expected.
    #[tokio::test]
    async fn temp_sensor_read() {
        let mock_out = vec![1110];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Box::new(Context { client });

        let sensor = TemperatureSensor(Sensor::new("Battery Temperature", &[182], 10, false));

        let value: String;
        (_, value) = sensor.read(ctx).await.unwrap();

        assert_eq!("11", value);
    }

    /// Check that the Serial Number read method works as expected.
    #[tokio::test]
    async fn serial_sensor_read() {
        let mock_out = vec![513, 513, 513, 513, 513];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Box::new(Context { client });

        let serial = SerialSensor {
            name: "Serial Number",
            registers: [3, 4, 5, 6, 7],
        };

        let value: String;
        (_, value) = serial.read(ctx).await.unwrap();

        assert_eq!("2121212121", value);
    }

    #[tokio::test]
    async fn temp_sensor_read() {
        let mock_out = vec![1110, 123, 567, 891];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Box::new(Context { client });

        let sensor = TemperatureSensor(Sensor::new("Battery Temperature", &[182], 10, false));

        let value: String;
        (_, value) = sensor.read(ctx).await.unwrap();

        assert_eq!("11", value);
    }

    // TODO: Add signed test for each type
}
