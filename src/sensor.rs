use crate::helpers::{signed, slug_name};

use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::{IntGauge, IntGaugeVec, Opts, Registry};
//use std::fmt::Error;
use std::io::{Error, ErrorKind};
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

#[async_trait]
pub trait SensorWrite {
    async fn write(
        &self,
        ctx: Box<dyn Writer>,
        value: u16,
    ) -> Result<Box<dyn Writer>, Box<dyn std::error::Error>>;
}

#[derive(Clone)]
pub struct Sensor<'a> {
    pub name: &'a str,
    registers: &'a [u16],
    factor: i64,
    is_signed: bool,
    is_mut: bool,
    max: Option<u16>,
    min: Option<u16>,
    metric: IntGauge,
}

impl<'a> Default for Sensor<'a> {
    fn default() -> Sensor<'a> {
        let name = "";
        let metric = IntGauge::new(slug_name(name), name).unwrap();
        REGISTRY.register(Box::new(metric.clone())).unwrap();

        Sensor {
            name: "",
            registers: &[],
            factor: 0,
            is_signed: false,
            is_mut: false,
            max: None,
            min: None,
            metric,
        }
    }
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
            is_mut: false,
            max: None,
            min: None,
            metric,
        }
    }

    pub fn new_mut<'a>(
        name: &'a str,
        registers: &'a [u16],
        factor: i64,
        is_signed: bool,
        max: Option<u16>,
        min: Option<u16>,
    ) -> Sensor<'a> {
        let metric = IntGauge::new(slug_name(name), name).unwrap();
        REGISTRY.register(Box::new(metric.clone())).unwrap();

        Sensor {
            name,
            registers,
            factor,
            is_signed,
            is_mut: true,
            max,
            min,
            metric,
        }
    }
}

#[async_trait]
impl SensorWrite for Sensor<'_> {
    async fn write(
        &self,
        mut ctx: Box<dyn Writer>,
        value: u16,
    ) -> Result<Box<dyn Writer>, Box<dyn std::error::Error>> {
        if !self.is_mut {
            return Err(Box::new(Error::new(ErrorKind::InvalidData, "Not Mut")));
        }
        if let Some(max) = self.max {
            if max < value {
                return Err(Box::new(Error::new(ErrorKind::InvalidData, "Value to large")));
            }
        }
        if let Some(min) = self.min {
            if min > value {
                return Err(Box::new(Error::new(ErrorKind::InvalidData, "Value to small")));
            }
        }

        // Can't write more than one value at a time right now.
        if self.registers.len() > 1 {
            return Err(Box::new(Error::new(ErrorKind::InvalidData, "invalid response")));
        }

        ctx.write_single_register(self.registers[0], value).await?;
        Ok(ctx)
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
            output += signed / self.factors[i];
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
    pub(crate) metric: IntGaugeVec,
}

impl<'a> FaultSensor {
    pub fn new(name: &'a str, registers: [u16; 4]) -> FaultSensor {
        let metric = IntGaugeVec::new(Opts::new(slug_name(name), name), &["code"]).unwrap();
        REGISTRY.register(Box::new(metric.clone())).unwrap();

        FaultSensor { registers, metric }
    }
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
        let faults = faults_decode(raw_output);

        for fault in faults.iter() {
            self.metric.with_label_values(&[&fault.to_string()]).set(1);
        }

        Ok((
            ctx,
            faults
                .iter()
                .map(|f| format!("F{}", f))
                .collect::<Vec<_>>()
                .join(", "),
        ))
    }
}

