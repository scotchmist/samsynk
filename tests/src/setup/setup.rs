use crate::setup::modbus::{get_test_port_names, ModbusServer, MOCK_VALUES};
use async_trait::async_trait;
use lazy_static::lazy_static;
use reqwest;
use reqwest::Response;
use samsynk::sensor::register_sensors;
use samsynk::sensor::SensorTypes;
use samsynk::server::{origin_url, Server};
use std::collections::HashMap;
use std::sync::Arc;
use test_context::AsyncTestContext;
use tokio::sync::Mutex;
use tokio_modbus::prelude::*;

const TEST_IP_ADDR: [u8; 4] = [127, 0, 0, 1];
const TEST_PORT: u16 = 8080;

lazy_static! {
    pub static ref SERVER_STATE: Mutex<Option<TestState>> = Mutex::new(None);
}

pub struct TestState {
    _http_server: Server,
    _modbus_server: ModbusServer,
}

pub(crate) struct TestContext {
    base_url: String,
    sensors: HashMap<String, SensorTypes<'static>>,
}

impl TestContext {
    pub fn new(addr: ([u8; 4], u16)) -> TestContext {
        TestContext {
            base_url: origin_url(addr),
            sensors: register_sensors(),
        }
    }

    pub async fn http_get(&self, uri: &str) -> Result<Response, reqwest::Error> {
        reqwest::get(self.base_url.clone() + uri).await
    }

    pub async fn http_post(
        &self,
        uri: &str,
        val: &'static str,
    ) -> Result<Response, reqwest::Error> {
        let client = reqwest::Client::new();
        client
            .post(self.base_url.clone() + uri)
            .body(val)
            .send()
            .await
    }

    pub async fn set_sensor_state(
        &mut self,
        sensor_name: String,
        values: Vec<u16>,
    ) -> Result<(), &'static str> {
        let sensor_type = self
            .sensors
            .get(&sensor_name)
            .ok_or("No sensor found with that name.")?;

        let sensor_registers = match sensor_type {
            SensorTypes::Basic(s) => s.registers,
            SensorTypes::Compound(s) => s.registers,
            SensorTypes::Temperature(s) => s.0.registers,
            _ => panic!(),
        };
        let mut mock_values = MOCK_VALUES.lock().unwrap();
        if let None = *mock_values {
            *mock_values = Some(HashMap::new());
        }
        for (index, register) in sensor_registers.iter().enumerate() {
            mock_values
                .as_mut()
                .unwrap()
                .insert(*register, values[index]);
        }

        Ok(())
    }
}

#[async_trait]
impl AsyncTestContext for TestContext {
    async fn setup() -> TestContext {
        let addr = (TEST_IP_ADDR, TEST_PORT);
        let mut server_state = SERVER_STATE.lock().await;
        match *server_state {
            None => {
                let modbus_server = ModbusServer::start().await;
                let modbus_addr = get_test_port_names().1.to_string();
                let builder = tokio_serial::new(modbus_addr, 0);

                let client_serial = tokio_serial::SerialStream::open(&builder)
                    .expect("Could not open a serial connection.");

                let ctx = Arc::new(Mutex::new(rtu::attach(client_serial)));
                let sensors = register_sensors();
                *server_state = Some(TestState {
                    _modbus_server: modbus_server,
                    _http_server: Server::new(ctx.clone(), addr, sensors).await.unwrap(),
                });
                TestContext::new(addr)
            }
            Some(_) => TestContext::new(addr),
        }
    }
}
