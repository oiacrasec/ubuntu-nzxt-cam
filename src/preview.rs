//! Prévia visual em memória: não cria Engine, IPC, arquivos nem abre dispositivos.
use crate::{
    devices::{Info, Kind, Transport},
    driver::{self, FanStatus, Status},
    engine::{DeviceState, DeviceStatus, Request, SharedState, Snapshot},
    lighting::{ChannelLighting, Mode, Rgb},
    sensors::{Cpu, Disk, Gpu, Memory, Network, Process, Readings},
    specs::{CpuDetails, Specs},
};
use anyhow::{Result, bail};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

type Wake = Arc<Mutex<Option<Box<dyn Fn() + Send>>>>;

pub struct Controller {
    pub tx: Sender<Request>,
    pub shared: SharedState,
    pub wake: Wake,
    thread: Option<JoinHandle<()>>,
}

impl Controller {
    pub fn spawn() -> Self {
        let shared = Arc::new(Mutex::new(snapshot()));
        let wake: Wake = Arc::default();
        let (tx, rx) = mpsc::channel();
        let worker_shared = shared.clone();
        let worker_wake = wake.clone();
        let thread = thread::spawn(move || {
            while let Ok(request) = rx.recv() {
                let quit = {
                    let mut state = worker_shared.lock().unwrap_or_else(|e| e.into_inner());
                    state.error = apply(&mut state, request).err().map(|e| e.to_string());
                    state.quit
                };
                if let Some(callback) = &*worker_wake.lock().unwrap_or_else(|e| e.into_inner()) {
                    callback();
                }
                if quit {
                    break;
                }
            }
        });
        Self {
            tx,
            shared,
            wake,
            thread: Some(thread),
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Os drivers só fornecem os descritores de canais. Qualquer E/S é recusada.
struct NoIo;
impl Transport for NoIo {
    fn write(&mut self, _: &[u8]) -> Result<()> {
        bail!("Prévia sem acesso a hardware")
    }
    fn read(&mut self, _: &mut [u8], _: Duration) -> Result<usize> {
        bail!("Prévia sem acesso a hardware")
    }
}

fn snapshot() -> Snapshot {
    let mut state = Snapshot {
        programs: vec![
            crate::profiles::Program {
                name: "Steam".into(),
                executable: "/usr/bin/steam".into(),
            },
            crate::profiles::Program {
                name: "Firefox".into(),
                executable: "/usr/bin/firefox".into(),
            },
            crate::profiles::Program {
                name: "Blender".into(),
                executable: "/usr/bin/blender".into(),
            },
        ],
        local_time: Some(crate::profiles::LocalTime {
            day: 5,
            minute: 600,
        }),
        languages: vec!["pt-BR".into(), "en-US".into()],
        sensors: Readings {
            cpu: Cpu {
                name: "Intel Core i9-9900K".into(),
                package_temperature: Some(48.0),
                average_temperature: Some(45.0),
                load_percent: Some(12.0),
                clock_mhz: Some(4700.0),
            },
            gpus: vec![Gpu {
                name: "NVIDIA GeForce RTX 4070 SUPER".into(),
                temperature: Some(42.0),
                load_percent: Some(6.0),
                clock_mhz: Some(2100.0),
                fan_percent: Some(30.0),
            }],
            memory: Some(Memory {
                total_bytes: 32 << 30,
                used_bytes: 14 << 30,
            }),
            network: Network {
                receive_bytes_per_second: Some(250_000.0),
                transmit_bytes_per_second: Some(32_000.0),
            },
            disks: vec![Disk {
                mount: "/".into(),
                total_bytes: 512_000_000_000,
                used_bytes: 280_000_000_000,
            }],
            processes: vec![Process {
                name: "Exemplo de processo".into(),
                cpu_percent: 4.2,
                ram_bytes: 420 << 20,
                gpu_percent: Some(2.0),
                download_bytes_per_second: Some(12_000.0),
                upload_bytes_per_second: Some(2_000.0),
            }],
            ..Default::default()
        },
        specs: Specs {
            cpu: CpuDetails {
                name: "Intel Core i9-9900K".into(),
                vendor: "Intel".into(),
                cores: 8,
                threads: 16,
                base_mhz: Some(3600.0),
                max_mhz: Some(5000.0),
            },
            motherboard: Some("ASUS ROG MAXIMUS XI HERO".into()),
            memory_total_bytes: Some(32 << 30),
            storage: vec!["SSD NVMe (simulado)".into()],
            audio: vec!["Sound Blaster AE-5 Plus (simulada)".into()],
            network: vec!["Ethernet (simulada)".into()],
            memory_modules: vec!["G.Skill F4-3200C16-8GTZR DDR4 (simulada)".into(); 4],
            details: Default::default(),
        },
        message: "Prévia — dados simulados; alterações apenas nesta janela".into(),
        ..Default::default()
    };
    let fields = |items: &[(&str, &str)]| {
        items
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect()
    };
    state.specs.details.insert(
        "cpu".into(),
        fields(&[
            ("Fabricante", "Intel"),
            ("Nome do código", "Coffee Lake"),
            ("Velocidade padrão do clock", "3600 MHz"),
            ("Tipo de soquete", "Socket 1151 LGA"),
            ("TDP", "95 W"),
        ]),
    );
    state.specs.details.insert(
        "board".into(),
        fields(&[
            ("Modelo", "ROG MAXIMUS XI HERO"),
            ("Chipset", "Coffee Lake"),
            ("Southbridge", "Z390"),
            ("Versão do BIOS", "0805"),
            ("Data do BIOS", "01/29/2019"),
        ]),
    );
    for i in 0..4 {
        state.specs.details.insert(
            format!("ram:{i}"),
            fields(&[
                ("Modelo", "G.Skill F4-3200C16-8GTZR"),
                ("Capacidade", "8.0 GB"),
                ("Frequência padrão", "1066 MHz"),
                ("Tipo", "DDR4"),
                ("Tempos", "15-15-15-36"),
            ]),
        );
    }
    state.specs.details.insert(
        "storage:0".into(),
        fields(&[
            ("Nome", "SSD NVMe (simulado)"),
            ("Fabricante", "Exemplo"),
            ("Formato", "ext4"),
            ("Capacidade", "512 GB"),
        ]),
    );
    state.specs.details.insert(
        "audio:0".into(),
        fields(&[
            ("Nome", "Sound Blaster AE-5 Plus (simulada)"),
            ("Formato", "Speakers"),
            ("Número de canais", "5.1"),
        ]),
    );
    state.specs.details.insert(
        "network:0".into(),
        fields(&[
            ("Nome do adaptador", "Ethernet (simulada)"),
            ("Tipo de conexão", "Ethernet"),
            ("Endereço IP", "192.0.2.10"),
        ]),
    );
    for (name, cpu, gpu, ram) in [
        ("Editor (simulado)", 8.0, 1.0, 1200),
        ("Navegador (simulado)", 2.0, 3.0, 850),
    ] {
        state.sensors.processes.push(Process {
            name: name.into(),
            cpu_percent: cpu,
            gpu_percent: Some(gpu),
            ram_bytes: ram * 1_000_000,
            download_bytes_per_second: Some(0.0),
            upload_bytes_per_second: Some(0.0),
        });
    }
    for kind in Kind::ALL {
        let id = format!("{}:preview", kind.slug());
        let info = Info {
            kind,
            id: id.clone(),
            node: Default::default(),
            serial: "preview".into(),
            phys: String::new(),
        };
        let driver = driver::open(info.clone(), Box::new(NoIo));
        let cooling = driver.cooling_channels();
        for channel in &cooling {
            state.config.cooling.insert(
                format!("{id}/{}", channel.name),
                crate::curves::ChannelCooling {
                    source: if kind == Kind::SmartDevice {
                        crate::curves::Source::Cpu
                    } else {
                        crate::curves::Source::Liquid
                    },
                    ..Default::default()
                },
            );
        }
        let lighting = driver.lighting_channels();
        for channel in &lighting {
            state.config.lighting.insert(
                format!("{id}/{}", channel.name),
                ChannelLighting {
                    mode: if kind == Kind::SmartDevice {
                        Mode::Breathing
                    } else {
                        Mode::Fixed
                    },
                    colors: vec![Rgb(0x51, 0, 0x7a)],
                    ..Default::default()
                },
            );
        }
        let fans = cooling
            .iter()
            .map(|channel| FanStatus {
                channel: channel.name.clone(),
                rpm: Some(if channel.name == "pump" { 2034 } else { 1300 }),
                duty: Some(50),
            })
            .collect();
        state.devices.push(DeviceState {
            info,
            name: kind.display_name().into(),
            status: DeviceStatus::Ready,
            error: None,
            cooling,
            lighting,
            channels: Default::default(),
            last_status: Some(Status {
                liquid_temperature: (kind == Kind::KrakenX).then_some(33.1),
                fans,
                noise_db: (kind == Kind::SmartDevice).then_some(32.0),
                firmware: Some("Simulado".into()),
            }),
        });
    }
    crate::profiles::initialize(&mut state.config);
    state
}

fn apply(state: &mut Snapshot, request: Request) -> Result<()> {
    match request {
        Request::CheckUpdates => {
            state.updates = crate::updater::State {
                message: "Prévia: consulta de atualizações simulada, sem acesso à rede".into(),
                phase: crate::updater::Phase::Available,
                installed: true,
                candidate: Some(crate::updater::Candidate {
                    branch: "main".into(),
                    revision: "a".repeat(40),
                }),
                ..Default::default()
            }
        }
        Request::InstallUpdate => {
            state.updates.phase = crate::updater::Phase::RestartRequired;
            state.updates.message =
                "Prévia: instalação simulada; nenhum arquivo foi alterado".into();
        }
        Request::RestartAfterUpdate => {
            state.updates.phase = crate::updater::Phase::Current;
            state.updates.message = "Prévia: reinício simulado".into();
        }
        Request::SaveProfileAndActivate { name, profile } => {
            let mut next = state.clone();
            crate::profiles::edit(
                &mut next.config,
                crate::profiles::Edit::Save {
                    name: name.clone(),
                    profile,
                },
            )?;
            next.config = crate::profiles::prepare(&next, &name)?;
            *state = next;
        }
        Request::EditProfile { edit } => {
            let was_active = matches!(&edit, crate::profiles::Edit::Delete { name } if state.config.profiles.active.as_ref() == Some(name));
            let mut next = state.clone();
            crate::profiles::edit(&mut next.config, edit)?;
            if was_active {
                next.config = crate::profiles::prepare(&next, "Default")?;
            }
            *state = next;
        }
        Request::RefreshPrograms => {}
        Request::BrowsePrograms { .. } => {
            state.program_browser = crate::profiles::ProgramBrowser {
                directory: "/usr/bin".into(),
                entries: state
                    .programs
                    .iter()
                    .map(|program| crate::profiles::ProgramEntry {
                        path: (&program.executable).into(),
                        directory: false,
                    })
                    .collect(),
            };
        }
        Request::ActivateProfile { name } => state.config = crate::profiles::prepare(state, &name)?,
        Request::Quit => state.quit = true,
        Request::Show => state.show_requests += 1,
        Request::Status | Request::Refresh | Request::CancelPlan => {}
        Request::SetLighting { channel, lighting } => {
            let physical = state
                .devices
                .iter()
                .find_map(|device| {
                    device
                        .lighting
                        .iter()
                        .find(|physical| format!("{}/{}", device.info.id, physical.name) == channel)
                })
                .ok_or_else(|| anyhow::anyhow!("Canal de iluminação não encontrado"))?;
            lighting.validate(&physical.capabilities)?;
            state.config.lighting.insert(channel, lighting);
            crate::profiles::remember_channels(&mut state.config, true);
        }
        Request::SetCooling { channel, cooling } => {
            let physical = state
                .devices
                .iter()
                .find_map(|device| {
                    device
                        .cooling
                        .iter()
                        .find(|physical| format!("{}/{}", device.info.id, physical.name) == channel)
                })
                .ok_or_else(|| anyhow::anyhow!("Canal de refrigeração não encontrado"))?;
            cooling.points(physical.limits)?;
            state.config.cooling.insert(channel, cooling);
            crate::profiles::remember_channels(&mut state.config, false);
        }
        Request::UpdateUi { ui } => {
            state.config.ui = ui;
        }
        Request::UpdateSettings {
            language,
            temperature_unit,
            cpu_average_temperature,
            start_minimized,
            mini_mode,
            mini_always_on_top,
            gpu_index,
            close_to_tray,
        } => {
            let ui = &mut state.config.ui;
            if let Some(value) = language {
                ui.language = value;
            }
            if let Some(value) = temperature_unit {
                ui.temperature_unit = value;
            }
            if let Some(value) = cpu_average_temperature {
                ui.cpu_average_temperature = value;
            }
            if let Some(value) = start_minimized {
                ui.start_minimized = value;
            }
            if let Some(value) = mini_mode {
                ui.mini_mode = value;
            }
            if let Some(value) = mini_always_on_top {
                ui.mini_always_on_top = value;
            }
            if let Some(value) = close_to_tray {
                ui.close_to_tray = value;
            }
            if let Some(value) = gpu_index {
                state.config.governor.gpu_index = value;
            }
        }
        Request::ExportConfig { .. }
        | Request::ImportConfig { .. }
        | Request::SetAutostart { .. }
        | Request::PreparePermissions { .. }
        | Request::ConfirmPlan { .. } => {
            bail!("Esta ação não está disponível na prévia visual");
        }
    }
    // Os catálogos são embutidos; o caminho vazio não acessa a configuração real.
    crate::i18n::activate(crate::i18n::load(
        &state.config.ui.language,
        std::path::Path::new(""),
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_only_simulated_state_and_rejects_system_actions() {
        let mut state = snapshot();
        assert_eq!(state.devices.len(), 2);
        assert_eq!(
            state
                .devices
                .iter()
                .map(|d| d.lighting.len())
                .sum::<usize>(),
            3
        );
        let key = "kraken_x:preview/ring".to_owned();
        let mut lighting = state.config.lighting[&key].clone();
        lighting.colors = vec![Rgb(1, 2, 3)];
        apply(
            &mut state,
            Request::SetLighting {
                channel: key.clone(),
                lighting: lighting.clone(),
            },
        )
        .unwrap();
        assert_eq!(state.config.lighting[&key], lighting);
        assert!(apply(&mut state, Request::SetAutostart { enabled: true }).is_err());
        assert!(apply(&mut state, Request::ConfirmPlan { id: 1 }).is_err());
        assert!(
            apply(
                &mut state,
                Request::ExportConfig {
                    path: "must-not-exist.json".into()
                }
            )
            .is_err()
        );
        assert!(!state.autostart);
        let mut transport = NoIo;
        assert!(transport.write(&[0; 65]).is_err());
        assert!(transport.read(&mut [0; 65], Duration::ZERO).is_err());
    }
}
