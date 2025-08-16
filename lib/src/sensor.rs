use crate::helpers::{signed, slug_name};
use crate::modbus::{self, Query};
use crate::modbus::{ModbusQueue, Response as ModbusResponse};
use crate::sensor_definitions::{BINARY_SENSORS, COMPOUND_SENSORS, FAULTS, SENSORS, TEMP_SENSORS};
use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::{IntGauge, IntGaugeVec, Opts, Registry};
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::marker::{Send, Sync};
use std::ops::Deref;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use tokio::sync::oneshot;
pub use tokio_modbus::client::Context;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
}

#[derive(Default, Clone)]
pub enum PriorityLoad {
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
    async fn read(&self, ctx: ModbusQueue) -> Result<String, Box<dyn Error>>;
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
    async fn write(&self, queue: ModbusQueue, value: T) -> Result<(), Box<dyn std::error::Error>>;
}

#[async_trait]
impl SensorWrite<AtomicU16> for Sensor<'_> {
    async fn write(&self, queue: ModbusQueue, data: AtomicU16) -> Result<(), Box<dyn Error>> {
        if self.is_mut {
            let (tx, rx) = oneshot::channel::<ModbusResponse>();
            queue
                .send((
                    Query::Write((self.registers[0], data.load(Ordering::Relaxed))),
                    tx,
                ))
                .await
                .expect("Could not write query to Modbus queue.");
            rx.await
                .expect("Did not receive response confirming a written value to the Modbus queue.");
        } else {
            return Err(SensorError::IsNotMut.into());
        }
        Ok(())
    }
}

impl Sensor<'_> {
    #[must_use]
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

    #[must_use]
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

    async fn read(&self, queue: ModbusQueue) -> Result<i64, Box<dyn Error>> {
        let output = modbus::modbus_read(queue, self.registers.to_vec()).await;

        let mut value: i64 = 0;
        for (i, reg_val) in output.iter().enumerate() {
            value += i64::from(reg_val << (16 * i));
        }

        if self.is_signed {
            value = signed(value);
        }
        value /= self.factor;
        Ok(value)
    }
}

#[derive(Clone, Debug)]
pub struct BinarySensor<'a>(pub Sensor<'a>);

impl<'a> Deref for BinarySensor<'a> {
    type Target = Sensor<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl SensorRead for BinarySensor<'_> {
    async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        let output = self.0.read(queue).await.unwrap();
        self.0.metric.set(output);
        Ok(format!("{output}"))
    }
}

#[async_trait]
impl SensorWrite<AtomicU16> for BinarySensor<'_> {
    async fn write(&self, queue: ModbusQueue, data: AtomicU16) -> Result<(), Box<dyn Error>> {
        if data.load(Ordering::Relaxed) > 1 {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Unsupported,
                "Binary sensors must receive either a 1 or 0.",
            )));
        }
        let res = self.0.write(queue, data).await.unwrap();
        Ok(res)
    }
}

#[derive(Clone, Debug)]
pub struct ProgChargeOptionsSensor<'a>(pub Sensor<'a>);

//InverterStateSensor(59, "Overall state")
#[derive(Clone, Debug)]
pub struct InverterStateSensor<'a>(pub Sensor<'a>);

#[derive(Clone, Debug)]
pub struct ProgModeOptionsSensor<'a>(pub Sensor<'a>);

#[derive(Clone, Debug)]
pub struct NumberSensor<'a>(pub Sensor<'a>);

impl<'a> Deref for NumberSensor<'a> {
    type Target = Sensor<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
    async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        let output = self.deref().read(queue).await.unwrap();
        self.metric.set(output);
        Ok(format!("{output}"))
    }
}

#[async_trait]
impl SensorWrite<AtomicU16> for BasicSensor<'_> {
    async fn write(&self, queue: ModbusQueue, data: AtomicU16) -> Result<(), Box<dyn Error>> {
        let res = self.write(queue, data).await.unwrap();
        Ok(res)
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
    async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        let mut output = self.deref().read(queue).await.unwrap();
        output -= 100_i64;
        self.metric.set(output);
        Ok(format!("{output}"))
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
    #[must_use]
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
    async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        let mut raw_output = Vec::new();
        for reg in self.registers.iter() {
            raw_output.append(&mut modbus::modbus_read(queue.clone(), vec![*reg]).await);
        }

        let mut output: i64 = raw_output
            .iter()
            .enumerate()
            .map(|(i, reg)| {
                let signed = if self.factors[i] < 0 {
                    signed(i64::from(*reg))
                } else {
                    i64::from(*reg)
                };
                signed / self.factors[i]
            })
            .sum();

        if self.absolute && output < 0 {
            output = -output;
        }
        if self.no_negative && output < 0 {
            output = 0;
        }

        self.metric.set(output);
        Ok(format!("{output}"))
    }
}

