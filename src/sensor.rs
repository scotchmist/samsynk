use crate::helpers::{group_consecutive, signed, slug_name};
use crate::sensor_definitions::*;
use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::{IntGauge, IntGaugeVec, Opts, Registry};
use std::collections::HashMap;
use std::error::Error;
use std::marker::{Send, Sync};
use std::ops::Deref;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;
pub use tokio_modbus::client::Context;
use tokio_modbus::prelude::*;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
}

#[derive(Default, Clone)]
pub enum PriorityMode {
    #[default]
    BatteryFirst = 0,
    LoadFirst,
}

#[derive(Debug)]
enum SensorError {
    IsNotMut,
}

impl std::fmt::Display for SensorError {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Ok(())
    }
}

impl Error for SensorError {}

#[async_trait]
pub trait SensorRead {
    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>>;
}

#[derive(Clone, Debug)]
pub struct Sensor<'a> {
    pub name: &'a str,
    pub registers: &'a [u16],
    factor: i64,
    is_signed: bool,
    is_mut: bool,
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
            metric,
        }
    }
}

#[async_trait]
pub trait SensorWrite<T: Send + Sync> {
    async fn write(
        &self,
        ctx: Arc<Mutex<dyn Writer>>,
        value: T,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

#[async_trait]
impl SensorWrite<AtomicU16> for Sensor<'_> {
    async fn write(
        &self,
        ctx: Arc<Mutex<dyn Writer>>,
        data: AtomicU16,
    ) -> Result<(), Box<dyn Error>> {
        if self.is_mut {
            ctx.lock()
                .await
                .write_single_register(self.registers[0], data.load(Ordering::Relaxed))
                .await?;
        } else {
            return Err(SensorError::IsNotMut.into());
        }
        Ok(())
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
            metric,
        }
    }

    pub fn new_mut<'a>(
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
            is_mut: true,
            metric,
        }
    }

    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<i64, Box<dyn Error>> {
        let mut output: Vec<u16> = Vec::new();
        for (reg, len) in group_consecutive(self.registers.to_vec()) {
            let raw_out = ctx.lock().await.read_holding_registers(reg, len).await?;
            output.extend(raw_out);
        }

        let mut value: i64 = 0;
        for (i, reg_val) in output.iter().enumerate() {
            value += (reg_val << (16 * i)) as i64
        }

        if self.is_signed {
            value = signed(value)
        }
        value /= self.factor;
        Ok(value)
    }
}

#[derive(Clone, Debug)]
pub struct PriorityModeSensor<'a>(pub Sensor<'a>);

#[derive(Clone, Debug)]
pub struct LoadLimitSensor<'a>(pub Sensor<'a>);

#[derive(Clone, Debug)]
pub struct ProgChargeOptionsSensor<'a>(pub Sensor<'a>);

#[derive(Clone, Debug)]
pub struct ProgModeOptionsSensor<'a>(pub Sensor<'a>);

#[derive(Clone, Debug)]
pub struct NumberSensor<'a>(pub Sensor<'a>);

#[derive(Clone, Debug)]
pub struct BasicSensor<'a>(pub Sensor<'a>);

impl<'a> Deref for BasicSensor<'a> {
    type Target = Sensor<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl SensorRead for BasicSensor<'_> {
    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>> {
        let output = self.deref().read(ctx).await.unwrap();
        self.0.metric.set(output);
        Ok(format!("{}", output))
    }
}

#[derive(Clone, Debug)]
pub struct TemperatureSensor<'a>(pub Sensor<'a>);

impl<'a> Deref for TemperatureSensor<'a> {
    type Target = Sensor<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl SensorRead for TemperatureSensor<'_> {
    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>> {
        let mut output = self.deref().read(ctx).await.unwrap();
        output -= 100_i64;
        self.0.metric.set(output);
        Ok(format!("{}", output))
    }
}

#[derive(Clone, Debug)]
pub struct CompoundSensor<'a> {
    pub name: &'a str,
    pub registers: &'a [u16],
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
    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>> {
        let mut output: i64 = 0;
        for (i, reg) in self.registers.iter().enumerate() {
            let raw_output = ctx.lock().await.read_holding_registers(*reg, 1u16).await?;
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
        Ok(format!("{}", output))
    }
}

#[derive(Clone, Debug)]
pub struct FaultSensor<'a> {
    pub name: &'a str,
    pub(crate) registers: [u16; 4],
    pub(crate) metric: IntGaugeVec,
}

impl<'a> FaultSensor<'_> {
    pub fn new(name: &'a str, registers: [u16; 4]) -> FaultSensor {
        let metric = IntGaugeVec::new(Opts::new(slug_name(name), name), &["code"]).unwrap();
        REGISTRY.register(Box::new(metric.clone())).unwrap();

        FaultSensor {
            name,
            registers,
            metric,
        }
    }
}

