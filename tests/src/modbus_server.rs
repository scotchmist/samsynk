use std::io::Error;
use tokio::sync::oneshot;
use tokio_modbus::prelude::*;
use futures::future;

struct ModbusService;

impl tokio_modbus::server::Service for ModbusService {
    type Request = SlaveRequest<'static>;
    type Response = Response;
    type Error = Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        match req.request {
            Request::ReadInputRegisters(_addr, cnt) => {
                let mut registers = vec![0; cnt.into()];
                registers[2] = 0x77;
                future::ready(Ok(Response::ReadInputRegisters(registers)))
            }
            _ => unimplemented!(),
        }
    }
}

pub struct ModbusServer {
    pub(crate) address: String,
    pub(crate) baud_rate: u32,
    pub(crate) shutdown_signal: Option<oneshot::Sender<()>>,
    pub(crate) _join_handle: tokio::task::JoinHandle<Result<(), Error>>,
}

impl ModbusServer {
    pub async fn start(address: &str, baud_rate: u32) -> ModbusServer {
        let (tx, rx) = oneshot::channel();
        let builder = tokio_serial::new(address, baud_rate);
        let server_serial = tokio_serial::SerialStream::open(&builder).unwrap();
        let service = ModbusService;
        let server = tokio_modbus::server::rtu::Server::new(server_serial).serve_forever(service);

        let join_handle = tokio::task::spawn(server);

        ModbusServer {
            address: address.to_owned(),
            baud_rate,
            shutdown_signal: Some(tx),
            _join_handle: join_handle
        }
    }
}