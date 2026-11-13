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
    /// Cor calculada pelo aplicativo a partir da temperatura (não existe no firmware).
    CpuTemperature,
    GpuTemperature,
}

impl Mode {
    pub fn is_temperature(self) -> bool {
        matches!(self, Self::CpuTemperature | Self::GpuTemperature)
    }

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
            Self::CpuTemperature => "CPU Temperature",
            Self::GpuTemperature => "GPU Temperature",
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
    /// Limites dos modos por temperatura: acima de cada um vale a sua cor.
    pub thresholds: Vec<Threshold>,
}

/// Linha "é superior a" do editor de temperatura do CAM.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Threshold {
    pub temperature: f64,
    pub color: Rgb,
}

/// Limites padrão observados no CAM 4.76.8 (Logótipo em Temperatura do CPU).
pub fn default_thresholds() -> Vec<Threshold> {
    [
        (0.0, Rgb(0x00, 0xff, 0x00)),
        (65.0, Rgb(0x84, 0xff, 0x00)),
        (70.0, Rgb(0xba, 0xff, 0x00)),
        (75.0, Rgb(0xff, 0xff, 0x00)),
        (80.0, Rgb(0xff, 0xae, 0x00)),
        (85.0, Rgb(0xff, 0x48, 0x00)),
        (90.0, Rgb(0xff, 0x00, 0x00)),
    ]
    .into_iter()
    .map(|(temperature, color)| Threshold { temperature, color })
    .collect()
}

/// Cor da faixa: o último limite que a temperatura supera; abaixo de todos, o primeiro.
pub fn threshold_color(thresholds: &[Threshold], temperature: f64) -> Option<Rgb> {
    let first = thresholds.first()?;
    Some(
        thresholds
            .iter()
            .rev()
            .find(|t| temperature > t.temperature)
            .unwrap_or(first)
            .color,
    )
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
            thresholds: default_thresholds(),
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
        if self.mode.is_temperature() {
            if self.thresholds.is_empty() || self.thresholds.len() > 10 {
                bail!("O modo por temperatura aceita de 1 a 10 limites");
            }
            for pair in self.thresholds.windows(2) {
                if pair[1].temperature <= pair[0].temperature {
                    bail!("Os limites de temperatura devem ser crescentes");
                }
            }
            if self
                .thresholds
                .iter()
                .any(|t| !t.temperature.is_finite() || !(0.0..=120.0).contains(&t.temperature))
            {
                bail!("Limite de temperatura deve estar entre 0 e 120 °C");
            }
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

impl ChannelLighting {
    /// Iluminação a enviar ao dispositivo. Modos por temperatura viram cor fixa da
    /// faixa atual; sem leitura do sensor, `None` mantém a cor já enviada.
    pub fn resolved(&self, cpu: Option<f64>, gpu: Option<f64>) -> Option<ChannelLighting> {
        let temperature = match self.mode {
            Mode::CpuTemperature => cpu?,
            Mode::GpuTemperature => gpu?,
            _ => return Some(self.clone()),
        };
        let color = threshold_color(&self.thresholds, temperature)?;
        Some(ChannelLighting {
            mode: Mode::Fixed,
            colors: vec![color],
            per_led: false,
            ..self.clone()
        })
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
    fn temperature_modes_pick_threshold_band() {
        let thresholds = default_thresholds();
        assert_eq!(
            threshold_color(&thresholds, 40.0),
            Some(Rgb(0x00, 0xff, 0x00))
        );
        assert_eq!(
            threshold_color(&thresholds, 65.0),
            Some(Rgb(0x00, 0xff, 0x00))
        );
        assert_eq!(
            threshold_color(&thresholds, 65.5),
            Some(Rgb(0x84, 0xff, 0x00))
        );
        assert_eq!(
            threshold_color(&thresholds, 95.0),
            Some(Rgb(0xff, 0x00, 0x00))
        );
        assert_eq!(threshold_color(&[], 50.0), None);
        let lighting = ChannelLighting {
            mode: Mode::CpuTemperature,
            brightness: 40,
            ..Default::default()
        };
        let resolved = lighting.resolved(Some(77.0), None).unwrap();
        assert_eq!(resolved.mode, Mode::Fixed);
        assert_eq!(resolved.colors, vec![Rgb(0xff, 0xff, 0x00)]);
        assert_eq!(resolved.brightness, 40);
        assert!(lighting.resolved(None, Some(50.0)).is_none());
        let gpu = ChannelLighting {
            mode: Mode::GpuTemperature,
            ..Default::default()
        };
        assert_eq!(
            gpu.resolved(None, Some(91.0)).unwrap().colors,
            vec![Rgb(0xff, 0, 0)]
        );
        assert_eq!(
            ChannelLighting::default().resolved(None, None),
            Some(ChannelLighting::default())
        );
    }

    #[test]
    fn validates_temperature_thresholds() {
        let caps = vec![Capability {
            mode: Mode::CpuTemperature,
            min_colors: 0,
            max_colors: 0,
            speed: false,
            direction: false,
            per_led: false,
        }];
        let mut lighting = ChannelLighting {
            mode: Mode::CpuTemperature,
            colors: vec![],
            ..Default::default()
        };
        lighting.validate(&caps).unwrap();
        lighting.thresholds.swap(1, 2);
        assert!(lighting.validate(&caps).is_err());
        lighting.thresholds = vec![];
        assert!(lighting.validate(&caps).is_err());
        lighting.thresholds = vec![Threshold {
            temperature: f64::NAN,
            color: Rgb::default(),
        }];
        assert!(lighting.validate(&caps).is_err());
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
