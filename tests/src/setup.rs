use samsynk::server::{Server, origin_url};
use reqwest::Response;
use reqwest;
use async_trait::async_trait;
use test_context::AsyncTestContext;
use tokio::sync::Mutex;
use crate::modbus_server::ModbusServer;

const TEST_IP_ADDR: [u8; 4] = [127, 0, 0, 1];
const TEST_PORT: u16 = 8080;

const MODBUS_ADDRESS: &str = "/Users/sam/ttyUSB0";
const MODBUS_BAUD: u32 = 19200;

static SERVER_STATE: Mutex<Option<TestState>> = Mutex::const_new(None);

struct TestState {
    _http_server: Server,
    _modbus_server: ModbusServer, 
    tests_running: u8,
}

pub(crate) struct TestContext {
    base_url: String,
}

impl TestContext {
    pub fn new(addr: ([u8; 4], u16)) -> TestContext {
        TestContext {base_url: origin_url(addr)}
    }

    pub async fn http_get(&self, uri: &str) -> Result<Response, reqwest::Error> {
        reqwest::get(format!("{}/{}", self.base_url, uri)).await
    }
}

#[async_trait]
impl AsyncTestContext for TestContext{
    async fn setup() -> TestContext {
        let addr = (TEST_IP_ADDR, TEST_PORT);
        let mut server_state = SERVER_STATE.lock().await;
        match *server_state {
            None => {
                let server = Server::start(addr).await;
                let modbus_server = ModbusServer::start(MODBUS_ADDRESS, MODBUS_BAUD).await;
                *server_state = Some(TestState { _http_server: server, tests_running: 1, _modbus_server: modbus_server});
                TestContext::new(addr)
            },
            Some(TestState { ref mut tests_running, .. }) => {
                assert!(*tests_running > 0);
                *tests_running += 1;
                TestContext::new(addr)
            },
        }
    }

    async fn teardown(self) {
        let mut server_state = SERVER_STATE.lock().await;

        match *server_state {
            None => {
                panic!("This should never happen: the server was not running.");
            }
            Some(TestState { ref mut tests_running, .. }) => {
                assert!(*tests_running > 0);
                *tests_running -= 1;
                if *tests_running == 0 {
                    *server_state = None;
                }
            }
        }
    }
}