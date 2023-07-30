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
        self.0.metric.set(output);

        Ok((ctx, format!("{}", output)))
    }
}

#[derive(Clone)]
pub struct CompoundSensor<'a> {
    pub name: &'a str,
    registers: &'a [u16],
    factors: &'a [i64],
    no_negative: bool,
    absolute: bool,
    metric: IntGauge,
}

impl CompoundSensor<'_> {
    pub fn new<'a>(
        name: &'a str,
        registers: &'a [u16],
        factors: &'a [i64],
        no_negative: bool,
        absolute: bool,
    ) -> CompoundSensor<'a> {
        let metric = IntGauge::new(slug_name(name), name).unwrap();
        REGISTRY.register(Box::new(metric.clone())).unwrap();

        CompoundSensor {
            name,
            registers,
            factors,
            no_negative,
            absolute,
            metric,
        }
    }
}

#[async_trait]
impl SensorRead for CompoundSensor<'_> {
    async fn read(
        &self,
        mut ctx: Box<dyn Reader>,
    ) -> Result<(Box<dyn Reader>, String), Box<dyn std::error::Error>> {
        let mut output: i64 = 0;
        for (i, reg) in self.registers.iter().enumerate() {
            let raw_output = ctx.read_holding_registers(*reg, 1u16).await?;
            let signed = match self.factors[i] < 0 {
                true => signed(raw_output[0] as i64),
                false => raw_output[0] as i64,
            };
            output += signed * self.factors[i];
        }
        if self.absolute && output < 0 {
            output = -output
        }
        if self.no_negative && output < 0 {
            output = 0;
        }

        self.metric.set(output);

        Ok((ctx, format!("{}", output)))
    }
}

#[derive(Clone)]
pub struct FaultSensor {
    pub(crate) registers: [u16; 4],
}

#[async_trait]
impl SensorRead for FaultSensor {
    async fn read(
        &self,
        mut ctx: Box<dyn Reader>,
    ) -> Result<(Box<dyn Reader>, String), Box<dyn std::error::Error>> {
        let raw_output = ctx
            .read_holding_registers(self.registers[0], self.registers.len() as u16)
            .await?;

        Ok((ctx, faults_decode(raw_output).join(", ")))
    }
}

