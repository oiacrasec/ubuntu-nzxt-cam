use crate::{
    config::GovernorConfig,
    curves::{self, ChannelCooling, Point, Source},
    driver::CoolingChannel,
    sensors::Readings,
};
use serde::{Deserialize, Serialize};

/// Duty usado quando a fonte de temperatura falha.
pub const SAFETY_DUTY: u8 = 100;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Gravar a curva no firmware (canal com perfil interno e fonte líquido).
    Profile(Vec<Point>),
    /// Gravar uma rotação calculada pelo aplicativo.
    Duty(u8),
    Keep,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelState {
    pub applied_duty: Option<u8>,
    pub applied_profile: Option<Vec<(f64, u8)>>,
    /// Motivo de segurança ativo, exibido no snapshot.
    pub safety: Option<String>,
    pub error: Option<String>,
}

pub struct Inputs<'a> {
    pub readings: &'a Readings,
    pub liquid: Option<f64>,
    pub cpu_average: bool,
    pub config: &'a GovernorConfig,
}

fn source_temperature(source: Source, inputs: &Inputs) -> Option<f64> {
    match source {
        Source::Liquid => inputs.liquid,
        Source::Cpu => inputs.readings.cpu_temperature(inputs.cpu_average),
        Source::Gpu => inputs.readings.gpu_temperature(inputs.config.gpu_index),
    }
}

/// Decide a próxima escrita de um canal. Não acessa hardware.
pub fn decide(
    channel: &CoolingChannel,
    cooling: &ChannelCooling,
    state: &mut ChannelState,
    inputs: &Inputs,
) -> Action {
    let points = match cooling.points(channel.limits) {
        Ok(points) => points,
        Err(e) => {
            // Configuração inválida nunca chega ao dispositivo: canal em segurança.
            state.error = Some(e.to_string());
            state.safety = Some("perfil inválido".into());
            return duty_action(state, SAFETY_DUTY);
        }
    };
    state.error = None;
    if channel.internal_profile && cooling.source == Source::Liquid {
        state.safety = None;
        let key: Vec<(f64, u8)> = points.iter().map(|p| (p.temperature, p.duty)).collect();
        if state.applied_profile.as_ref() == Some(&key) {
            return Action::Keep;
        }
        return Action::Profile(points);
    }
    let Some(temperature) = source_temperature(cooling.source, inputs) else {
        state.safety = Some("fonte de temperatura indisponível".into());
        return duty_action(state, SAFETY_DUTY);
    };
    state.safety = None;
    let target = curves::duty_at(&points, temperature);
    let previous = if state.applied_profile.is_some() {
        None
    } else {
        state.applied_duty
    };
    let duty = channel
        .limits
        .clamp(curves::smooth(previous, target, inputs.config.fall_step));
    duty_action(state, duty)
}

fn duty_action(state: &ChannelState, duty: u8) -> Action {
    if state.applied_profile.is_none() && state.applied_duty == Some(duty) {
        Action::Keep
    } else {
        Action::Duty(duty)
    }
}

