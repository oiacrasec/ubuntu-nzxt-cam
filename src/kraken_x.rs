use crate::{
    curves::{ChannelCooling, Limits, Point},
    devices::{Info, Transport},
    driver::{self, ChannelKind, CoolingChannel, Driver, FanStatus, LightingChannel, Status},
    lighting::{Capability, ChannelLighting, Mode},
    protocol::{self, KrakenStatus},
};
use anyhow::{Context, Result, bail};
use std::time::{Duration, Instant};

/// Base do canal no comando `02 4d`: fan `0x00`, bomba `0x40` (curva soma `0x80`).
const FAN_BASE: u8 = 0x00;
const PUMP_BASE: u8 = 0x40;
/// Pisos do porte. A bomba nunca desce de 50% (perfil de reset da documentação).
pub const PUMP_LIMITS: Limits = Limits {
    floor: 50,
    ceiling: 100,
};
pub const FAN_LIMITS: Limits = Limits {
    floor: 25,
    ceiling: 100,
};
const READ_TIMEOUT: Duration = Duration::from_millis(2000);

pub struct KrakenX {
    info: Info,
    transport: Box<dyn Transport>,
    firmware: Option<(u8, u8, u8, u8)>,
}

/// Lê todos os relatórios pendentes e devolve o mais recente que `parse` aceitar;
/// se nenhum estiver na fila, espera até `timeout`.
pub(crate) fn latest<T>(
    transport: &mut dyn Transport,
    len: usize,
    timeout: Duration,
    mut parse: impl FnMut(&[u8]) -> Option<T>,
) -> Result<Option<T>> {
    let mut buf = vec![0u8; len];
    let mut found = None;
    for _ in 0..256 {
        let n = transport.read(&mut buf, Duration::ZERO)?;
        if n == 0 {
            break;
        }
        if let Some(value) = parse(&buf[..n]) {
            found = Some(value);
        }
    }
    let deadline = Instant::now() + timeout;
    while found.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let n = transport.read(&mut buf, remaining)?;
        if n == 0 {
            break;
        }
        found = parse(&buf[..n]);
    }
    Ok(found)
}

fn cap(mode: Mode, min: usize, max: usize, speed: bool, direction: bool) -> Capability {
    Capability {
        mode,
        min_colors: min,
        max_colors: max,
        speed,
        direction,
    }
}

impl KrakenX {
    pub fn new(info: Info, transport: Box<dyn Transport>) -> Self {
        Self {
            info,
            transport,
            firmware: None,
        }
    }

    /// Curvas no firmware exigem versão principal ≥ 3.
    fn profiles_supported(&self) -> bool {
        self.firmware.is_some_and(|f| f.0 >= 3)
    }

    fn read_status(&mut self) -> Result<KrakenStatus> {
        latest(
            self.transport.as_mut(),
            64,
            READ_TIMEOUT,
            protocol::kraken_status,
        )?
        .context("O Kraken X não enviou relatório de status")
    }

    fn channel(name: &str) -> Result<(u8, Limits)> {
        match name {
            "pump" => Ok((PUMP_BASE, PUMP_LIMITS)),
            "fan" => Ok((FAN_BASE, FAN_LIMITS)),
            _ => bail!("Canal {name} não existe no Kraken X"),
        }
    }

    fn write_all(&mut self, reports: &[Vec<u8>]) -> Result<()> {
        for report in reports {
            self.transport.write(report)?;
        }
        Ok(())
    }
}

impl Driver for KrakenX {
    fn info(&self) -> &Info {
        &self.info
    }

    fn cooling_channels(&self) -> Vec<CoolingChannel> {
        let internal = self.profiles_supported();
        vec![
            CoolingChannel {
                name: "pump".into(),
                kind: ChannelKind::Pump,
                limits: PUMP_LIMITS,
                internal_profile: internal,
            },
            CoolingChannel {
                name: "fan".into(),
                kind: ChannelKind::Fan,
                limits: FAN_LIMITS,
                internal_profile: internal,
            },
        ]
    }

    fn lighting_channels(&self) -> Vec<LightingChannel> {
        let common = vec![
            cap(Mode::Off, 0, 0, false, false),
            cap(Mode::Fixed, 1, 1, false, false),
            cap(Mode::Fading, 2, 8, true, false),
            cap(Mode::SpectrumWave, 0, 0, true, true),
            cap(Mode::Breathing, 1, 8, true, false),
            cap(Mode::Pulse, 1, 8, true, false),
        ];
        let mut ring = common.clone();
        ring.extend([
            cap(Mode::Marquee, 1, 1, true, true),
            cap(Mode::CoveringMarquee, 1, 8, true, true),
            cap(Mode::Alternating, 2, 2, true, true),
            cap(Mode::TaiChi, 2, 2, true, false),
            cap(Mode::WaterCooler, 0, 0, true, false),
            cap(Mode::Loading, 1, 1, true, false),
            cap(Mode::Wings, 1, 1, true, false),
        ]);
        vec![
            LightingChannel {
                name: "logo".into(),
                capabilities: common,
            },
            LightingChannel {
                name: "ring".into(),
                capabilities: ring,
            },
        ]
    }

    fn initialize(&mut self) -> Result<()> {
        let status = self.read_status()?;
        self.firmware = Some(status.firmware);
        if self.profiles_supported() {
            // Curva de segurança pelo líquido antes de qualquer outra escrita.
            for name in ["pump", "fan"] {
                let (_, limits) = Self::channel(name)?;
                let points = ChannelCooling::default().points(limits)?;
                self.set_profile(name, &points)?;
            }
        }
        Ok(())
    }