#[async_trait]
impl SensorRead for FaultSensor<'_> {
    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>> {
        let mut output: Vec<u16> = Vec::new();
        for (reg, len) in group_consecutive(self.registers.to_vec()) {
            let raw_output = ctx.lock().await.read_holding_registers(reg, len).await?;
            output.extend(raw_output);
        }
        let faults = faults_decode(output);

        for fault in faults.iter() {
            self.metric.with_label_values(&[&fault.to_string()]).set(1);
        }

        Ok(faults
            .iter()
            .map(|f| format!("F{}", f))
            .collect::<Vec<_>>()
            .join(", "))
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

#[derive(Clone, Debug)]
pub struct SerialSensor<'a> {
    pub name: &'a str,
    pub(crate) registers: [u16; 5],
}

#[async_trait]
impl SensorRead for SerialSensor<'_> {
    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>> {
        let raw_value = ctx
            .lock()
            .await
            .read_holding_registers(self.registers[0], self.registers.len() as u16)
            .await?;
        let mut output = "".to_owned();
        for b16 in raw_value {
            let first_char = format!("{}", (b16 >> 8) as u8);
            let second_char = format!("{}", (b16 & 0xFF) as u8);
            output.push_str(&first_char);
            output.push_str(&second_char);
        }

        Ok(output)
    }
}

#[derive(Clone, Debug)]
pub enum SDStatus {
    Fault,
    Ok,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct SDStatusSensor<'a> {
    pub name: &'a str,
    pub(crate) registers: [u16; 1],
}

#[async_trait]
impl SensorRead for SDStatusSensor<'_> {
    async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>> {
        let raw_value = ctx
            .lock()
            .await
            .read_holding_registers(self.registers[0], 1u16)
            .await?;

        let status = match raw_value[0] {
            1000 => SDStatus::Fault,
            2000 => SDStatus::Ok,
            _ => SDStatus::Unknown,
        };
        Ok(format!("{:?}", status))
    }
}