#[derive(Clone, Debug)]
pub struct FaultSensor<'a> {
    pub name: &'a str,
    pub(crate) registers: [u16; 4],
    pub(crate) metric: IntGaugeVec,
}

impl<'a> FaultSensor<'_> {
    #[must_use]
    pub fn new(name: &'a str, registers: [u16; 4]) -> FaultSensor<'a> {
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
    async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        let output = modbus::modbus_read(queue, self.registers.to_vec()).await;
        let faults = faults_decode(output);

        for fault in &faults {
            self.metric.with_label_values(&[&fault.to_string()]).set(1);
        }

        Ok(faults
            .iter()
            .map(|f| format!("F{f}"))
            .collect::<Vec<_>>()
            .join(", "))
    }
}

fn faults_decode(reg_vals: Vec<u16>) -> Vec<u16> {
    let mut faults: Vec<u16> = Vec::new();
    let mut off = 0;
    for val in &reg_vals {
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
    async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        let raw_value = modbus::modbus_read(queue, self.registers.to_vec()).await;

        let mut output = String::new();
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
    async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        let raw_value = modbus::modbus_read(queue, self.registers.to_vec()).await;

        let status = match raw_value[0] {
            1000 => SDStatus::Fault,
            2000 => SDStatus::Ok,
            _ => SDStatus::Unknown,
        };
        Ok(format!("{status:?}"))
    }
}

#[derive(Clone, Debug)]
pub enum SensorTypes<'a> {
    Basic(BasicSensor<'a>),
    Binary(BinarySensor<'a>),
    Compound(CompoundSensor<'a>),
    Fault(FaultSensor<'a>),
    Serial(SerialSensor<'a>),
    Temperature(TemperatureSensor<'a>),
}

impl SensorTypes<'_> {
    pub async fn read(&self, queue: ModbusQueue) -> Result<String, Box<dyn Error>> {
        match self {
            SensorTypes::Basic(s) => s.read(queue).await,
            SensorTypes::Binary(s) => s.read(queue).await,
            SensorTypes::Temperature(s) => s.read(queue).await,
            SensorTypes::Compound(s) => s.read(queue).await,
            SensorTypes::Fault(s) => s.read(queue).await,
            SensorTypes::Serial(s) => s.read(queue).await,
        }
    }

    pub async fn write(&self, queue: ModbusQueue, data: AtomicU16) -> Result<(), Box<dyn Error>> {
        match self {
            SensorTypes::Basic(s) => s.write(queue, data).await,
            SensorTypes::Binary(s) => s.write(queue, data).await,
            _ => Err(Box::new(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Sensor not writeable.",
            ))),
        }
    }
}

#[must_use]
pub fn register_sensors() -> HashMap<String, SensorTypes<'static>> {
    let mut all_sensors: HashMap<String, SensorTypes<'static>> = HashMap::new();

    for sensor in SENSORS.clone() {
        all_sensors.insert(
            slug_name(sensor.name).clone(),
            SensorTypes::Basic(sensor.clone()),
        );
    }
    for sensor in BINARY_SENSORS.clone() {
        all_sensors.insert(
            slug_name(sensor.name).clone(),
            SensorTypes::Binary(sensor.clone()),
        );
    }
    for sensor in TEMP_SENSORS.clone() {
        all_sensors.insert(
            slug_name(sensor.0.name).clone(),
            SensorTypes::Temperature(sensor.clone()),
        );
    }
    for sensor in COMPOUND_SENSORS.clone() {
        all_sensors.insert(
            slug_name(sensor.name).clone(),
            SensorTypes::Compound(sensor.clone()),
        );
    }

    all_sensors.insert(
        slug_name(FAULTS.name).clone(),
        SensorTypes::Fault(FAULTS.clone()),
    );
    all_sensors
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::modbus::{Query, Response};
    use tokio::sync::{mpsc, oneshot};

    async fn mock_query_modbus_source(
        mut dummy_values: Vec<(Query, Response)>,
        mut job_queue: mpsc::Receiver<(Query, oneshot::Sender<Response>)>,
    ) {
        while let Some((registers, sender)) = job_queue.recv().await {
            let (query, response) = dummy_values.pop().unwrap();

            if registers == query {
                sender.send(response).unwrap();
            }
        }
    }

    // Basic test that you can read data from a sensor.
    #[tokio::test]
    async fn read_data_from_modbus_sensor() {
        let mock_out = vec![240];
        let registers = &[183];

        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);
        let dummy_values = vec![(Query::Read(registers.to_vec()), Response::Read(mock_out))];
        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let sensor = BasicSensor(Sensor::new("Battery Voltage", registers, 1, false));

        let result = sensor.read(modbus_sender).await.unwrap();

        assert_eq!("240", result);
    }

    /// Check that the Temperature read method works as expected.
    #[tokio::test]
    async fn temp_sensor_read() {
        let mock_out = vec![1110];
        let registers = &[182];

        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);
        let dummy_values = vec![(Query::Read(registers.to_vec()), Response::Read(mock_out))];
        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let sensor = TemperatureSensor(Sensor::new("Battery Temperature", registers, 10, false));

        let value = sensor.read(modbus_sender).await.unwrap();

        assert_eq!("11", value);
    }

    /// Check that the Serial Number read method works as expected.
    #[tokio::test]
    async fn serial_sensor_read() {
        let mock_out = vec![513, 513, 513, 513, 513];
        let registers = &[3, 4, 5, 6, 7];

        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);
        let dummy_values = vec![(Query::Read(registers.to_vec()), Response::Read(mock_out))];
        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let serial = SerialSensor {
            name: "Serial Number",
            registers: *registers,
        };

        let value = serial.read(modbus_sender).await.unwrap();

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
        let registers = [103u16, 104, 105, 106];
        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);
        let dummy_values = vec![(Query::Read(registers.to_vec()), Response::Read(mock_out))];
        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let fault_sensor = FaultSensor::new("Sunsynk Faults Sensor", registers);