/// Registra o que foi efetivamente escrito.
pub fn applied(state: &mut ChannelState, action: &Action) {
    match action {
        Action::Profile(points) => {
            state.applied_profile = Some(points.iter().map(|p| (p.temperature, p.duty)).collect());
            state.applied_duty = None;
        }
        Action::Duty(duty) => {
            state.applied_duty = Some(*duty);
            state.applied_profile = None;
        }
        Action::Keep => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        curves::{Limits, Profile},
        driver::ChannelKind,
        sensors::Cpu,
    };

    fn channel(internal: bool) -> CoolingChannel {
        CoolingChannel {
            name: "fan".into(),
            kind: ChannelKind::Fan,
            limits: Limits {
                floor: 25,
                ceiling: 100,
            },
            internal_profile: internal,
            liquid_sensor: internal,
        }
    }

    fn readings(cpu: Option<f64>) -> Readings {
        Readings {
            cpu: Cpu {
                package_temperature: cpu,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn run(
        channel: &CoolingChannel,
        cooling: &ChannelCooling,
        state: &mut ChannelState,
        readings: &Readings,
        liquid: Option<f64>,
    ) -> Action {
        let config = GovernorConfig::default();
        let inputs = Inputs {
            readings,
            liquid,
            cpu_average: false,
            config: &config,
        };
        let action = decide(channel, cooling, state, &inputs);
        applied(state, &action);
        action
    }

    #[test]
    fn channel_without_liquid_sensor_defaults_to_cpu_instead_of_safety() {
        // Smart Device V1: sem perfil salvo, o padrão não pode cair numa fonte
        // que o dispositivo não mede.
        let channel = channel(false);
        let cooling = channel.default_cooling();
        assert_eq!(cooling.source, Source::Cpu);
        let mut state = ChannelState::default();
        let action = run(&channel, &cooling, &mut state, &readings(Some(30.0)), None);
        assert!(state.safety.is_none(), "{:?}", state.safety);
        assert!(matches!(action, Action::Duty(duty) if duty < SAFETY_DUTY));
    }

    #[test]
    fn channel_with_liquid_sensor_keeps_liquid_default() {
        assert_eq!(channel(true).default_cooling().source, Source::Liquid);
    }

    #[test]
    fn liquid_source_on_internal_channel_writes_profile_once() {
        let mut state = ChannelState::default();
        let cooling = ChannelCooling::default();
        let first = run(&channel(true), &cooling, &mut state, &readings(None), None);
        assert!(matches!(first, Action::Profile(_)));
        let second = run(&channel(true), &cooling, &mut state, &readings(None), None);
        assert_eq!(second, Action::Keep);
    }

    #[test]
    fn cpu_source_follows_curve_and_smooths_down() {
        let mut state = ChannelState::default();
        let cooling = ChannelCooling {
            profile: Profile::Performance,
            source: Source::Cpu,
        };
        let ch = channel(true);
        assert_eq!(
            run(&ch, &cooling, &mut state, &readings(Some(90.0)), None),
            Action::Duty(100)
        );
        assert_eq!(
            run(&ch, &cooling, &mut state, &readings(Some(20.0)), None),
            Action::Duty(95)
        );
        assert_eq!(
            run(&ch, &cooling, &mut state, &readings(Some(95.0)), None),
            Action::Duty(100)
        );
        assert_eq!(
            run(&ch, &cooling, &mut state, &readings(Some(95.0)), None),
            Action::Keep
        );
    }

    #[test]
    fn missing_source_forces_safety_duty() {
        let mut state = ChannelState::default();
        let cooling = ChannelCooling {
            profile: Profile::Silent,
            source: Source::Gpu,
        };
        assert_eq!(
            run(
                &channel(false),
                &cooling,
                &mut state,
                &readings(Some(40.0)),
                None
            ),
            Action::Duty(SAFETY_DUTY)
        );
        assert!(state.safety.is_some());
        let liquid = ChannelCooling {
            profile: Profile::Silent,
            source: Source::Liquid,
        };
        let mut state = ChannelState::default();
        assert_eq!(
            run(&channel(false), &liquid, &mut state, &readings(None), None),
            Action::Duty(SAFETY_DUTY)
        );
    }

    #[test]
    fn invalid_profile_goes_to_safety_without_reaching_device() {
        let mut state = ChannelState::default();
        let cooling = ChannelCooling {
            profile: Profile::Custom {
                points: vec![Point {
                    temperature: 30.0,
                    duty: 10,
                }],
            },
            source: Source::Cpu,
        };
        let action = run(
            &channel(true),
            &cooling,
            &mut state,
            &readings(Some(30.0)),
            None,
        );
        assert_eq!(action, Action::Duty(SAFETY_DUTY));
        assert!(state.error.is_some());
    }

    #[test]
    fn switching_from_profile_to_duty_writes_again() {
        let mut state = ChannelState::default();
        let ch = channel(true);
        run(
            &ch,
            &ChannelCooling::default(),
            &mut state,
            &readings(None),
            Some(30.0),
        );
        let fixed = ChannelCooling {
            profile: Profile::Fixed { duty: 40 },
            source: Source::Cpu,
        };
        assert_eq!(
            run(&ch, &fixed, &mut state, &readings(Some(30.0)), None),
            Action::Duty(40)
        );
    }
}