fn faults_decode(reg_vals: Vec<u16>) -> Vec<String> {
    let mut faults: Vec<String> = Vec::new();
    let mut off = 0;
    for val in reg_vals.iter() {
        for bit in 0..16 {
            let mask = 1 << bit;
            if mask & val != 0 {
                let fault = match off + mask {
                    13 => " Working mode change",
                    18 => " AC over current",
                    20 => " DC over current",
                    23 => " F23 AC leak current or transient over current",
                    24 => " F24 DC insulation impedance",
                    26 => " F26 DC busbar imbalanced",
                    29 => " Parallel comms cable",
                    35 => " No AC grid",
                    42 => " AC line low voltage",
                    47 => " AC freq high/low",
                    56 => " DC busbar voltage low",
                    63 => " ARC fault",
                    64 => " Heat sink tempfailure",
                    _ => "",
                };
                faults.push(format!("F{}{}", (bit + off + 1), fault));
            }
        }
        off += 16;
    }
    faults
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
    Fault(FaultSensor),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_modbus::prelude::Response::ReadHoldingRegisters;

    use std::{
        fmt::Debug,
        io::{Error, ErrorKind},
        sync::Mutex,
    };

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
        responses: Vec<Result<Response, Error>>,
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
            self.responses.push(next_response)
        }
    }

    #[async_trait]
    impl Client for ClientMock {
        async fn call<'a>(&'a mut self, request: Request) -> Result<Response, Error> {
            *self.last_request.lock().unwrap() = Some(request);
            self.responses.pop().unwrap()
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
    async fn test_faults_decode() {
        assert_eq!(
            vec!["F1".to_string()],
            faults_decode(vec![0x01, 0x0, 0x0, 0x0])
        );

        assert_eq!(
            vec!["F8".to_string()],
            faults_decode(vec![0x80, 0x0, 0x0, 0x0])
        );

        assert_eq!(
            vec!["F32".to_string()],
            faults_decode(vec![0x0, 0x8000, 0x0, 0x0])
        );

        assert_eq!(
            vec!["F1".to_string(), "F8".to_string(), "F32".to_string()],
            faults_decode(vec![0x81, 0x8000, 0x0, 0x0])
        );

        assert_eq!(
            vec!["F33".to_string()],
            faults_decode(vec![0x0, 0x0, 0x1, 0x0])
        );
    }

    #[tokio::test]
    async fn faults_sensor_read() {
        let mock_out: Vec<u16> = vec![0x81, 0x8000, 0x0, 0x0];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Box::new(Context { client });

        let fault_sensor = FaultSensor {
            registers: [103, 104, 105, 106],
        };

        let value: String;
        (_, value) = fault_sensor.read(ctx).await.unwrap();

        assert_eq!("F1, F8, F32", value);
    }

    #[tokio::test]
    async fn compound_sensor_read() {
        let mock_out: Vec<u16> = vec![1000, 800];
        let mut client = Box::<ClientMock>::default();
        // Loop in reverse order, to stack the responses.
        for mock_val in mock_out.iter().rev().collect::<Vec<_>>() {
            client.set_next_response(Ok(ReadHoldingRegisters(vec![*mock_val])));
        }
        let ctx = Box::new(Context { client });

        let compound_sensor =
            CompoundSensor::new("Grid Current", &[160, 161], &[1, -1], false, false);

        let value: String;
        (_, value) = compound_sensor.read(ctx).await.unwrap();

        assert_eq!("200", value);
    }

    #[tokio::test]
    async fn compound_sensor_read2() {
        let mock_out: Vec<u16> = vec![200, 800];
        let mut client = Box::<ClientMock>::default();
        // Loop in reverse order, to stack the responses.
        for mock_val in mock_out.iter().rev().collect::<Vec<_>>() {
            client.set_next_response(Ok(ReadHoldingRegisters(vec![*mock_val])));
        }
        let ctx = Box::new(Context { client });

        let compound_sensor =
            CompoundSensor::new("Fake Sensor", &[160, 161], &[1, -1], false, false);

        let value: String;
        (_, value) = compound_sensor.read(ctx).await.unwrap();

        assert_eq!("-600", value);
    }

    #[tokio::test]
    async fn compound_sensor_read_no_negative() {
        let mock_out: Vec<u16> = vec![200, 800];
        let mut client = Box::<ClientMock>::default();
        // Loop in reverse order, to stack the responses.
        for mock_val in mock_out.iter().rev().collect::<Vec<_>>() {
            client.set_next_response(Ok(ReadHoldingRegisters(vec![*mock_val])));
        }
        let ctx = Box::new(Context { client });

        let compound_sensor = CompoundSensor::new(
            "Compound Sensor No Negative",
            &[160, 161],
            &[1, -1],
            true,
            false,
        );

        let value: String;
        (_, value) = compound_sensor.read(ctx).await.unwrap();

        assert_eq!("0", value);
    }

    #[tokio::test]
    async fn compound_sensor_absolute() {
        let mock_out: Vec<u16> = vec![200, 800];
        let mut client = Box::<ClientMock>::default();
        // Loop in reverse order, to stack the responses.
        for mock_val in mock_out.iter().rev().collect::<Vec<_>>() {
            client.set_next_response(Ok(ReadHoldingRegisters(vec![*mock_val])));
        }
        let ctx = Box::new(Context { client });

        let compound_sensor = CompoundSensor::new(
            "Compound Sensor Absolute",
            &[160, 161],
            &[1, -1],
            false,
            true,
        );

        let value: String;
        (_, value) = compound_sensor.read(ctx).await.unwrap();

        assert_eq!("600", value);
    }
}
