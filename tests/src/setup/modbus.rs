use futures::future;
use itertools::Itertools;
use samsynk::sensor::{register_sensors, SensorTypes};
use std::collections::HashMap;
use std::io::Error;
use std::sync::Mutex;
use time::Duration;
use tokio::{process, time};
use tokio_modbus;
use tokio_modbus::prelude::*;

pub const DEFAULT_TEST_PORT_NAMES: &str = concat!(
    env!("CARGO_TARGET_TMPDIR"),
    "/ttyUSB0;",
    env!("CARGO_TARGET_TMPDIR"),
    "/ttyUSB1"
);

pub static MOCK_VALUES: Mutex<Option<HashMap<u16, u16>>> = Mutex::new(None);

pub fn get_test_port_names() -> (&'static str, &'static str) {
    std::option_env!("TEST_PORT_NAMES")
        .unwrap_or(DEFAULT_TEST_PORT_NAMES)
        .split(';')
        .collect_tuple()
        .expect("Expected 2 ports, found a different number.")
}

pub struct SerialInterface {
    _process: process::Child,
    pub port_a: &'static str,
    pub port_b: &'static str,
}

impl SerialInterface {
    pub async fn new(port_a: &'static str, port_b: &'static str) -> Self {
        let args = [
            format!("pty,rawer,echo=0,link={}", port_a),
            format!("pty,rawer,echo=0,link={}", port_b),
        ];
        let process: process::Child = process::Command::new("socat")
            .kill_on_drop(true)
            .args(&args)
            .spawn()
            .expect("unable to spawn socat process: Is socat installed?");

        Self {
            _process: process,
            port_a,
            port_b,
        }
    }
}

#[derive(Default)]
struct ModbusService;

impl tokio_modbus::server::Service for ModbusService {
    type Request = SlaveRequest<'static>;
    type Response = Option<Response>;
    type Error = Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        match req.request {
            Request::ReadHoldingRegisters(addr, cnt) => {
                let mut mock_values = MOCK_VALUES.lock().unwrap();
                if let Some(values) = mock_values.as_mut() {
                    let out = values.get(&addr).unwrap().clone();
                    future::ready(Ok(Some(Response::ReadHoldingRegisters(vec![out]))))
                } else {
                    future::ready(Ok(Some(Response::ReadHoldingRegisters(vec![
                        0u16;
                        cnt as usize
                    ]))))
                }
            }
            Request::WriteSingleRegister(addr, val) => {
                let mut mock_values = MOCK_VALUES.lock().unwrap();
                if let Some(values) = mock_values.as_mut() {
                    values.insert(addr, val);
                    future::ready(Ok(Some(Response::WriteSingleRegister(addr, val))))
                } else {
                    panic!();
                }
            }
            _ => unimplemented!(),
        }
    }
}

pub struct ModbusServer {
    pub(crate) _join_handle: tokio::task::JoinHandle<Result<(), Error>>,
    pub(crate) _serial_interface: tokio::task::JoinHandle<SerialInterface>,
    pub sensors: HashMap<String, SensorTypes<'static>>,
}

impl ModbusServer {
    pub async fn start() -> ModbusServer {
        let port_names = get_test_port_names();

        let serial_interface = SerialInterface::new(port_names.0, port_names.1);
        let serial_handler = tokio::spawn(async move { serial_interface.await });
        // Wait a little bit for the serial interface to start up.
        time::sleep(Duration::from_millis(50)).await;
        let service = ModbusService::default();

        // Baud rate must be 0 here. We skip setting the baud rate so it can be set via ioctl.
        // See: https://docs.rs/serialport/latest/serialport/struct.TTYPort.html
        let server = tokio_modbus::server::rtu::Server::new_from_path(port_names.0, 0)
            .unwrap()
            .serve_forever(service);

        ModbusServer {
            _serial_interface: serial_handler,
            _join_handle: tokio::spawn(async move { server.await }),
            sensors: register_sensors(),
        }
    }
}