fn faults_decode(reg_vals: Vec<u16>) -> Vec<u16> {
    let mut faults: Vec<u16> = Vec::new();
    let mut off = 0;
    for val in reg_vals.iter() {
        for bit in 0..16 {
            let mask = 1 << bit;
            if mask & val != 0 {
                faults.push(bit + off + 1);
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
    Compound(CompoundSensor<'a>),
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
        requests: Vec<Result<Request, Error>>
    }

    impl ClientMock {
        pub(crate) fn set_next_response(&mut self, next_response: Result<Response, Error>) {
            self.responses.push(next_response)
        }

        pub(crate) fn set_next_request(&mut self, next_request: Result<Request, Error>) {
            self.requests.push(next_request)
        }
    }

    #[async_trait]
    impl Client for ClientMock {
        async fn call<'a>(&'a mut self, request: Request) -> Result<Response, Error> {
            match request {
                Request::ReadHoldingRegisters(_, _) => {
                    *self.last_request.lock().unwrap() = Some(request);
                    self.responses.pop().unwrap()
                },
                Request::WriteSingleRegister(addr, val) => {
                    if let Ok(Request::WriteSingleRegister(exp_addr, exp_val)) = self.requests.pop().unwrap() {
                        if exp_addr == addr && exp_val == val {
                            return Ok(Response::WriteSingleRegister(addr, val))
                        }
                    };
                    Err(Error::new(ErrorKind::InvalidData, "invalid response"))
                },
                _ => todo!(),
                    
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
            todo!()
        }

        async fn read_coils(&mut self, _: u16, _: u16) -> Result<Vec<bool>, Error> {
            todo!()
        }

        async fn read_input_registers(&mut self, _: u16, _: u16) -> Result<Vec<u16>, Error> {
            todo!()
        }

        async fn read_write_multiple_registers(
            &mut self,
            _: u16,
            _: u16,
            _: u16,
            _: &[u16],
        ) -> Result<Vec<u16>, Error> {
            todo!()
        }
    }

    #[async_trait]
    impl Writer for Context {
        async fn write_single_register<'a>(&'a mut self, addr: u16, val: u16) -> Result<(), Error> {
            self.client.call(Request::WriteSingleRegister(addr, val)).await?;
            Ok(())
        }

        async fn write_single_coil(&mut self, _: u16, _: bool) -> Result<(), Error> {
            todo!()
        }

        async fn write_multiple_coils(&mut self, _: u16, _: &[bool]) -> Result<(), Error> {
            todo!()
        }

        async fn write_multiple_registers(&mut self, _: u16, _: &[u16]) -> Result<(), Error> {
            todo!()
        }

        async fn masked_write_register(&mut self, _: u16, _: u16, _: u16) -> Result<(), Error> {
            todo!()
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
        assert_eq!(vec![1u16], faults_decode(vec![0x01, 0x0, 0x0, 0x0]));

        assert_eq!(vec![8u16], faults_decode(vec![0x80, 0x0, 0x0, 0x0]));

        assert_eq!(vec![32u16], faults_decode(vec![0x0, 0x8000, 0x0, 0x0]));

        assert_eq!(
            vec![1u16, 8u16, 32u16],
            faults_decode(vec![0x81, 0x8000, 0x0, 0x0])
        );

        assert_eq!(vec![33u16], faults_decode(vec![0x0, 0x0, 0x1, 0x0]));
    }

    #[tokio::test]
    async fn faults_sensor_read() {
        let mock_out: Vec<u16> = vec![0x81, 0x8000, 0x0, 0x0];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Box::new(Context { client });

        let fault_sensor = FaultSensor::new("Sunsynk Faults Sensor", [103, 104, 105, 106]);

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

    #[tokio::test]
    async fn write_data_to_modbus_over_serial() {
        let mock_reg = 220;
        let mock_val = 45;
        let mut client = Box::<ClientMock>::default();
        client.set_next_request(Ok(Request::WriteSingleRegister(mock_reg, mock_val)));
        let ctx = Box::new(Context { client });

        let sensor = Sensor::new_mut("Battery Shutdown Voltage", &[220], 100, false, Some(60), None);

        let _ctx = sensor.write(ctx, mock_val).await.unwrap();
    }
}
