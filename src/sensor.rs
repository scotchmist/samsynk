use tokio_modbus::client::Context;
use tokio_modbus::prelude::*;
use async_trait::async_trait;


#[async_trait]
pub trait Sensor {
    fn new(name: String, registers: Vec<u8>) -> Self;

    async fn read(&self, ctx: Context) -> Result<(Context, String), Box<dyn std::error::Error>>;
}

pub struct SerialSensor {
    pub name: String,
    pub value: String,
    register: u16,
    register_range: usize,
}

#[async_trait]
impl Sensor for SerialSensor {

    fn new(name: String, registers: Vec<u8>) -> SerialSensor {
        SerialSensor {
            name: name,
            value: String::new(),
            register_range: registers.len(),
            register: registers[0] as u16,
        }
    }

    async fn read(&self, mut ctx: Context) -> Result<(Context, String), Box<dyn std::error::Error>> {
        let raw_value = ctx.read_holding_registers(self.register, self.register_range as u16).await?;
        let mut output = "".to_owned();
        for b16 in raw_value {
            let first_char = &((b16 >> 8) as u8 as char).to_string();
            let second_char = &((b16 & 0xFF) as u8 as char).to_string();
            output.push_str(first_char);
            output.push_str(second_char);
        }

        Ok((ctx, output))
    }
}


pub struct RatedPowerSensor {
    pub name: String,
    pub value: String,
    register: u16,
    register_range: usize,
}

#[async_trait]
impl Sensor for RatedPowerSensor {

    fn new(name: String, registers: Vec<u8>) -> RatedPowerSensor {
        RatedPowerSensor {
            name: name,
            value: String::new(),
            register_range: registers.len(),
            register: registers[0] as u16,
        }
    }

    async fn read(&self, mut ctx: Context) -> Result<(Context, String), Box<dyn std::error::Error>> {
        let raw_value = ctx.read_holding_registers(self.register, self.register_range as u16).await?;
        let mut output = "".to_owned();
        for b16 in raw_value {
            let first_char = &((b16 >> 8) as u8 as char).to_string();
            let second_char = &((b16 & 0xFF) as u8 as char).to_string();
            output.push_str(first_char);
            output.push_str(second_char);
        }

        Ok((ctx, output))
    }
}



