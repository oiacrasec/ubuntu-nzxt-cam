use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Modos de iluminação com os nomes do catálogo `lighting.json` do CAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Off,
    Fixed,
    Fading,
    SpectrumWave,
    Marquee,
    CoveringMarquee,
    Alternating,
    Breathing,
    Pulse,
    TaiChi,
    WaterCooler,
    Loading,
    Wings,
    SuperRainbow,
    RainbowFlow,
    RainbowPulse,
    StarryNight,
    Candle,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Desligado",
            Self::Fixed => "Fixed",
            Self::Fading => "Fading",
            Self::SpectrumWave => "Spectrum Wave",
            Self::Marquee => "Marquee",
            Self::CoveringMarquee => "Covering Marquee",
            Self::Alternating => "Alternating",
            Self::Breathing => "Breathing",
            Self::Pulse => "Pulse",
            Self::TaiChi => "Tai Chi",
            Self::WaterCooler => "Water Cooler",
            Self::Loading => "Loading",
            Self::Wings => "Wings",
            Self::SuperRainbow => "Super Rainbow",
            Self::RainbowFlow => "Rainbow Flow",
            Self::RainbowPulse => "Rainbow Pulse",
            Self::StarryNight => "Starry Night",
            Self::Candle => "Candle",
        }
    }
}

/// Cinco níveis de velocidade do CAM (`modeOption-speedValue-*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    Slowest,
    Slow,
    #[default]
    Normal,
    Fast,
    Fastest,
}

#[cfg_attr(not(feature = "gui"), allow(dead_code))]
impl Speed {
    pub const ALL: [Self; 5] = [
        Self::Slowest,
        Self::Slow,
        Self::Normal,
        Self::Fast,
        Self::Fastest,
    ];

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Slowest => "Slowest",
            Self::Slow => "Slow",
            Self::Normal => "Normal",
            Self::Fast => "Fast",
            Self::Fastest => "Fastest",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelLighting {
    pub mode: Mode,
    pub colors: Vec<Rgb>,
    pub speed: Speed,
    /// `true` inverte a direção da animação.
    pub backwards: bool,
    /// Tamanho do grupo de LEDs em modos de marquee (3 a 6 no CAM).
    pub led_size: u8,
    pub moving: bool,
    /// Brilho de 0 a 100%, aplicado às cores antes do envio.
    pub brightness: u8,
    /// "Personalizar cores LED": uma cor por LED nos modos aceitos pelo canal.
    pub per_led: bool,
    pub led_colors: Vec<Rgb>,
}

impl Default for ChannelLighting {
    fn default() -> Self {
        Self {
            mode: Mode::Fixed,
            colors: vec![Rgb(0x7f, 0x00, 0xff)],
            speed: Speed::Normal,
            backwards: false,
            led_size: 3,
            moving: false,
            brightness: 100,
            per_led: false,
            led_colors: Vec::new(),
        }
    }
}

/// O que um canal físico aceita, declarado pelo driver do dispositivo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    pub mode: Mode,
    pub min_colors: usize,
    pub max_colors: usize,
    pub speed: bool,
    pub direction: bool,
    #[serde(default)]
    pub per_led: bool,
}

impl ChannelLighting {
    pub fn validate(&self, supported: &[Capability]) -> Result<()> {
        let Some(capability) = supported.iter().find(|c| c.mode == self.mode) else {
            bail!("Modo {} não é suportado neste canal", self.mode.label());
        };
        let count = self.colors.len();
        if count < capability.min_colors || count > capability.max_colors {
            bail!(
                "O modo {} aceita de {} a {} cores",
                self.mode.label(),
                capability.min_colors,
                capability.max_colors
            );
        }
        if self.per_led && !capability.per_led {
            bail!("Este modo não aceita cores por LED");
        }
        if self.led_colors.len() > 40 {
            bail!("No máximo 40 cores por LED");
        }
        if self.brightness > 100 {
            bail!("Brilho deve estar entre 0 e 100%");
        }
        if !(3..=6).contains(&self.led_size) {
            bail!("Tamanho do grupo de LEDs deve estar entre 3 e 6");
        }
        if self.backwards && !capability.direction {
            bail!("O modo {} não tem direção", self.mode.label());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> Vec<Capability> {
        vec![
            Capability {
                mode: Mode::Fixed,
                min_colors: 1,
                max_colors: 1,
                speed: false,
                direction: false,
                per_led: true,
            },
            Capability {
                mode: Mode::Breathing,
                min_colors: 1,
                max_colors: 8,
                speed: true,
                direction: false,
                per_led: false,
            },
            Capability {
                mode: Mode::SpectrumWave,
                min_colors: 0,
                max_colors: 0,
                speed: true,
                direction: true,
                per_led: false,
            },
        ]
    }

    #[test]
    fn default_is_valid_fixed_color() {
        ChannelLighting::default().validate(&caps()).unwrap();
    }

    #[test]
    fn rejects_unsupported_mode_and_color_counts() {
        let mut lighting = ChannelLighting {
            mode: Mode::Candle,
            ..Default::default()
        };
        assert!(lighting.validate(&caps()).is_err());
        lighting.mode = Mode::Fixed;
        lighting.colors.clear();
        assert!(lighting.validate(&caps()).is_err());
        lighting.colors = vec![Rgb::default(); 2];
        assert!(lighting.validate(&caps()).is_err());
        lighting.mode = Mode::Breathing;
        lighting.colors = vec![Rgb::default(); 9];
        assert!(lighting.validate(&caps()).is_err());
        lighting.colors.truncate(8);
        lighting.validate(&caps()).unwrap();
    }

    #[test]
    fn checks_direction_and_led_size() {
        let mut lighting = ChannelLighting {
            mode: Mode::SpectrumWave,
            colors: vec![],
            backwards: true,
            ..Default::default()
        };
        lighting.validate(&caps()).unwrap();
        lighting.mode = Mode::Breathing;
        lighting.colors = vec![Rgb(1, 2, 3)];
        assert!(lighting.validate(&caps()).is_err());
        lighting.backwards = false;
        for size in [0, 2, 7, 255] {
            lighting.led_size = size;
            assert!(lighting.validate(&caps()).is_err());
        }
    }

    #[test]
    fn json_round_trip_uses_snake_case() {
        let lighting = ChannelLighting {
            mode: Mode::CoveringMarquee,
            speed: Speed::Fastest,
            ..Default::default()
        };
        let text = serde_json::to_string(&lighting).unwrap();
        assert!(text.contains("\"covering_marquee\""));
        assert!(text.contains("\"fastest\""));
        assert_eq!(
            serde_json::from_str::<ChannelLighting>(&text).unwrap(),
            lighting
        );
        assert!(serde_json::from_str::<ChannelLighting>(r#"{"mode":"laser"}"#).is_err());
    }
}
