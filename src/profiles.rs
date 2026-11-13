//! Perfis nomeados e seleção automática; sem acesso aos dispositivos.
use crate::{config::Config, curves::ChannelCooling, lighting::ChannelLighting};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutoLaunch {
    #[default]
    None,
    Program {
        executables: Vec<String>,
    },
    /// Minutos desde meia-noite no horário local, intervalo [início, fim).
    Schedule {
        start: u16,
        end: u16,
        /// Domingo primeiro, como no CAM. O dia identifica o início do intervalo.
        #[serde(default = "all_days")]
        days: [bool; 7],
    },
}

pub fn all_days() -> [bool; 7] {
    [true; 7]
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LocalTime {
    pub day: usize,
    pub minute: u16,
}

pub fn local_time() -> Option<LocalTime> {
    let output = std::process::Command::new("date")
        .arg("+%w:%H:%M")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let values: Vec<u16> = text
        .trim()
        .split(':')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match values.as_slice() {
        [day, hour, minute] if *day < 7 && *hour < 24 && *minute < 60 => Some(LocalTime {
            day: *day as usize,
            minute: hour * 60 + minute,
        }),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncProfile {
    pub lighting: String,
    pub cooling: String,
    pub auto_launch: AutoLaunch,
}

impl Default for SyncProfile {
    fn default() -> Self {
        Self {
            lighting: "Padrão".into(),
            cooling: "Padrão".into(),
            auto_launch: AutoLaunch::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profiles {
    pub active: Option<String>,
    pub sync: BTreeMap<String, SyncProfile>,
    pub lighting: BTreeMap<String, BTreeMap<String, ChannelLighting>>,
    pub cooling: BTreeMap<String, BTreeMap<String, ChannelCooling>>,
    pub initialized: bool,
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            active: Some("Default".into()),
            sync: BTreeMap::from([("Default".into(), SyncProfile::default())]),
            lighting: BTreeMap::from([("Padrão".into(), BTreeMap::new())]),
            cooling: BTreeMap::from([("Padrão".into(), BTreeMap::new())]),
            initialized: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Edit {
    CaptureLighting { name: String },
    CaptureCooling { name: String },
    Save { name: String, profile: SyncProfile },
    Rename { name: String, new_name: String },
    AddLighting { name: String, source: String },
    AddCooling { name: String, source: String },
    Delete { name: String },
}

fn valid_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 50 || name.chars().any(char::is_control) {
        bail!("Nome de perfil inválido");
    }
    Ok(())
}

impl Profiles {
    pub fn validate(&self) -> Result<()> {
        if self.sync.len() > 100 || self.lighting.len() > 100 || self.cooling.len() > 100 {
            bail!("No máximo 100 perfis por categoria");
        }
        if !self.sync.contains_key("Default") {
            bail!("O perfil Default é obrigatório");
        }
        for name in self.lighting.keys().chain(self.cooling.keys()) {
            valid_name(name)?;
        }
        if let Some(active) = &self.active {
            self.sync.get(active).context("Perfil ativo inexistente")?;
        }
        for (name, profile) in &self.sync {
            valid_name(name)?;
            self.lighting
                .get(&profile.lighting)
                .context("Perfil de iluminação inexistente")?;
            self.cooling
                .get(&profile.cooling)
                .context("Perfil de refrigeração inexistente")?;
            match &profile.auto_launch {
                AutoLaunch::None => {}
                _ if name == "Default" => bail!("Default não aceita ativação automática"),
                AutoLaunch::Program { executables } => {
                    if executables.len() > 100 {
                        bail!("No máximo 100 programas por perfil");
                    }
                    for executable in executables {
                        if executable.trim().is_empty()
                            || executable.len() > 4096
                            || executable.chars().any(char::is_control)
                        {
                            bail!("Executável inválido");
                        }
                    }
                }
                AutoLaunch::Schedule { start, end, days }
                    if *start >= 1440 || *end >= 1440 || !days.contains(&true) =>
                {
                    bail!("Horário de ativação inválido");
                }
                AutoLaunch::Schedule { .. } => {}
            }
        }
        Ok(())
    }

    pub fn matching(
        &self,
        programs: &BTreeSet<String>,
        time: Option<LocalTime>,
    ) -> BTreeSet<String> {
        self.sync
            .iter()
            .filter_map(|(name, profile)| {
                let matches = match &profile.auto_launch {
                    AutoLaunch::None => false,
                    AutoLaunch::Program { executables } => {
                        executables.iter().any(|exe| programs.contains(exe))
                    }
                    AutoLaunch::Schedule { start, end, days } => time
                        .filter(|t| t.day < 7 && t.minute < 1440)
                        .is_some_and(|time| {
                            if *start == 0 && *end == 0 {
                                days[time.day]
                            } else if start <= end {
                                days[time.day] && time.minute >= *start && time.minute < *end
                            } else {
                                (days[time.day] && time.minute >= *start)
                                    || (days[(time.day + 6) % 7] && time.minute < *end)
                            }
                        }),
                };
                matches.then(|| name.clone())
            })
            .collect()
    }
}

pub fn initialize(config: &mut Config) {
    if !config.profiles.initialized {
        config
            .profiles
            .lighting
            .insert("Padrão".into(), config.lighting.clone());
        config
            .profiles
            .cooling
            .insert("Padrão".into(), config.cooling.clone());
        config.profiles.initialized = true;
    }
}

/// As páginas de dispositivo editam o perfil associado ao mestre selecionado.
pub fn remember_channels(config: &mut Config, lighting: bool) {
    let Some(profile) = config
        .profiles
        .active
        .as_ref()
        .and_then(|name| config.profiles.sync.get(name))
    else {
        return;
    };
    if lighting {
        config
            .profiles
            .lighting
            .insert(profile.lighting.clone(), config.lighting.clone());
    } else {
        config
            .profiles
            .cooling
            .insert(profile.cooling.clone(), config.cooling.clone());
    }
}

pub fn edit(config: &mut Config, edit: Edit) -> Result<()> {
    let mut next = config.clone();
    initialize(&mut next);
    match edit {
        Edit::CaptureLighting { name } => {
            valid_name(&name)?;
            next.profiles.lighting.insert(name, next.lighting.clone());
        }
        Edit::CaptureCooling { name } => {
            valid_name(&name)?;
            next.profiles.cooling.insert(name, next.cooling.clone());
        }
        Edit::Save { name, profile } => {
            valid_name(&name)?;
            next.profiles.sync.insert(name, profile);
        }
        Edit::Rename { name, new_name } => {
            if name == "Default" {
                bail!("Default não pode ser renomeado");
            }
            valid_name(&new_name)?;
            if new_name != name && next.profiles.sync.contains_key(&new_name) {
                bail!("Já existe um perfil com esse nome");
            }
            let profile = next
                .profiles
                .sync
                .remove(&name)
                .context("Perfil inexistente")?;
            next.profiles.sync.insert(new_name.clone(), profile);
            if next.profiles.active.as_ref() == Some(&name) {
                next.profiles.active = Some(new_name);
            }
        }
        Edit::AddLighting { name, source } => {
            valid_name(&name)?;
            if next.profiles.lighting.contains_key(&name) {
                bail!("Já existe um perfil com esse nome");
            }
            let value = next
                .profiles
                .lighting
                .get(&source)
                .context("Perfil de origem inexistente")?
                .clone();
            next.profiles.lighting.insert(name, value);
        }
        Edit::AddCooling { name, source } => {
            valid_name(&name)?;
            if next.profiles.cooling.contains_key(&name) {
                bail!("Já existe um perfil com esse nome");
            }
            let value = next
                .profiles
                .cooling
                .get(&source)
                .context("Perfil de origem inexistente")?
                .clone();
            next.profiles.cooling.insert(name, value);
        }
        Edit::Delete { name } => {
            if name == "Default" {
                bail!("Default não pode ser removido");
            }
            next.profiles
                .sync
                .remove(&name)
                .context("Perfil inexistente")?;
            if next.profiles.active.as_ref() == Some(&name) {
                next.profiles.active = Some("Default".into());
            }
        }
    }
    next.profiles.validate()?;
    *config = next;
    Ok(())
}

pub fn programs(proc_: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    if let Ok(entries) = fs::read_dir(proc_) {
        for entry in entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().parse::<u32>().is_ok())
        {
            let executable = fs::read_link(entry.path().join("exe")).ok();
            if let Some(path) = &executable {
                names.insert(path.to_string_lossy().into_owned());
            }
            let name = executable
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .or_else(|| {
                    fs::read_to_string(entry.path().join("comm"))
                        .ok()
                        .map(|n| n.trim().to_owned())
                });
            if let Some(name) = name {
                names.insert(name);
            }
        }
    }
    names
}

/// A pilha é transitória. Só é confirmada pelo controlador após aplicar o destino.
#[derive(Clone, Debug, Default)]
pub struct ActivationStack {
    entries: Vec<(String, bool)>,
    satisfied: BTreeSet<String>,
}

impl ActivationStack {
    pub fn manual(&mut self, name: &str) {
        self.entries.retain(|entry| entry != &(name.into(), true));
        self.entries.push((name.into(), true));
    }
    pub fn rename(&mut self, name: &str, new_name: &str) {
        for entry in &mut self.entries {
            if entry.0 == name {
                entry.0 = new_name.into();
            }
        }
        if self.satisfied.remove(name) {
            self.satisfied.insert(new_name.into());
        }
    }
    pub fn reconcile(&self, profiles: &Profiles, matching: BTreeSet<String>) -> Self {
        let mut next = self.clone();
        if next.entries.is_empty() {
            next.manual(profiles.active.as_deref().unwrap_or("Default"));
        }
        next.entries.retain(|(name, manual)| {
            profiles.sync.contains_key(name) && (*manual || matching.contains(name))
        });
        for name in matching.difference(&self.satisfied) {
            next.entries.push((name.clone(), false));
        }
        next.satisfied = matching;
        next
    }
    pub fn target(&self) -> &str {
        self.entries
            .last()
            .map_or("Default", |(name, _)| name.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
    pub executable: String,
}

/// Catálogo local do Ubuntu; não usa o catálogo nem os recursos de jogos da NZXT.
pub fn catalog(proc_: &Path) -> Vec<Program> {
    let mut result = BTreeMap::<String, String>::new();
    let mut roots = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share/applications"));
    }
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten().take(2000) {
            if entry.path().extension().is_none_or(|ext| ext != "desktop") {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let mut active = false;
            let (mut name, mut executable) = (None, None);
            let mut hidden = false;
            for line in text.lines().map(str::trim) {
                if line.starts_with('[') {
                    active = line == "[Desktop Entry]";
                    continue;
                }
                if !active {
                    continue;
                }
                if let Some(value) = line.strip_prefix("Name=") {
                    name = Some(value.to_owned());
                }
                if let Some(value) = line.strip_prefix("Exec=") {
                    executable = desktop_executable(value);
                }
                hidden |= matches!(line, "Hidden=true" | "NoDisplay=true");
            }
            if !hidden && let (Some(name), Some(executable)) = (name, executable) {
                result.insert(executable, name);
            }
        }
    }
    for executable in programs(proc_)
        .into_iter()
        .filter(|name| name.starts_with('/'))
    {
        let name = Path::new(&executable)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        result.entry(executable).or_insert(name);
    }
    let mut result: Vec<_> = result
        .into_iter()
        .map(|(executable, name)| Program { name, executable })
        .collect();
    result.sort_by_key(|program| program.name.to_lowercase());
    result
}

fn desktop_executable(value: &str) -> Option<String> {
    let first = if let Some(quoted) = value.strip_prefix('"') {
        quoted.split('"').next()?
    } else {
        value.split_whitespace().next()?
    };
    if first.is_empty() || first == "env" || first.contains(['%', '$', '`']) {
        return None;
    }
    if Path::new(first).is_absolute() {
        return Some(first.into());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(first))
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProgramBrowser {
    pub directory: PathBuf,
    pub entries: Vec<ProgramEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramEntry {
    pub path: PathBuf,
    pub directory: bool,
}

pub fn browse(directory: PathBuf) -> Result<ProgramBrowser> {
    let directory = directory
        .canonicalize()
        .context("Não foi possível abrir a pasta")?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&directory)?.flatten().take(5000) {
        let path = entry.path();
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = metadata.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
        if metadata.is_dir() || executable {
            entries.push(ProgramEntry {
                path,
                directory: metadata.is_dir(),
            });
        }
    }
    entries.sort_by_key(|entry| (!entry.directory, entry.path.clone()));
    Ok(ProgramBrowser { directory, entries })
}

/// Valida todo o destino antes de permitir qualquer escrita de hardware.
pub fn prepare(state: &crate::engine::Snapshot, name: &str) -> Result<Config> {
    use crate::{curves::Source, devices::Kind, engine::DeviceStatus};
    let profile = state
        .config
        .profiles
        .sync
        .get(name)
        .context("Perfil inexistente")?;
    let mut next = state.config.clone();
    next.lighting = state
        .config
        .profiles
        .lighting
        .get(&profile.lighting)
        .context("Perfil de iluminação inexistente")?
        .clone();
    next.cooling = state
        .config
        .profiles
        .cooling
        .get(&profile.cooling)
        .context("Perfil de refrigeração inexistente")?
        .clone();
    for device in &state.devices {
        if device.status != DeviceStatus::Ready {
            bail!("Dispositivo indisponível: {}", device.name);
        }
        for channel in &device.lighting {
            let key = format!("{}/{}", device.info.id, channel.name);
            next.lighting
                .entry(key)
                .or_default()
                .validate(&channel.capabilities)?;
        }
        for channel in &device.cooling {
            let key = format!("{}/{}", device.info.id, channel.name);
            let cooling = next.cooling.entry(key).or_insert_with(|| ChannelCooling {
                source: if device.info.kind == Kind::SmartDevice {
                    Source::Cpu
                } else {
                    Source::Liquid
                },
                ..Default::default()
            });
            cooling.points(channel.limits)?;
            if cooling.source == Source::Liquid
                && device
                    .last_status
                    .as_ref()
                    .and_then(|s| s.liquid_temperature)
                    .is_none()
            {
                bail!("Este dispositivo não mede a temperatura do líquido");
            }
        }
    }
    for (keys, lighting) in [
        (next.lighting.keys().collect::<Vec<_>>(), true),
        (next.cooling.keys().collect(), false),
    ] {
        for key in keys {
            let found = state.devices.iter().any(|device| {
                if lighting {
                    device
                        .lighting
                        .iter()
                        .any(|c| format!("{}/{}", device.info.id, c.name) == *key)
                } else {
                    device
                        .cooling
                        .iter()
                        .any(|c| format!("{}/{}", device.info.id, c.name) == *key)
                }
            });
            if !found {
                bail!("Canal do perfil indisponível: {key}");
            }
        }
    }
    next.profiles.active = Some(name.into());
    next.validate()?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captures_independent_profiles_and_preserves_config_on_error() {
        let mut config = Config::default();
        config
            .lighting
            .insert("kraken/ring".into(), Default::default());
        edit(
            &mut config,
            Edit::CaptureLighting {
                name: "Jogo".into(),
            },
        )
        .unwrap();
        config.lighting.clear();
        assert_eq!(config.profiles.lighting["Jogo"].len(), 1);
        let before = config.clone();
        assert!(
            edit(
                &mut config,
                Edit::Save {
                    name: "Erro".into(),
                    profile: SyncProfile {
                        lighting: "Ausente".into(),
                        ..Default::default()
                    }
                }
            )
            .is_err()
        );
        assert_eq!(config, before);
        assert!(
            edit(
                &mut config,
                Edit::Delete {
                    name: "Default".into()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn automatic_rules_handle_midnight_exact_program_and_default() {
        let mut profiles = Profiles::default();
        profiles.sync.insert(
            "Noite".into(),
            SyncProfile {
                auto_launch: AutoLaunch::Schedule {
                    start: 22 * 60,
                    end: 6 * 60,
                    days: [false, true, false, false, false, false, false],
                },
                ..Default::default()
            },
        );
        profiles.validate().unwrap();
        assert_eq!(
            profiles.matching(
                &BTreeSet::new(),
                Some(LocalTime {
                    day: 1,
                    minute: 23 * 60
                })
            ),
            BTreeSet::from(["Noite".into()])
        );
        assert_eq!(
            profiles.matching(&BTreeSet::new(), Some(LocalTime { day: 2, minute: 0 })),
            BTreeSet::from(["Noite".into()])
        );
        for (day, minute) in [(2, 6 * 60), (2, 23 * 60), (1, 0)] {
            assert!(
                profiles
                    .matching(&BTreeSet::new(), Some(LocalTime { day, minute }))
                    .is_empty()
            );
        }
        profiles.sync.get_mut("Noite").unwrap().auto_launch = AutoLaunch::Program {
            executables: vec!["game".into(), "/opt/another game".into()],
        };
        assert_eq!(
            profiles.matching(&BTreeSet::from(["game".into()]), None),
            BTreeSet::from(["Noite".into()])
        );
        assert_eq!(
            profiles.matching(&BTreeSet::from(["other-game".into()]), None),
            BTreeSet::new()
        );
        profiles.sync.get_mut("Default").unwrap().auto_launch = AutoLaunch::Schedule {
            start: 0,
            end: 60,
            days: all_days(),
        };
        assert!(profiles.validate().is_err());
    }

    #[test]
    fn all_day_equal_times_and_missing_clock_follow_reference() {
        let mut profiles = Profiles::default();
        profiles.sync.insert(
            "Dia".into(),
            SyncProfile {
                auto_launch: AutoLaunch::Schedule {
                    start: 0,
                    end: 0,
                    days: [false, false, false, false, false, true, false],
                },
                ..Default::default()
            },
        );
        profiles.validate().unwrap();
        assert!(profiles.matching(&BTreeSet::new(), None).is_empty());
        for minute in [0, 600, 1439] {
            assert!(
                profiles
                    .matching(&BTreeSet::new(), Some(LocalTime { day: 5, minute }))
                    .contains("Dia")
            );
        }
        assert!(
            profiles
                .matching(&BTreeSet::new(), Some(LocalTime { day: 6, minute: 0 }))
                .is_empty()
        );
        profiles.sync.get_mut("Dia").unwrap().auto_launch = AutoLaunch::Schedule {
            start: 600,
            end: 600,
            days: all_days(),
        };
        assert!(
            profiles
                .matching(
                    &BTreeSet::new(),
                    Some(LocalTime {
                        day: 5,
                        minute: 600
                    })
                )
                .is_empty()
        );
        profiles.sync.get_mut("Dia").unwrap().auto_launch = AutoLaunch::Schedule {
            start: 0,
            end: 0,
            days: [false; 7],
        };
        assert!(profiles.validate().is_err());
    }

    #[test]
    fn automatic_stack_restores_previous_manual_and_overlapping_rules() {
        let mut profiles = Profiles::default();
        for name in ["Manual", "A", "B"] {
            profiles.sync.insert(name.into(), SyncProfile::default());
        }
        let mut stack = ActivationStack::default();
        stack.manual("Manual");
        stack = stack.reconcile(&profiles, BTreeSet::from(["A".into()]));
        assert_eq!(stack.target(), "A");
        stack = stack.reconcile(&profiles, BTreeSet::from(["A".into(), "B".into()]));
        assert_eq!(stack.target(), "B");
        stack = stack.reconcile(&profiles, BTreeSet::from(["A".into()]));
        assert_eq!(stack.target(), "A");
        stack.manual("Manual");
        stack = stack.reconcile(&profiles, BTreeSet::from(["A".into()]));
        assert_eq!(stack.target(), "Manual");
        stack = stack.reconcile(&profiles, BTreeSet::new());
        assert_eq!(stack.target(), "Manual");
        stack.manual("A");
        profiles.sync.remove("A");
        stack = stack.reconcile(&profiles, BTreeSet::new());
        assert_eq!(stack.target(), "Manual");
    }

    #[test]
    fn rename_preserves_selection_and_rejects_duplicate_names_atomically() {
        let mut config = Config::default();
        edit(
            &mut config,
            Edit::Save {
                name: "Jogo".into(),
                profile: SyncProfile::default(),
            },
        )
        .unwrap();
        config.profiles.active = Some("Jogo".into());
        edit(
            &mut config,
            Edit::Rename {
                name: "Jogo".into(),
                new_name: "Novo".into(),
            },
        )
        .unwrap();
        assert_eq!(config.profiles.active.as_deref(), Some("Novo"));
        let before = config.clone();
        assert!(
            edit(
                &mut config,
                Edit::Rename {
                    name: "Novo".into(),
                    new_name: "Default".into()
                }
            )
            .is_err()
        );
        assert_eq!(config, before);
        assert!(valid_name(&"é".repeat(50)).is_ok());
        assert!(valid_name(&"é".repeat(51)).is_err());
    }
}
