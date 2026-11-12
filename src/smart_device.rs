use crate::{
    curves::Point,
    devices::{Info, Transport},
    driver::{CoolingChannel, Driver, LightingChannel, Status},
    lighting::ChannelLighting,
};
use anyhow::{Result, bail};

pub struct SmartDevice {
    info: Info,
    _transport: Box<dyn Transport>,
}

impl SmartDevice {
    pub fn new(info: Info, transport: Box<dyn Transport>) -> Self {
        Self {
            info,
            _transport: transport,
        }
    }
}

impl Driver for SmartDevice {
    fn info(&self) -> &Info {
        &self.info
    }
    fn cooling_channels(&self) -> Vec<CoolingChannel> {
        Vec::new()
    }
    fn lighting_channels(&self) -> Vec<LightingChannel> {
        Vec::new()
    }
    fn initialize(&mut self) -> Result<()> {
        bail!("protocolo pendente")
    }
    fn status(&mut self) -> Result<Status> {
        bail!("protocolo pendente")
    }
    fn set_profile(&mut self, _: &str, _: &[Point]) -> Result<()> {
        bail!("protocolo pendente")
    }
    fn set_duty(&mut self, _: &str, _: u8) -> Result<()> {
        bail!("protocolo pendente")
    }
    fn set_lighting(&mut self, _: &str, _: &ChannelLighting) -> Result<()> {
        bail!("protocolo pendente")
    }
}
