use crate::{
    curves::{ChannelCooling, Limits, Point, Source},
    devices::{Info, Kind, Transport},
    lighting::{Capability, ChannelLighting},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Pump,
    Fan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoolingChannel {
    /// Nome estável usado na configuração (`pump`, `fan`, `fan1`…).
    pub name: String,
    pub kind: ChannelKind,
    pub limits: Limits,
    /// O firmware executa curvas pela temperatura do líquido sem o aplicativo.
    pub internal_profile: bool,
    /// O dispositivo mede a temperatura do líquido (fonte Líquido disponível).
    #[serde(default)]
    pub liquid_sensor: bool,
}

impl CoolingChannel {
    /// Refrigeração de um canal sem perfil salvo. Sem sensor de líquido, a fonte
    /// padrão é a CPU; Líquido levaria o canal a segurança permanente.
    pub fn default_cooling(&self) -> ChannelCooling {
        ChannelCooling {
            source: if self.liquid_sensor {
                Source::Liquid
            } else {
                Source::Cpu
            },
            ..ChannelCooling::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightingChannel {
    pub name: String,
    pub capabilities: Vec<Capability>,
    /// LEDs endereçáveis do canal (logo 1, anel 8, fita 40).
    pub leds: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FanStatus {
    pub channel: String,
    pub rpm: Option<u32>,
    /// Duty relatado pelo dispositivo, quando ele informa.
    pub duty: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub liquid_temperature: Option<f64>,
    pub fans: Vec<FanStatus>,
    /// dBA do sensor de ruído do Smart Device V1.
    pub noise_db: Option<f64>,
    pub firmware: Option<String>,
}

pub trait Driver: Send {
    fn info(&self) -> &Info;
    fn cooling_channels(&self) -> Vec<CoolingChannel>;
    fn lighting_channels(&self) -> Vec<LightingChannel>;
    /// Sequência de abertura; no Kraken inclui gravar a curva de segurança.
    fn initialize(&mut self) -> Result<()>;
    fn status(&mut self) -> Result<Status>;
    /// Curva por temperatura do líquido, executada pelo firmware.
    fn set_profile(&mut self, channel: &str, points: &[Point]) -> Result<()>;
    /// Rotação calculada pelo governador. Em canais com perfil interno vira uma
    /// curva plana que ainda sobe a 100% na temperatura crítica do líquido.
    fn set_duty(&mut self, channel: &str, duty: u8) -> Result<()>;
    fn set_lighting(&mut self, channel: &str, lighting: &ChannelLighting) -> Result<()>;
}

pub fn open(info: Info, transport: Box<dyn Transport>) -> Box<dyn Driver> {
    match info.kind {
        Kind::KrakenX => Box::new(crate::kraken_x::KrakenX::new(info, transport)),
        Kind::SmartDevice => Box::new(crate::smart_device::SmartDevice::new(info, transport)),
    }
}

/// Curva plana em `duty` até a temperatura crítica do líquido, 100% a partir dela.
pub fn flat_with_safety(duty: u8, limits: Limits) -> Vec<Point> {
    let critical = crate::curves::Source::Liquid.critical();
    let duty = limits.clamp(duty);
    vec![
        Point {
            temperature: 0.0,
            duty,
        },
        Point {
            temperature: critical - 1.0,
            duty,
        },
        Point {
            temperature: critical,
            duty: 100,
        },
    ]
}
