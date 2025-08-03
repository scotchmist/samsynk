use crate::helpers::group_consecutive;
use tokio::sync::{mpsc, oneshot};
use tokio_modbus::client::Context;
use tokio_modbus::prelude::*;

pub enum Query {
    Read(Vec<u16>),
    Write((u16, u16)),
}

#[derive(Debug)]
pub enum Response {
    Read(Vec<u16>),
    Write(()),
}

pub async fn modbus_read(queue: ModbusQueue, registers: Vec<u16>) -> Vec<u16> {
    let (tx, rx) = oneshot::channel::<Response>();

    queue
        .send((Query::Read(registers), tx))
        .await
        .expect("Could not add Query::Read to Modbus queue.");
    let resp = rx.await.expect("No response to Modbus read query.");

    let Response::Read(output) = resp else {
        unreachable!("Got a write while waiting for a read.")
    };
    output
}

pub type ModbusQueue = mpsc::Sender<(Query, oneshot::Sender<Response>)>;

pub async fn query_modbus_source(
    mut ctx: Context,
    mut job_queue: mpsc::Receiver<(Query, oneshot::Sender<Response>)>,
) {
    while let Some((registers, sender)) = job_queue.recv().await {
        match registers {
            Query::Read(regs) => {
                let mut output: Vec<u16> = Vec::new();
                for (reg, len) in group_consecutive(regs) {
                    let raw_out = ctx
                        .read_holding_registers(reg, len)
                        .await
                        .expect("Could not read modbus register.");
                    output.extend(raw_out);
                }
                sender
                    .send(Response::Read(output))
                    .expect("Could not send back modbus read response.");
            }
            Query::Write((reg, data)) => {
                ctx.write_single_register(reg, data)
                    .await
                    .expect("Could not write modbus register.");
                sender
                    .send(Response::Write(()))
                    .expect("Could not send back modbus write response.");
            }
        }
    }
}
