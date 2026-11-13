use crate::{
    curves::{Limits, Point},
    devices::{Info, Transport},
    driver::{ChannelKind, CoolingChannel, Driver, FanStatus, LightingChannel, Status},
    kraken_x::latest,
    lighting::{Capability, ChannelLighting, Mode},
    protocol::{self, SmartFanStatus},
};
use anyhow::{Context, Result, bail};
use std::time::Duration;

const FANS: u8 = 3;
/// Piso dos fans: o CAM declara `minActivePercentage` 0,2; o porte usa 25% para fans DC.
pub const FAN_LIMITS: Limits = Limits {
    floor: 25,
    ceiling: 100,
};
const READ_TIMEOUT: Duration = Duration::from_millis(2000);

pub struct SmartDevice {
    info: Info,
    transport: Box<dyn Transport>,
    /// Último relatório recebido de cada fan: o dispositivo envia um por vez.
    fans: [Option<SmartFanStatus>; FANS as usize],
}

fn cap(mode: Mode, min: usize, max: usize, speed: bool, direction: bool) -> Capability {
    Capability {
        mode,
        min_colors: min,
        max_colors: max,
        speed,
        direction,
        per_led: matches!(mode, Mode::Fixed | Mode::Breathing),
    }
}

impl SmartDevice {
    pub fn new(info: Info, transport: Box<dyn Transport>) -> Self {
        Self {
            info,
            transport,
            fans: [None, None, None],
        }
    }

    /// Lê os relatórios pendentes, guardando o mais recente de cada fan.
    fn collect(&mut self, timeout: Duration) -> Result<bool> {
        let mut received = false;
        let mut fans = self.fans.clone();
        latest(self.transport.as_mut(), 64, timeout, |report| {
            let status = protocol::smart_status(report)?;
            let index = usize::from(status.index);
            fans[index] = Some(status);
            received = true;
            // Continua drenando: `None` mantém a leitura até esvaziar a fila.
            None::<()>
        })?;
        self.fans = fans;
        Ok(received)
    }

    fn index(channel: &str) -> Result<u8> {
        channel
            .strip_prefix("fan")
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|n| (1..=FANS).contains(n))
            .map(|n| n - 1)
            .with_context(|| format!("Canal {channel} não existe no Smart Device"))
    }
}

impl Driver for SmartDevice {
    fn info(&self) -> &Info {
        &self.info
    }

    fn cooling_channels(&self) -> Vec<CoolingChannel> {
        (1..=FANS)
            .map(|n| CoolingChannel {
                name: format!("fan{n}"),
                kind: ChannelKind::Fan,
                limits: FAN_LIMITS,
                internal_profile: false,
            })
            .collect()
    }

    fn lighting_channels(&self) -> Vec<LightingChannel> {
        // Lista e ordem do menu do CAM 4.76.8 para a Fita do Smart Device.
        vec![LightingChannel {
            name: "led".into(),
            capabilities: vec![
                cap(Mode::Fixed, 1, 1, false, false),
                cap(Mode::Breathing, 1, 8, true, false),
                cap(Mode::Fading, 2, 8, true, false),
                cap(Mode::Marquee, 1, 1, true, true),
                cap(Mode::CoveringMarquee, 1, 8, true, true),
                cap(Mode::Pulse, 1, 8, true, false),
                cap(Mode::SpectrumWave, 0, 0, true, true),
                cap(Mode::Alternating, 2, 2, true, true),
                cap(Mode::Candle, 1, 1, false, false),
                cap(Mode::Wings, 1, 1, true, false),
            ],
            leds: 40,
        }]
    }

    fn initialize(&mut self) -> Result<()> {
        for report in protocol::smart_init() {
            self.transport.write(&report)?;
        }
        if !self.collect(READ_TIMEOUT)? {
            bail!("O Smart Device não enviou relatório de status após a inicialização");
        }
        Ok(())
    }

    fn status(&mut self) -> Result<Status> {
        if !self.collect(Duration::ZERO)? && !self.collect(READ_TIMEOUT)? {
            bail!("O Smart Device não enviou relatório de status");
        }
        let noise: Vec<f64> = self
            .fans
            .iter()
            .flatten()
            .map(|f| f64::from(f.noise))
            .collect();
        Ok(Status {
            liquid_temperature: None,
            fans: (0..FANS)
                .map(|i| FanStatus {
                    channel: format!("fan{}", i + 1),
                    rpm: self.fans[usize::from(i)].as_ref().map(|f| u32::from(f.rpm)),
                    duty: None,
                })
                .collect(),
            noise_db: (!noise.is_empty()).then(|| noise.iter().sum::<f64>() / noise.len() as f64),
            firmware: self
                .fans
                .iter()
                .flatten()
                .next()
                .map(|f| format!("{}.{}", f.firmware.0, f.firmware.1)),
        })
    }