#[derive(Clone, Debug)]
pub enum SensorTypes<'a> {
    Basic(BasicSensor<'a>),
    Temperature(TemperatureSensor<'a>),
    Compound(CompoundSensor<'a>),
    Serial(SerialSensor<'a>),
    Fault(FaultSensor<'a>),
}

impl SensorTypes<'_> {
    pub async fn read(&self, ctx: Arc<Mutex<dyn Reader>>) -> Result<String, Box<dyn Error>> {
        match self {
            SensorTypes::Basic(s) => s.read(ctx.clone()).await,
            SensorTypes::Temperature(s) => s.read(ctx.clone()).await,
            SensorTypes::Compound(s) => s.read(ctx.clone()).await,
            SensorTypes::Fault(s) => s.read(ctx.clone()).await,
            SensorTypes::Serial(s) => s.read(ctx.clone()).await,
        }
    }
}

pub fn register_sensors() -> HashMap<String, SensorTypes<'static>> {
    let mut all_sensors: HashMap<String, SensorTypes<'static>> = HashMap::new();

    for sensor in SENSORS.clone().into_iter() {
        all_sensors.insert(
            slug_name(sensor.name).to_owned(),
            SensorTypes::Basic(sensor.clone()),
        );
    }
    for sensor in TEMP_SENSORS.clone().into_iter() {
        all_sensors.insert(
            slug_name(sensor.0.name).to_owned(),
            SensorTypes::Temperature(sensor.clone()),
        );
    }
    for sensor in COMPOUND_SENSORS.clone().into_iter() {
        all_sensors.insert(
            slug_name(sensor.name).to_owned(),
            SensorTypes::Compound(sensor.clone()),
        );
    }
    all_sensors.insert(
        slug_name(FAULTS.name).to_owned(),
        SensorTypes::Fault(FAULTS.clone()),
    );
    all_sensors
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;
    use tokio_modbus::prelude::Response::ReadHoldingRegisters;

    use std::{
        fmt::Debug,
        io::{Error, ErrorKind},
    };

    #[derive(Debug)]
    struct Context {
        client: Box<dyn Client>,
    }

    #[async_trait]
    impl Client for Context {
        async fn call(&mut self, request: Request<'_>) -> Result<Response, Error> {
            self.client.call(request).await
        }
    }

    #[derive(Default, Debug)]
    pub(crate) struct ClientMock {
        slave: Option<Slave>,
        last_request: Mutex<Option<Request<'static>>>,
        responses: Vec<Result<Response, Error>>,
        requests: Vec<Result<Request<'static>, Error>>,
    }

    impl ClientMock {
        pub(crate) fn set_next_response(&mut self, next_response: Result<Response, Error>) {
            self.responses.push(next_response)
        }

        pub(crate) fn set_next_request(&mut self, next_request: Result<Request<'static>, Error>) {
            self.requests.push(next_request)
        }
    }

    #[async_trait]
    impl Client for ClientMock {
        async fn call(&mut self, request: Request<'_>) -> Result<Response, Error> {
            match request {
                Request::ReadHoldingRegisters(_, _) => {
                    *self.last_request.lock().await = Some(request.into_owned());
                    self.responses.pop().unwrap()
                }
                Request::WriteSingleRegister(addr, val) => {
                    if let Ok(Request::WriteSingleRegister(exp_addr, exp_val)) =
                        self.requests.pop().unwrap()
                    {
                        if exp_addr == addr && exp_val == val {
                            return Ok(Response::WriteSingleRegister(addr, val));
                        }
                    };
                    Err(Error::new(ErrorKind::InvalidData, "invalid response"))
                }
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
            self.client
                .call(Request::WriteSingleRegister(addr, val))
                .await?;
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
        let ctx = Arc::new(Mutex::new(Context { client }));

        let sensor = BasicSensor(Sensor::new("Battery Voltage", &[183], 1, false));

        let value = sensor.read(ctx).await.unwrap();

        assert_eq!("240", value);
    }

    /// Check that the Temperature read method works as expected.
    #[tokio::test]
    async fn temp_sensor_read() {
        let mock_out = vec![1110];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Arc::new(Mutex::new(Context { client }));

        let sensor = TemperatureSensor(Sensor::new("Battery Temperature", &[182], 10, false));

        let value = sensor.read(ctx).await.unwrap();

        assert_eq!("11", value);
    }

    /// Check that the Serial Number read method works as expected.
    #[tokio::test]
    async fn serial_sensor_read() {
        let mock_out = vec![513, 513, 513, 513, 513];
        let mut client = Box::<ClientMock>::default();
        client.set_next_response(Ok(ReadHoldingRegisters(mock_out)));
        let ctx = Arc::new(Mutex::new(Context { client }));

        let serial = SerialSensor {
            name: "Serial Number",
            registers: [3, 4, 5, 6, 7],
        };

        let value = serial.read(ctx).await.unwrap();

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
        let ctx = Arc::new(Mutex::new(Context { client }));

        let fault_sensor = FaultSensor::new("Sunsynk Faults Sensor", [103, 104, 105, 106]);

        let value = fault_sensor.read(ctx).await.unwrap();

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
        let ctx = Arc::new(Mutex::new(Context { client }));

        let compound_sensor =
            CompoundSensor::new("Grid Current", &[160, 161], &[1, -1], false, false);

        let value = compound_sensor.read(ctx).await.unwrap();

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
        let ctx = Arc::new(Mutex::new(Context { client }));

        let compound_sensor =
            CompoundSensor::new("Fake Sensor", &[160, 161], &[1, -1], false, false);

        let value = compound_sensor.read(ctx).await.unwrap();

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
        let ctx = Arc::new(Mutex::new(Context { client }));

        let compound_sensor = CompoundSensor::new(
            "Compound Sensor No Negative",
            &[160, 161],
            &[1, -1],
            true,
            false,
        );

        let value = compound_sensor.read(ctx).await.unwrap();

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
        let ctx = Arc::new(Mutex::new(Context { client }));

        let compound_sensor = CompoundSensor::new(
            "Compound Sensor Absolute",
            &[160, 161],
            &[1, -1],
            false,
            true,
        );

        let value = compound_sensor.read(ctx).await.unwrap();

        assert_eq!("600", value);
    }

    #[tokio::test]
    async fn write_data_to_modbus_over_serial() {
        let mock_reg = 220;
        let mock_val = AtomicU16::new(45);
        let mut client = Box::<ClientMock>::default();
        client.set_next_request(Ok(tokio_modbus::Request::WriteSingleRegister(
            mock_reg,
            mock_val.load(Ordering::Relaxed),
        )));
        let ctx = Arc::new(Mutex::new(Context { client }));

        let sensor = Sensor::new_mut("Battery Shutdown Voltage", &[220], 100, false);

        sensor.write(ctx, mock_val).await.unwrap();
    }

    #[tokio::test]
    async fn write_data_to_modbus_over_serial_err() {
        let mock_val = AtomicU16::new(45);
        let client = Box::<ClientMock>::default();
        let ctx = Arc::new(Mutex::new(Context { client }));

        let sensor = Sensor::new("Load Power", &[178], 1, true);

        assert!(sensor.write(ctx, mock_val).await.is_err());
    }
}
