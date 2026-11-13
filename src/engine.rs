use crate::{
    config::{Config, TemperatureUnit},
    curves::{ChannelCooling, Point, Source},
    devices::{self, Info, Transport},
    driver::{self, CoolingChannel, Driver, LightingChannel, Status},
    governor::{self, Action, ChannelState},
    lighting::ChannelLighting,
    permissions,
    sensors::{Readings, Sensors},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

const DISCOVERY_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Request {
    RefreshPrograms,
    BrowsePrograms {
        directory: PathBuf,
    },
    CheckUpdates,
    InstallUpdate,
    RestartAfterUpdate,
    SaveProfileAndActivate {
        name: String,
        profile: crate::profiles::SyncProfile,
    },
    EditProfile {
        edit: crate::profiles::Edit,
    },
    ActivateProfile {
        name: String,
    },
    Status,
    Refresh,
    SetCooling {
        channel: String,
        cooling: ChannelCooling,
    },
    SetLighting {
        channel: String,
        lighting: ChannelLighting,
    },
    UpdateSettings {
        #[serde(default)]
        language: Option<String>,
        #[serde(default)]
        temperature_unit: Option<TemperatureUnit>,
        #[serde(default)]
        cpu_average_temperature: Option<bool>,
        #[serde(default)]
        start_minimized: Option<bool>,
        #[serde(default)]
        mini_mode: Option<bool>,
        #[serde(default)]
        mini_always_on_top: Option<bool>,
        #[serde(default)]
        gpu_index: Option<usize>,
        #[serde(default)]
        close_to_tray: Option<bool>,
    },
    /// Substitui as preferências de interface (painéis, Mini Mode, idioma…).
    UpdateUi {
        ui: crate::config::UiConfig,
    },
    ExportConfig {
        path: PathBuf,
    },
    ImportConfig {
        path: PathBuf,
    },
    SetAutostart {
        enabled: bool,
    },
    PreparePermissions {
        action: permissions::Action,
    },
    ConfirmPlan {
        id: u64,
    },
    CancelPlan,
    Show,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeviceStatus {
    Initializing,
    Ready,
    PermissionDenied,
    Conflict { processes: Vec<String> },
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceState {
    pub info: Info,
    pub name: String,
    pub status: DeviceStatus,
    pub error: Option<String>,
    pub cooling: Vec<CoolingChannel>,
    pub lighting: Vec<LightingChannel>,
    pub last_status: Option<Status>,
    pub channels: BTreeMap<String, ChannelState>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub programs: Vec<crate::profiles::Program>,
    #[serde(default)]
    pub program_browser: crate::profiles::ProgramBrowser,
    #[serde(default)]
    pub local_time: Option<crate::profiles::LocalTime>,
    #[serde(default)]
    pub updates: crate::updater::State,
    pub config: Config,
    pub devices: Vec<DeviceState>,
    pub sensors: Readings,
    pub permissions: permissions::State,
    pub autostart: bool,
    pub languages: Vec<String>,
    pub specs: crate::specs::Specs,
    pub message: String,
    pub error: Option<String>,
    /// Incrementado a cada pedido de mostrar a janela (bandeja, segunda instância).
    pub show_requests: u64,
    pub quit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    pub message: String,
    pub state: Snapshot,
}

pub type SharedState = Arc<Mutex<Snapshot>>;

/// Caminhos do sistema; os testes apontam para árvores temporárias.
#[derive(Clone, Debug)]
pub struct System {
    pub sys: PathBuf,
    pub dev: PathBuf,
    pub proc_: PathBuf,
    pub rule: PathBuf,
}

impl Default for System {
    fn default() -> Self {
        Self {
            sys: "/sys".into(),
            dev: "/dev".into(),
            proc_: "/proc".into(),
            rule: permissions::RULE_PATH.into(),
        }
    }
}

pub type Opener = Box<dyn Fn(&Info) -> Result<Box<dyn Transport>> + Send>;

pub struct Engine {
    config_path: PathBuf,
    system: System,
    pub state: Snapshot,
    pub shared: SharedState,
    drivers: BTreeMap<String, Box<dyn Driver>>,
    sensors: Sensors,
    opener: Opener,
    last_cycle: Option<Instant>,
    last_discovery: Option<Instant>,
    next_plan: u64,
    plan_result: Option<Receiver<Result<String>>>,
    last_profile_check: Option<Instant>,
    profile_stack: crate::profiles::ActivationStack,
    /// Última iluminação enviada por canal; evita reenviar a mesma cor a cada ciclo.
    lighting_sent: BTreeMap<String, ChannelLighting>,
    update_job: Option<crate::updater::Job>,
    installation: Option<crate::installation::Installed>,
}

fn channel_key(device: &str, channel: &str) -> String {
    format!("{device}/{channel}")
}

impl Engine {
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let opener: Opener = Box::new(|info: &Info| {
            Ok(Box::new(devices::Hidraw::open(&info.node)?) as Box<dyn Transport>)
        });
        Self::with_system(config_path, System::default(), opener)
    }

    pub fn with_system(config_path: PathBuf, system: System, opener: Opener) -> Result<Self> {
        let mut config = Config::load(&config_path)?;
        crate::profiles::initialize(&mut config);
        let sensors = Sensors::new(system.sys.clone(), system.proc_.clone());
        let mut engine = Self {
            state: Snapshot {
                config,
                languages: crate::i18n::available(&config_path),
                specs: crate::specs::read_specs(&system.sys, &system.proc_),
                local_time: crate::profiles::local_time(),
                ..Default::default()
            },
            shared: SharedState::default(),
            config_path,
            system,
            drivers: BTreeMap::new(),
            sensors,
            opener,
            last_cycle: None,
            last_discovery: None,
            next_plan: 1,
            plan_result: None,
            last_profile_check: None,
            profile_stack: Default::default(),
            lighting_sent: BTreeMap::new(),
            update_job: None,
            installation: None,
        };
        match crate::installation::Installed::discover() {
            Ok(installed) => {
                engine.state.updates.installed = installed.is_some();
                engine.installation = installed;
            }
            Err(error) => engine.state.updates.message = format!("{error:#}"),
        }
        engine.refresh_permissions();
        engine.state.autostart = crate::desktop::autostart_enabled().unwrap_or(false);
        engine.discover();
        engine.cycle();
        engine.publish();
        Ok(engine)
    }

    fn publish(&self) {
        *self.shared.lock().unwrap_or_else(|e| e.into_inner()) = self.state.clone();
    }

    fn refresh_permissions(&mut self) {
        let (installed, outdated) = permissions::read_state(&self.system.rule);
        self.state.permissions.rule_installed = installed;
        self.state.permissions.rule_outdated = outdated;
    }

    /// Abre dispositivos novos e retira os que sumiram.
    fn discover(&mut self) {
        self.last_discovery = Some(Instant::now());
        let found = devices::discover(&self.system.sys, &self.system.dev);
        self.state.devices.retain(|d| {
            found
                .iter()
                .any(|f| f.id == d.info.id && f.node == d.info.node)
        });
        self.drivers
            .retain(|id, _| found.iter().any(|f| &f.id == id));
        for info in found {
            if self.drivers.contains_key(&info.id) {
                continue;
            }
            let state = self.open_device(info.clone());
            match self.state.devices.iter_mut().find(|d| d.info.id == info.id) {
                Some(existing) => *existing = state,
                None => self.state.devices.push(state),
            }
        }
        self.state.devices.sort_by(|a, b| a.info.id.cmp(&b.info.id));
    }

    fn open_device(&mut self, info: Info) -> DeviceState {
        let mut state = DeviceState {
            name: info.kind.display_name().into(),
            info: info.clone(),
            status: DeviceStatus::Initializing,
            error: None,
            cooling: Vec::new(),
            lighting: Vec::new(),
            last_status: None,
            channels: BTreeMap::new(),
        };
        let holders = devices::holders(&self.system.proc_, &info.node);
        if !holders.is_empty() {
            state.status = DeviceStatus::Conflict {
                processes: holders
                    .into_iter()
                    .map(|(pid, comm)| format!("{comm} ({pid})"))
                    .collect(),
            };
            state.error = Some(
                "Outro programa está usando o dispositivo; feche-o e clique em Atualizar".into(),
            );
            return state;
        }
        let transport = match (self.opener)(&info) {
            Ok(transport) => transport,
            Err(e) => {
                let denied = format!("{e:#}").contains("Sem permissão");
                state.status = if denied {
                    DeviceStatus::PermissionDenied
                } else {
                    DeviceStatus::Failed
                };
                state.error = Some(format!("{e:#}"));
                return state;
            }
        };
        let mut driver = driver::open(info.clone(), transport);
        // Inicializar primeiro: no Kraken, o firmware lido decide se há curva interna.
        if let Err(e) = driver.initialize() {
            state.status = DeviceStatus::Failed;
            state.error = Some(format!("Falha ao inicializar: {e:#}"));
            return state;
        }
        state.cooling = driver.cooling_channels();
        state.lighting = driver.lighting_channels();
        for channel in &state.lighting {
            let key = channel_key(&info.id, &channel.name);
            if let Some(lighting) = self.state.config.lighting.get(&key)
                && lighting.validate(&channel.capabilities).is_ok()
                && let Some(resolved) = self.resolve_lighting(lighting)
            {
                match driver.set_lighting(&channel.name, &resolved) {
                    Ok(()) => {
                        self.lighting_sent.insert(key, resolved);
                    }
                    Err(e) => state.error = Some(format!("Iluminação {}: {e:#}", channel.name)),
                }
            }
        }
        state.status = DeviceStatus::Ready;
        self.drivers.insert(driver.info().id.clone(), driver);
        state
    }

    /// Um ciclo do governador: sensores, status e escritas de refrigeração.
    fn cycle(&mut self) {
        self.last_cycle = Some(Instant::now());
        self.state.sensors = self.sensors.read();
        let config = self.state.config.clone();
        for device in &mut self.state.devices {
            let Some(driver) = self.drivers.get_mut(&device.info.id) else {
                continue;
            };
            match driver.status() {
                Ok(status) => {
                    device.last_status = Some(status);
                    if device.status == DeviceStatus::Failed {
                        device.status = DeviceStatus::Ready;
                        device.error = None;
                    }
                }
                Err(e) => {
                    device.status = DeviceStatus::Failed;
                    device.error = Some(format!("Comunicação com o dispositivo falhou: {e:#}"));
                    continue;
                }
            }
            let liquid = device
                .last_status
                .as_ref()
                .and_then(|s| s.liquid_temperature);
            let inputs = governor::Inputs {
                readings: &self.state.sensors,
                liquid,
                cpu_average: config.ui.cpu_average_temperature,
                config: &config.governor,
            };
            for channel in &device.cooling {
                let key = channel_key(&device.info.id, &channel.name);
                let cooling = config.cooling.get(&key).cloned().unwrap_or_default();
                let state = device.channels.entry(channel.name.clone()).or_default();
                let action = governor::decide(channel, &cooling, state, &inputs);
                let result = match &action {
                    Action::Profile(points) => driver.set_profile(&channel.name, points),
                    Action::Duty(duty) => driver.set_duty(&channel.name, *duty),
                    Action::Keep => Ok(()),
                };
                match result {
                    Ok(()) => governor::applied(state, &action),
                    Err(e) => state.error = Some(format!("{e:#}")),
                }
            }
        }
        self.refresh_temperature_lighting();
    }

    /// Converte modos por temperatura em cor fixa com as leituras atuais.
    fn resolve_lighting(&self, lighting: &ChannelLighting) -> Option<ChannelLighting> {
        let sensors = &self.state.sensors;
        lighting.resolved(
            sensors.cpu_temperature(self.state.config.ui.cpu_average_temperature),
            sensors.gpu_temperature(self.state.config.governor.gpu_index),
        )
    }

    /// Envia a cor da faixa atual aos canais em modo por temperatura, só quando muda.
    fn refresh_temperature_lighting(&mut self) {
        let pending: Vec<(String, ChannelLighting)> = self
            .state
            .config
            .lighting
            .iter()
            .filter(|(_, lighting)| lighting.mode.is_temperature())
            .filter_map(|(key, lighting)| Some((key.clone(), self.resolve_lighting(lighting)?)))
            .filter(|(key, resolved)| self.lighting_sent.get(key) != Some(resolved))
            .collect();
        for (key, resolved) in pending {
            let Some((id, channel)) = key.rsplit_once('/') else {
                continue;
            };
            if let Some(driver) = self.drivers.get_mut(id)
                && driver.set_lighting(channel, &resolved).is_ok()
            {
                self.lighting_sent.insert(key.clone(), resolved);
            }
        }
    }

    pub fn reply(&self, ok: bool, message: impl Into<String>) -> Reply {
        Reply {
            ok,
            message: message.into(),
            state: self.state.clone(),
        }
    }

    pub fn handle(&mut self, request: Request) -> Reply {
        let result = self.dispatch(request);
        let reply = match result {
            Ok(message) => {
                self.state.message = message.clone();
                self.state.error = None;
                self.reply(true, message)
            }
            Err(e) => {
                let message = format!("{e:#}");
                self.state.error = Some(message.clone());
                self.reply(false, message)
            }
        };
        self.publish();
        reply
    }

    fn find_channel(&self, key: &str) -> Result<(&DeviceState, String)> {
        let (device, channel) = key
            .rsplit_once('/')
            .context("Canal deve ser <dispositivo>/<canal>")?;
        let state = self
            .state
            .devices
            .iter()
            .find(|d| d.info.id == device)
            .context("Dispositivo não encontrado")?;
        Ok((state, channel.to_owned()))
    }

    fn save_config(&mut self, config: Config) -> Result<()> {
        config.save(&self.config_path)?;
        self.state.config = config;
        Ok(())
    }

    fn apply_profile(&mut self, config: Config) -> Result<String> {
        let mut applying = self.state.config.clone();
        applying.profiles.active = None;
        self.save_config(applying)?;
        for (key, lighting) in &config.lighting {
            let (id, channel) = key.rsplit_once('/').context("Canal inválido")?;
            // Sem leitura do sensor, o modo por temperatura é enviado no próximo ciclo.
            let Some(resolved) = self.resolve_lighting(lighting) else {
                continue;
            };
            let result = self
                .drivers
                .get_mut(id)
                .context("Dispositivo indisponível")?
                .set_lighting(channel, &resolved);
            if result.is_ok() {
                self.lighting_sent.insert(key.clone(), resolved);
            }
            if let Err(error) = result {
                // Uma falha pode ocorrer depois de outros canais terem recebido comandos.
                self.state.config.profiles.active = None;
                bail!(
                    "Aplicação de perfil interrompida; iluminação pode estar parcialmente aplicada: {error:#}"
                );
            }
        }
        self.save_config(config)?;
        self.cycle();
        for device in &self.state.devices {
            if device.status != DeviceStatus::Ready
                || device
                    .channels
                    .values()
                    .any(|channel| channel.error.is_some())
            {
                self.state.config.profiles.active = None;
                self.save_config(self.state.config.clone())?;
                bail!("Falha ao aplicar refrigeração do perfil; confira os canais");
            }
        }
        Ok("Perfil ativado".into())
    }

    fn dispatch(&mut self, request: Request) -> Result<String> {
        match request {
            Request::RefreshPrograms => {
                self.state.programs = crate::profiles::catalog(&self.system.proc_);
                Ok(String::new())
            }
            Request::BrowsePrograms { directory } => {
                self.state.program_browser = crate::profiles::browse(directory)?;
                Ok(String::new())
            }
            Request::CheckUpdates => {
                if self.update_job.is_none() {
                    self.update_job = Some(crate::updater::Job::check());
                    self.state.updates.phase = crate::updater::Phase::Checking;
                    self.state.updates.message = "Verificando atualizações…".into();
                }
                Ok("Consultando atualizações".into())
            }
            Request::InstallUpdate => {
                if self.update_job.is_some()
                    || self.state.updates.busy()
                    || self.state.permissions.running
                {
                    bail!("Aguarde a operação atual terminar");
                }
                if self.state.updates.phase != crate::updater::Phase::Available {
                    bail!("Verifique as atualizações primeiro");
                }
                let installed = self
                    .installation
                    .clone()
                    .context("Use o instalador desktop para habilitar atualizações")?;
                let candidate = self
                    .state
                    .updates
                    .candidate
                    .clone()
                    .context("Atualização não selecionada")?;
                let log = crate::updater::log_path()?;
                self.update_job = Some(crate::updater::Job::install(
                    candidate,
                    installed,
                    log.clone(),
                ));
                self.state.updates.phase = crate::updater::Phase::Installing;
                self.state.updates.log = Some(log);
                Ok("Atualização iniciada; a refrigeração continua ativa".into())
            }
            Request::RestartAfterUpdate => {
                if self.state.updates.phase != crate::updater::Phase::RestartRequired
                    || self.state.permissions.running
                {
                    bail!("Não há atualização pronta para reiniciar");
                }
                let installed = self
                    .installation
                    .clone()
                    .context("Instalação não registrada")?;
                self.prepare_safe_exit()?;
                let gui =
                    cfg!(feature = "gui") && !std::env::args_os().any(|arg| arg == "--headless");
                crate::updater::restart(&installed, &self.config_path, gui)?;
                self.drivers.clear();
                self.state.quit = true;
                Ok("Reiniciando".into())
            }
            Request::SaveProfileAndActivate { name, profile } => {
                let mut next = self.state.clone();
                crate::profiles::edit(
                    &mut next.config,
                    crate::profiles::Edit::Save {
                        name: name.clone(),
                        profile,
                    },
                )?;
                let config = crate::profiles::prepare(&next, &name)?;
                let message = self.apply_profile(config)?;
                if self.state.config.profiles.active.as_deref() != Some(self.profile_stack.target())
                {
                    self.profile_stack.manual(&name);
                }
                Ok(message)
            }
            Request::EditProfile { edit } => {
                let mut config = self.state.config.clone();
                crate::profiles::edit(&mut config, edit.clone())?;
                if matches!(&edit, crate::profiles::Edit::Delete { name } if self.state.config.profiles.active.as_ref() == Some(name))
                {
                    let mut next = self.state.clone();
                    next.config = config;
                    let name = next
                        .config
                        .profiles
                        .active
                        .clone()
                        .unwrap_or_else(|| "Default".into());
                    self.apply_profile(crate::profiles::prepare(&next, &name)?)?;
                    self.profile_stack.manual(&name);
                } else {
                    if matches!(&edit, crate::profiles::Edit::Save { name, .. } if config.profiles.active.as_ref() == Some(name))
                    {
                        config.profiles.active = None;
                    }
                    self.save_config(config)?;
                }
                if let crate::profiles::Edit::Rename { name, new_name } = edit {
                    self.profile_stack.rename(&name, &new_name);
                }
                Ok("Perfil salvo".into())
            }
            Request::ActivateProfile { name } => {
                let config = crate::profiles::prepare(&self.state, &name)?;
                let message = self.apply_profile(config)?;
                self.profile_stack.manual(&name);
                Ok(message)
            }
            Request::Status => Ok(String::new()),
            Request::Refresh => {
                self.drivers.clear();
                self.state.devices.clear();
                self.refresh_permissions();
                self.state.languages = crate::i18n::available(&self.config_path);
                self.state.autostart = crate::desktop::autostart_enabled().unwrap_or(false);
                self.discover();
                self.cycle();
                Ok("Dispositivos atualizados".into())
            }
            Request::SetCooling { channel, cooling } => {
                let (device, name) = self.find_channel(&channel)?;
                let physical = device
                    .cooling
                    .iter()
                    .find(|c| c.name == name)
                    .context("Canal de refrigeração não encontrado")?;
                if cooling.source == Source::Liquid
                    && device
                        .last_status
                        .as_ref()
                        .and_then(|s| s.liquid_temperature)
                        .is_none()
                    && !physical.internal_profile
                {
                    bail!("Este dispositivo não mede a temperatura do líquido");
                }
                cooling.points(physical.limits)?;
                let mut config = self.state.config.clone();
                config.cooling.insert(channel, cooling);
                config.profiles.active = None;
                self.save_config(config)?;
                self.cycle();
                Ok("Perfil de refrigeração aplicado".into())
            }
            Request::SetLighting { channel, lighting } => {
                let (device, name) = self.find_channel(&channel)?;
                let physical = device
                    .lighting
                    .iter()
                    .find(|c| c.name == name)
                    .context("Canal de iluminação não encontrado")?;
                lighting.validate(&physical.capabilities)?;
                let id = device.info.id.clone();
                let resolved = self.resolve_lighting(&lighting);
                let driver = self
                    .drivers
                    .get_mut(&id)
                    .context("Dispositivo indisponível")?;
                if let Some(resolved) = resolved {
                    driver.set_lighting(&name, &resolved)?;
                    self.lighting_sent.insert(channel.clone(), resolved);
                }
                let mut config = self.state.config.clone();
                config.lighting.insert(channel, lighting);
                config.profiles.active = None;
                self.save_config(config)?;
                Ok("Iluminação aplicada".into())
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
                let mut config = self.state.config.clone();
                if let Some(language) = language {
                    crate::i18n::load(&language, &self.config_path)?;
                    config.ui.language = language;
                }
                if let Some(unit) = temperature_unit {
                    config.ui.temperature_unit = unit;
                }
                if let Some(average) = cpu_average_temperature {
                    config.ui.cpu_average_temperature = average;
                }
                if let Some(minimized) = start_minimized {
                    config.ui.start_minimized = minimized;
                }
                if let Some(mini) = mini_mode {
                    config.ui.mini_mode = mini;
                }
                if let Some(on_top) = mini_always_on_top {
                    config.ui.mini_always_on_top = on_top;
                }
                if let Some(index) = gpu_index {
                    config.governor.gpu_index = index;
                }
                if let Some(close) = close_to_tray {
                    config.ui.close_to_tray = close;
                }
                self.save_config(config)?;
                Ok("Configurações salvas".into())
            }
            Request::UpdateUi { ui } => {
                if ui.language != self.state.config.ui.language {
                    crate::i18n::load(&ui.language, &self.config_path)?;
                }
                let mut config = self.state.config.clone();
                config.ui = ui;
                self.save_config(config)?;
                Ok("Configurações salvas".into())
            }
            Request::ExportConfig { path } => {
                self.state.config.save(&path)?;
                Ok(format!("Configurações exportadas para {}", path.display()))
            }
            Request::ImportConfig { path } => {
                if !path.is_file() {
                    bail!("Arquivo de backup não encontrado: {}", path.display());
                }
                let config = Config::load(&path)?;
                self.save_config(config)?;
                self.cycle();
                Ok("Configurações importadas".into())
            }
            Request::SetAutostart { enabled } => {
                if enabled {
                    crate::desktop::install_autostart()?;
                } else {
                    crate::desktop::remove_autostart()?;
                }
                self.state.autostart = crate::desktop::autostart_enabled()?;
                Ok(if enabled {
                    "Início automático ativado"
                } else {
                    "Início automático desativado"
                }
                .into())
            }
            Request::PreparePermissions { action } => {
                if self.state.permissions.running {
                    bail!("Uma preparação de permissões já está em andamento");
                }
                let id = self.next_plan;
                self.next_plan += 1;
                self.state.permissions.plan =
                    Some(permissions::plan(id, action, &std::env::current_exe()?));
                Ok("Revise o plano e confirme".into())
            }
            Request::ConfirmPlan { id } => {
                let plan = self
                    .state
                    .permissions
                    .plan
                    .take()
                    .filter(|p| p.id == id)
                    .context("Plano inexistente ou já usado; prepare novamente")?;
                let executable = std::env::current_exe()?;
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(permissions::run(plan.action, &executable));
                });
                self.plan_result = Some(rx);
                self.state.permissions.running = true;
                self.state.permissions.error = None;
                Ok("Aguardando autorização do Ubuntu".into())
            }
            Request::CancelPlan => {
                self.state.permissions.plan = None;
                Ok("Plano descartado".into())
            }
            Request::Show => {
                self.state.show_requests += 1;
                Ok(String::new())
            }
            Request::Quit => {
                if self.state.permissions.running {
                    bail!("Aguarde a preparação de permissões terminar");
                }
                self.shutdown();
                self.state.quit = true;
                Ok("Encerrando".into())
            }
        }
    }

    /// Deixa cada canal num estado seguro sem o aplicativo.
    fn prepare_safe_exit(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        let config = self.state.config.clone();
        for device in &self.state.devices {
            let Some(driver) = self.drivers.get_mut(&device.info.id) else {
                continue;
            };
            for channel in &device.cooling {
                let key = channel_key(&device.info.id, &channel.name);
                let cooling = config.cooling.get(&key).cloned().unwrap_or_default();
                let result = if channel.internal_profile {
                    // Curva do perfil se a fonte for o líquido; senão Silent pelo líquido.
                    let liquid = if cooling.source == Source::Liquid {
                        cooling
                    } else {
                        ChannelCooling::default()
                    };
                    let points: Vec<Point> = liquid
                        .points(channel.limits)
                        .unwrap_or_else(|_| driver::flat_with_safety(100, channel.limits));
                    driver.set_profile(&channel.name, &points)
                } else {
                    driver.set_duty(&channel.name, config.governor.exit_duty)
                };
                if let Err(e) = result {
                    errors.push(format!("Encerramento de {key}: {e:#}"));
                }
            }
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("; "));
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        if let Err(error) = self.prepare_safe_exit() {
            eprintln!("{error:#}");
        }
        self.drivers.clear();
    }

    /// Chamado pelo laço do controlador; retorna `true` se o estado mudou.
    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        while let Some(event) = self.update_job.as_ref().and_then(|job| job.poll()) {
            let complete = !matches!(&event, crate::updater::Event::Progress(_));
            self.state.updates.accept(event);
            if complete {
                self.update_job = None;
            }
            changed = true;
        }
        if self
            .last_profile_check
            .is_none_or(|time| time.elapsed() >= Duration::from_secs(3))
        {
            self.last_profile_check = Some(Instant::now());
            self.state.local_time = crate::profiles::local_time();
            let rules = &self.state.config.profiles;
            let matched = rules.matching(
                &crate::profiles::programs(&self.system.proc_),
                self.state.local_time,
            );
            let next = self.profile_stack.reconcile(rules, matched);
            if next.target() != self.profile_stack.target() {
                let result = crate::profiles::prepare(&self.state, next.target())
                    .and_then(|config| self.apply_profile(config));
                match result {
                    Ok(_) => self.profile_stack = next,
                    Err(error) => {
                        self.state.error = Some(format!("Ativação automática: {error:#}"))
                    }
                }
                changed = true;
            } else {
                self.profile_stack = next;
            }
        }
        if let Some(rx) = &self.plan_result
            && let Ok(result) = rx.try_recv()
        {
            self.plan_result = None;
            self.state.permissions.running = false;
            match result {
                Ok(log) => {
                    self.state.permissions.log = log;
                    self.state.message = "Permissões preparadas".into();
                }
                Err(e) => self.state.permissions.error = Some(format!("{e:#}")),
            }
            self.refresh_permissions();
            // A regra nova vale para nós reabertos.
            self.drivers.clear();
            self.state.devices.clear();
            self.discover();
            changed = true;
        }
        let interval = Duration::from_millis(self.state.config.governor.interval_ms);
        if self.last_cycle.is_none_or(|t| t.elapsed() >= interval) {
            if self
                .last_discovery
                .is_none_or(|t| t.elapsed() >= DISCOVERY_INTERVAL)
            {
                self.discover();
            }
            self.cycle();
            changed = true;
        }
        if changed {
            self.publish();
        }
        changed
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if !self.drivers.is_empty() {
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeDriver {
        metadata: Box<dyn Driver>,
        writes: Arc<AtomicUsize>,
        fail_exit: bool,
    }
    impl Driver for FakeDriver {
        fn info(&self) -> &Info {
            self.metadata.info()
        }
        fn cooling_channels(&self) -> Vec<CoolingChannel> {
            self.metadata.cooling_channels()
        }
        fn lighting_channels(&self) -> Vec<LightingChannel> {
            self.metadata.lighting_channels()
        }
        fn initialize(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&mut self) -> Result<Status> {
            Ok(Status {
                liquid_temperature: Some(33.0),
                ..Default::default()
            })
        }
        fn set_profile(&mut self, _: &str, _: &[Point]) -> Result<()> {
            if self.fail_exit {
                bail!("Fixture: falha no estado de saída");
            }
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn set_duty(&mut self, _: &str, _: u8) -> Result<()> {
            if self.fail_exit {
                bail!("Fixture: falha no estado de saída");
            }
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn set_lighting(&mut self, _: &str, _: &ChannelLighting) -> Result<()> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn fixture(root: &std::path::Path) -> (Engine, Arc<AtomicUsize>) {
        let system = System {
            sys: root.join("sys"),
            dev: root.join("dev"),
            proc_: root.join("proc"),
            rule: root.join("rule"),
        };
        let mut engine = Engine::with_system(
            root.join("config.json"),
            system,
            Box::new(|_| bail!("No hardware in tests")),
        )
        .unwrap();
        let info = Info {
            kind: devices::Kind::KrakenX,
            id: "kraken:mock".into(),
            node: root.join("fake-hidraw"),
            serial: "mock".into(),
            phys: String::new(),
        };
        let metadata = driver::open(info.clone(), Box::new(devices::Mock::default()));
        let writes = Arc::new(AtomicUsize::new(0));
        engine.state.devices.push(DeviceState {
            info: info.clone(),
            name: "Kraken X".into(),
            status: DeviceStatus::Ready,
            error: None,
            cooling: metadata.cooling_channels(),
            lighting: metadata.lighting_channels(),
            last_status: Some(Status {
                liquid_temperature: Some(33.0),
                ..Default::default()
            }),
            channels: Default::default(),
        });
        engine.drivers.insert(
            info.id,
            Box::new(FakeDriver {
                metadata,
                writes: writes.clone(),
                fail_exit: false,
            }),
        );
        (engine, writes)
    }

    #[test]
    fn invalid_profile_is_rejected_before_any_device_write() {
        let dir = tempfile::tempdir().unwrap();
        let (mut engine, writes) = fixture(dir.path());
        let invalid = ChannelLighting {
            brightness: 101,
            ..Default::default()
        };
        engine
            .state
            .config
            .profiles
            .lighting
            .get_mut("Padrão")
            .unwrap()
            .insert("kraken:mock/ring".into(), invalid);
        let reply = engine.handle(Request::ActivateProfile {
            name: "Default".into(),
        });
        assert!(!reply.ok);
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        let reply = engine.handle(Request::SaveProfileAndActivate {
            name: "Broken".into(),
            profile: crate::profiles::SyncProfile {
                lighting: "missing".into(),
                ..Default::default()
            },
        });
        assert!(!reply.ok);
        assert!(!engine.state.config.profiles.sync.contains_key("Broken"));
        assert_eq!(writes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn applies_profile_through_controller_and_persists_active_name() {
        let dir = tempfile::tempdir().unwrap();
        let (mut engine, writes) = fixture(dir.path());
        let reply = engine.handle(Request::ActivateProfile {
            name: "Default".into(),
        });
        assert!(reply.ok, "{}", reply.message);
        assert!(writes.load(Ordering::SeqCst) >= 2);
        assert_eq!(
            Config::load(&dir.path().join("config.json"))
                .unwrap()
                .profiles
                .active
                .as_deref(),
            Some("Default")
        );
        assert!(
            engine
                .handle(Request::SetLighting {
                    channel: "kraken:mock/ring".into(),
                    lighting: Default::default()
                })
                .ok
        );
        assert!(engine.state.config.profiles.active.is_none());
    }

    #[test]
    fn failed_exit_preparation_keeps_the_controller_for_safe_retry() {
        let dir = tempfile::tempdir().unwrap();
        let (mut engine, writes) = fixture(dir.path());
        let info = engine.state.devices[0].info.clone();
        engine.drivers.insert(
            info.id.clone(),
            Box::new(FakeDriver {
                metadata: driver::open(info, Box::new(devices::Mock::default())),
                writes,
                fail_exit: true,
            }),
        );
        assert!(engine.prepare_safe_exit().is_err());
        assert!(!engine.state.quit);
        assert_eq!(engine.drivers.len(), 1);
    }
}