    fn set_profile(&mut self, _: &str, _: &[Point]) -> Result<()> {
        bail!("O Smart Device V1 não guarda curvas; o aplicativo controla a rotação")
    }

    fn set_duty(&mut self, channel: &str, duty: u8) -> Result<()> {
        let index = Self::index(channel)?;
        self.transport
            .write(&protocol::fixed_speed(index, FAN_LIMITS.clamp(duty)))
    }

    fn set_lighting(&mut self, channel: &str, lighting: &ChannelLighting) -> Result<()> {
        if channel != "led" {
            bail!("Canal de iluminação {channel} não existe no Smart Device");
        }
        for report in protocol::smart_lighting(lighting)? {
            self.transport.write(&report)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{Kind, Mock};

    fn hex(text: &str) -> Vec<u8> {
        text.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// Relatórios reais do Smart Device V1 do usuário (Windows, 12/11/2026).
    const REAL: [&str; 3] = [
        "04 52 00 05 1a 00 00 09 55 00 0d 01 00 00 07 11 01 02 00 1e 00",
        "04 58 00 05 3d 00 00 09 63 00 0f 01 00 00 07 21 00 02 00 1e 00",
        "04 56 00 00 00 00 00 0c 12 00 00 01 00 00 07 00 00 02 00 1e 00",
    ];

    fn device(mock: &Mock) -> SmartDevice {
        let info = Info {
            kind: Kind::SmartDevice,
            id: "smart_device:876F27655052".into(),
            node: "/dev/hidraw1".into(),
            serial: "876F27655052".into(),
            phys: String::new(),
        };
        SmartDevice::new(info, Box::new(mock.clone()))
    }

    #[test]
    fn initialize_sends_detect_and_start_then_reads() {
        let mock = Mock::default();
        for report in REAL {
            mock.queue(&hex(report));
        }
        let mut sd = device(&mock);
        sd.initialize().unwrap();
        let writes = mock.written();
        assert_eq!(writes.len(), 2);
        assert_eq!(&writes[0][..2], &[0x01, 0x5c]);
        assert_eq!(&writes[1][..2], &[0x01, 0x5d]);
        // O dispositivo envia relatórios continuamente; um novo chega antes do ciclo.
        mock.queue(&hex(REAL[0]));
        let status = sd.status().unwrap();
        let rpm: Vec<_> = status.fans.iter().map(|f| f.rpm).collect();
        assert_eq!(rpm, vec![Some(0), Some(1306), Some(1341)]);
        assert!((status.noise_db.unwrap() - 256.0 / 3.0).abs() < 1e-9);
        assert_eq!(status.firmware.as_deref(), Some("1.7"));
    }

    #[test]
    fn initialize_without_reports_fails() {
        let mock = Mock::default();
        assert!(device(&mock).initialize().is_err());
    }

    #[test]
    fn duty_uses_zero_based_channel_and_floor() {
        let mock = Mock::default();
        let mut sd = device(&mock);
        sd.set_duty("fan3", 10).unwrap();
        assert_eq!(&mock.written()[0][..5], &[0x02, 0x4d, 0x02, 0x00, 25]);
        sd.set_duty("fan1", 80).unwrap();
        assert_eq!(&mock.written()[1][..5], &[0x02, 0x4d, 0x00, 0x00, 80]);
        for bad in ["fan0", "fan4", "pump", "fan"] {
            assert!(sd.set_duty(bad, 50).is_err(), "{bad}");
        }
        assert!(sd.set_profile("fan1", &[]).is_err());
        assert_eq!(mock.written().len(), 2);
    }

    #[test]
    fn lighting_modes_are_encodable() {
        let sd = device(&Mock::default());
        for capability in &sd.lighting_channels()[0].capabilities {
            assert!(protocol::smart_mode(capability.mode).is_some());
        }
    }

    #[test]
    fn breathing_accepts_individual_leds_in_one_animation_step() {
        let mock = Mock::default();
        let mut sd = device(&mock);
        let lighting = ChannelLighting {
            mode: Mode::Breathing,
            per_led: true,
            led_colors: (0..40)
                .map(|index| crate::lighting::Rgb(index, 20, 30))
                .collect(),
            ..Default::default()
        };
        lighting
            .validate(&sd.lighting_channels()[0].capabilities)
            .unwrap();
        sd.set_lighting("led", &lighting).unwrap();
        let writes = mock.written();
        assert_eq!(writes.len(), 2);
        assert_eq!(&writes[0][..5], &[0x02, 0x4b, 0x07, 0, 2]);
        assert_eq!(&writes[0][5..11], &[20, 0, 30, 20, 1, 30]);
        assert_eq!(&writes[1][..4], &[0x03, 20, 19, 30]);
        let mut unsupported = lighting;
        unsupported.mode = Mode::Fading;
        unsupported.colors.push(crate::lighting::Rgb::default());
        assert!(
            unsupported
                .validate(&sd.lighting_channels()[0].capabilities)
                .is_err()
        );
    }
}
