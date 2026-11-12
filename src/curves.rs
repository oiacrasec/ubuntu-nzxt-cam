use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Máximo de pontos aceito pelo CAM ("curve length can not over than 64").
pub const MAX_POINTS: usize = 64;

/// Formas dos perfis do CAM 4.76.8 (`silentCurves`/`performanceCurves` nos
/// descritores de canal do renderer). Temperaturas em °C, duty em %.
pub const SILENT: &[(f64, u8)] = &[(0.0, 25), (35.0, 25), (50.0, 55), (55.0, 75), (60.0, 100)];
pub const PERFORMANCE: &[(f64, u8)] = &[(0.0, 50), (35.0, 50), (60.0, 100)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    #[default]
    Liquid,
    Cpu,
    Gpu,
}

impl Source {
    /// Temperatura em que toda curva precisa chegar a 100%.
    pub fn critical(self) -> f64 {
        match self {
            Self::Liquid => 60.0,
            Self::Cpu | Self::Gpu => 90.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub temperature: f64,
    pub duty: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Profile {
    #[default]
    Silent,
    Performance,
    Fixed {
        duty: u8,
    },
    Custom {
        points: Vec<Point>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ChannelCooling {
    pub profile: Profile,
    pub source: Source,
}

/// Limites de um canal físico, definidos pelo driver do dispositivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub floor: u8,
    pub ceiling: u8,
}

impl Limits {
    pub fn clamp(self, duty: u8) -> u8 {
        duty.clamp(self.floor, self.ceiling)
    }
}

fn preset(points: &[(f64, u8)], source: Source) -> Vec<Point> {
    // Os perfis do CAM têm forma definida para o líquido (crítico 60 °C);
    // para CPU/GPU a mesma forma é esticada até a temperatura crítica da fonte.
    let scale = source.critical() / Source::Liquid.critical();
    points
        .iter()
        .map(|&(temperature, duty)| Point {
            temperature: temperature * scale,
            duty,
        })
        .collect()
}

impl ChannelCooling {
    /// Pontos efetivos do perfil, já validados contra os limites do canal.
    pub fn points(&self, limits: Limits) -> Result<Vec<Point>> {
        let points = match &self.profile {
            Profile::Silent => preset(SILENT, self.source),
            Profile::Performance => preset(PERFORMANCE, self.source),
            Profile::Fixed { duty } => {
                if *duty > 100 {
                    bail!("Duty fixo deve estar entre 0 e 100%");
                }
                let duty = limits.clamp(*duty);
                let critical = self.source.critical();
                // Mesmo um valor fixo sobe para 100% na temperatura crítica.
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
            Profile::Custom { points } => points.clone(),
        };
        validate(&points, self.source)?;
        Ok(points
            .into_iter()
            .map(|p| Point {
                temperature: p.temperature,
                duty: limits.clamp(p.duty),
            })
            .collect())
    }
}

pub fn validate(points: &[Point], source: Source) -> Result<()> {
    if points.len() < 2 {
        bail!("A curva precisa de pelo menos dois pontos");
    }
    if points.len() > MAX_POINTS {
        bail!("A curva aceita no máximo {MAX_POINTS} pontos");
    }
    for pair in points.windows(2) {
        if pair[1].temperature <= pair[0].temperature {
            bail!("As temperaturas da curva devem ser crescentes");
        }
        if pair[1].duty < pair[0].duty {
            bail!("A rotação da curva não pode diminuir quando a temperatura sobe");
        }
    }
    for point in points {
        if !point.temperature.is_finite() || !(0.0..=100.0).contains(&point.temperature) {
            bail!("Temperatura da curva deve estar entre 0 e 100 °C");
        }
        if point.duty > 100 {
            bail!("Rotação da curva deve estar entre 0 e 100%");
        }
    }
    if duty_at(points, source.critical()) < 100 {
        bail!("A curva precisa chegar a 100% até {} °C", source.critical());
    }
    Ok(())
}

/// Interpolação linear; fora da curva vale o ponto extremo.
pub fn duty_at(points: &[Point], temperature: f64) -> u8 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return 100;
    };
    if !temperature.is_finite() {
        return 100;
    }
    if temperature <= first.temperature {
        return first.duty;
    }
    if temperature >= last.temperature {
        return last.duty;
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if temperature <= b.temperature {
            let ratio = (temperature - a.temperature) / (b.temperature - a.temperature);
            let duty = f64::from(a.duty) + ratio * (f64::from(b.duty) - f64::from(a.duty));
            return duty.round().clamp(0.0, 100.0) as u8;
        }
    }
    last.duty
}

/// Suaviza a resposta: sobe imediatamente, desce no máximo `step` por ciclo.
pub fn smooth(previous: Option<u8>, target: u8, step: u8) -> u8 {
    match previous {
        Some(previous) if target < previous => target.max(previous.saturating_sub(step)),
        _ => target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAN: Limits = Limits {
        floor: 20,
        ceiling: 100,
    };

    fn custom(points: &[(f64, u8)]) -> ChannelCooling {
        ChannelCooling {
            profile: Profile::Custom {
                points: points
                    .iter()
                    .map(|&(temperature, duty)| Point { temperature, duty })
                    .collect(),
            },
            source: Source::Liquid,
        }
    }

    #[test]
    fn presets_reach_full_speed_at_critical_temperature() {
        for source in [Source::Liquid, Source::Cpu, Source::Gpu] {
            for profile in [Profile::Silent, Profile::Performance] {
                let points = ChannelCooling { profile, source }.points(FAN).unwrap();
                assert_eq!(duty_at(&points, source.critical()), 100);
            }
        }
    }

    #[test]
    fn silent_matches_cam_shape_on_liquid() {
        let points = ChannelCooling::default().points(FAN).unwrap();
        assert_eq!(duty_at(&points, 20.0), 25);
        assert_eq!(duty_at(&points, 50.0), 55);
        assert_eq!(duty_at(&points, 52.5), 65);
        assert_eq!(duty_at(&points, 70.0), 100);
    }

    #[test]
    fn fixed_duty_respects_floor_and_critical_temperature() {
        let cooling = ChannelCooling {
            profile: Profile::Fixed { duty: 0 },
            source: Source::Liquid,
        };
        let points = cooling.points(FAN).unwrap();
        assert_eq!(duty_at(&points, 30.0), 20);
        assert_eq!(duty_at(&points, 60.0), 100);
        let negative: Result<ChannelCooling, _> =
            serde_json::from_str(r#"{"profile":{"type":"fixed","duty":-5}}"#);
        assert!(negative.is_err());
        let cooling = ChannelCooling {
            profile: Profile::Fixed { duty: 101 },
            source: Source::Liquid,
        };
        assert!(cooling.points(FAN).is_err());
    }

    #[test]
    fn rejects_invalid_custom_curves() {
        assert!(custom(&[(30.0, 50)]).points(FAN).is_err());
        assert!(custom(&[(30.0, 50), (30.0, 100)]).points(FAN).is_err());
        assert!(custom(&[(30.0, 80), (60.0, 50)]).points(FAN).is_err());
        assert!(custom(&[(30.0, 50), (60.0, 90)]).points(FAN).is_err());
        assert!(custom(&[(-1.0, 50), (60.0, 100)]).points(FAN).is_err());
        assert!(custom(&[(30.0, 50), (f64::NAN, 100)]).points(FAN).is_err());
        assert!(custom(&[(30.0, 50), (101.0, 100)]).points(FAN).is_err());
        assert!(custom(&[(30.0, 50), (60.0, 101)]).points(FAN).is_err());
        let many: Vec<(f64, u8)> = (0..=MAX_POINTS).map(|i| (i as f64, 100)).collect();
        assert!(custom(&many).points(FAN).is_err());
    }

    #[test]
    fn custom_curve_is_clamped_to_floor() {
        let points = custom(&[(0.0, 0), (60.0, 100)]).points(FAN).unwrap();
        assert_eq!(points[0].duty, 20);
    }

    #[test]
    fn interpolation_handles_extremes() {
        let points = custom(&[(20.0, 30), (60.0, 100)]).points(FAN).unwrap();
        assert_eq!(duty_at(&points, -40.0), 30);
        assert_eq!(duty_at(&points, 0.0), 30);
        assert_eq!(duty_at(&points, 40.0), 65);
        assert_eq!(duty_at(&points, 1000.0), 100);
        assert_eq!(duty_at(&points, f64::NAN), 100);
        assert_eq!(duty_at(&points, f64::INFINITY), 100);
        assert_eq!(duty_at(&[], 30.0), 100);
    }

    #[test]
    fn smoothing_rises_immediately_and_falls_gradually() {
        assert_eq!(smooth(None, 40, 5), 40);
        assert_eq!(smooth(Some(40), 90, 5), 90);
        assert_eq!(smooth(Some(90), 40, 5), 85);
        assert_eq!(smooth(Some(42), 40, 5), 40);
        assert_eq!(smooth(Some(3), 0, 5), 0);
    }
}