    fn status(&mut self) -> Result<Status> {
        let status = self.read_status()?;
        self.firmware = Some(status.firmware);
        Ok(Status {
            liquid_temperature: Some(status.liquid),
            fans: vec![
                FanStatus {
                    channel: "pump".into(),
                    rpm: Some(u32::from(status.pump_rpm)),
                    duty: None,
                },
                FanStatus {
                    channel: "fan".into(),
                    rpm: Some(u32::from(status.fan_rpm)),
                    duty: None,
                },
            ],
            noise_db: None,
            firmware: Some(protocol::kraken_firmware(status.firmware)),
        })
    }

    fn set_profile(&mut self, channel: &str, points: &[Point]) -> Result<()> {
        if !self.profiles_supported() {
            bail!("Firmware do Kraken X sem suporte a curvas; use rotação calculada");
        }
        let (base, limits) = Self::channel(channel)?;
        let reports = protocol::kraken_profile(base, points, limits.floor)?;
        self.write_all(&reports)
    }

    fn set_duty(&mut self, channel: &str, duty: u8) -> Result<()> {
        let (base, limits) = Self::channel(channel)?;
        if self.profiles_supported() {
            // Curva plana que ainda sobe a 100% com o líquido a 60 °C.
            let points = driver::flat_with_safety(duty, limits);
            return self.set_profile(channel, &points);
        }
        self.transport
            .write(&protocol::fixed_speed(base, limits.clamp(duty)))
    }

    fn set_lighting(&mut self, channel: &str, lighting: &ChannelLighting) -> Result<()> {
        let id = match channel {
            "logo" => 0x01,
            "ring" => 0x02,
            _ => bail!("Canal de iluminação {channel} não existe no Kraken X"),
        };
        let reports = protocol::kraken_lighting(id, lighting)?;
        self.write_all(&reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{Kind, Mock};

    const REAL: [u8; 17] = [
        0x04, 0x21, 0x01, 0x00, 0x00, 0x07, 0xf2, 0, 0, 0, 0xe3, 0x06, 0x00, 0x00, 0x02, 0x1e, 0,
    ];

    fn kraken(mock: &Mock) -> KrakenX {
        let info = Info {
            kind: Kind::KrakenX,
            id: "kraken_x:6D8B4469545".into(),
            node: "/dev/hidraw0".into(),
            serial: "6D8B4469545".into(),
            phys: String::new(),
        };
        KrakenX::new(info, Box::new(mock.clone()))
    }

    #[test]
    fn initialize_reads_firmware_then_writes_safety_curves() {
        let mock = Mock::default();
        mock.queue(&REAL);
        let mut device = kraken(&mock);
        device.initialize().unwrap();
        let writes = mock.written();
        assert_eq!(writes.len(), 72);
        assert!(
            writes[..36]
                .iter()
                .all(|r| (0xc0..=0xe3).contains(&r[2]) && r[4] >= 50)
        );
        assert!(
            writes[36..]
                .iter()
                .all(|r| (0x80..=0xa3).contains(&r[2]) && r[4] >= 25)
        );
        assert_eq!(writes[35][4], 100);
        assert_eq!(writes[71][4], 100);
        assert!(device.cooling_channels().iter().all(|c| c.internal_profile));
    }

    #[test]
    fn missing_report_fails_without_writing() {
        let mock = Mock::default();
        let mut device = kraken(&mock);
        assert!(device.initialize().is_err());
        assert!(mock.written().is_empty());
    }

    #[test]
    fn status_uses_most_recent_report() {
        let mock = Mock::default();
        let mut device = kraken(&mock);
        mock.queue(&REAL);
        let mut newer = REAL;
        newer[1] = 0x25;
        mock.queue(&newer);
        let status = device.status().unwrap();
        assert_eq!(status.liquid_temperature, Some(37.1));
        assert_eq!(status.fans[0].rpm, Some(2034));
        assert_eq!(status.firmware.as_deref(), Some("6.2"));
    }

    #[test]
    fn duty_becomes_flat_curve_with_ceiling_and_pump_floor() {
        let mock = Mock::default();
        mock.queue(&REAL);
        let mut device = kraken(&mock);
        device.initialize().unwrap();
        mock.writes.lock().unwrap().clear();
        device.set_duty("pump", 10).unwrap();
        let writes = mock.written();
        assert_eq!(writes.len(), 36);
        assert!(writes[..35].iter().all(|r| r[4] == 50));
        assert_eq!(writes[35][3..5], [60, 100]);
        assert!(device.set_duty("cpu", 50).is_err());
    }

    #[test]
    fn old_firmware_uses_fixed_speed_and_refuses_profiles() {
        let mock = Mock::default();
        let mut old = REAL;
        old[11] = 0x02;
        mock.queue(&old);
        let mut device = kraken(&mock);
        device.initialize().unwrap();
        assert!(mock.written().is_empty());
        device.set_duty("pump", 30).unwrap();
        assert_eq!(&mock.written()[0][..5], &[0x02, 0x4d, 0x40, 0x00, 50]);
        assert!(device.set_profile("pump", &[]).is_err());
        assert!(
            device
                .cooling_channels()
                .iter()
                .all(|c| !c.internal_profile)
        );
    }

    #[test]
    fn lighting_channels_validate_ring_only_modes() {
        let device = kraken(&Mock::default());
        let channels = device.lighting_channels();
        let logo = &channels[0].capabilities;
        let ring = &channels[1].capabilities;
        assert!(!logo.iter().any(|c| c.mode == Mode::Marquee));
        assert!(ring.iter().any(|c| c.mode == Mode::Marquee));
        for capability in ring {
            assert!(protocol::kraken_mode(capability.mode).is_some());
        }
    }
}