        let value = fault_sensor.read(modbus_sender).await.unwrap();

        assert_eq!("F1, F8, F32", value);
    }

    #[tokio::test]
    async fn compound_sensor_read() {
        let mock_out: Vec<u16> = vec![1000, 800];
        let registers = [160, 161];
        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);

        let mut dummy_values = Vec::new();
        for (mock_val, reg) in mock_out.iter().zip(registers).rev() {
            dummy_values.push((Query::Read(vec![reg]), Response::Read(vec![*mock_val])))
        }

        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let compound_sensor =
            CompoundSensor::new("Grid Current", &registers, &[1, -1], false, false);

        let value = compound_sensor.read(modbus_sender).await.unwrap();

        assert_eq!("200", value);
    }

    #[tokio::test]
    async fn compound_sensor_read2() {
        let mock_out: Vec<u16> = vec![200, 800];
        let registers = [160, 161];
        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);

        let mut dummy_values = Vec::new();
        for (mock_val, reg) in mock_out.iter().zip(registers).rev() {
            dummy_values.push((Query::Read(vec![reg]), Response::Read(vec![*mock_val])))
        }
        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let compound_sensor =
            CompoundSensor::new("Fake Sensor", &registers, &[1, -1], false, false);

        let value = compound_sensor.read(modbus_sender).await.unwrap();

        assert_eq!("-600", value);
    }

    #[tokio::test]
    async fn compound_sensor_read_no_negative() {
        let mock_out: Vec<u16> = vec![200, 800];
        let registers = [160, 161];
        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);

        let mut dummy_values = Vec::new();
        for (mock_val, reg) in mock_out.iter().zip(registers).rev() {
            dummy_values.push((Query::Read(vec![reg]), Response::Read(vec![*mock_val])))
        }
        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let compound_sensor = CompoundSensor::new(
            "Compound Sensor No Negative",
            &registers,
            &[1, -1],
            true,
            false,
        );

        let value = compound_sensor.read(modbus_sender).await.unwrap();

        assert_eq!("0", value);
    }

    #[tokio::test]
    async fn compound_sensor_absolute() {
        let mock_out: Vec<u16> = vec![200, 800];
        let registers = [160, 161];
        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);

        let mut dummy_values = Vec::new();
        for (mock_val, reg) in mock_out.iter().zip(registers).rev() {
            dummy_values.push((Query::Read(vec![reg]), Response::Read(vec![*mock_val])))
        }
        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let compound_sensor = CompoundSensor::new(
            "Compound Sensor Absolute",
            &registers,
            &[1, -1],
            false,
            true,
        );

        let value = compound_sensor.read(modbus_sender).await.unwrap();

        assert_eq!("600", value);
    }

    #[tokio::test]
    async fn write_data_to_modbus_over_serial() {
        let mock_reg = 220;
        let mock_val = 45;

        let dummy_values = vec![(Query::Write((mock_reg, mock_val)), Response::Write(()))];

        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);

        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let sensor = Sensor::new_mut("Battery Shutdown Voltage", &[220], 100, false);

        sensor
            .write(modbus_sender, AtomicU16::new(mock_val))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn write_data_to_modbus_over_serial_err() {
        let mock_val = 45;
        let mock_reg = 178;

        let dummy_values = vec![(Query::Write((mock_reg, mock_val)), Response::Write(()))];

        let (modbus_sender, modbus_receiver) =
            mpsc::channel::<(Query, oneshot::Sender<Response>)>(10);

        tokio::spawn(mock_query_modbus_source(dummy_values, modbus_receiver));

        let sensor = Sensor::new("Load Power", &[178], 1, true);

        assert!(
            sensor
                .write(modbus_sender, AtomicU16::new(mock_val))
                .await
                .is_err()
        );
    }
}
