use crate::{
    config::TemperatureUnit,
    curves::{self, ChannelCooling, Point, Profile, Source},
    driver::{ChannelKind, CoolingChannel, LightingChannel},
    engine::{DeviceState, DeviceStatus, Request, Snapshot},
    i18n::{tr as t, trf},
    ipc::Controller,
    lighting::{ChannelLighting, Mode, Rgb, Speed},
    permissions,
};
use anyhow::Result;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use std::collections::HashMap;

const CANVAS: Color32 = Color32::from_rgb(18, 18, 24);
const PANEL: Color32 = Color32::from_rgb(28, 28, 36);
const BORDER: Color32 = Color32::from_rgb(48, 48, 60);
const TEXT: Color32 = Color32::from_rgb(232, 232, 240);
const MUTED: Color32 = Color32::from_rgb(150, 150, 165);
const ACCENT: Color32 = Color32::from_rgb(126, 34, 206);
const WARN: Color32 = Color32::from_rgb(255, 176, 120);
const GOOD: Color32 = Color32::from_rgb(110, 200, 150);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Monitoring,
    Cooling,
    Lighting,
    Settings,
}

impl Page {
    const ALL: [Self; 4] = [
        Self::Monitoring,
        Self::Cooling,
        Self::Lighting,
        Self::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Monitoring => "Monitoramento",
            Self::Cooling => "Refrigeração",
            Self::Lighting => "Iluminação",
            Self::Settings => "Configurações",
        }
    }
}

/// Ícone próprio: anel com o acento da interface (sem recursos da NZXT).
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    let center = (size as f32 - 1.0) / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt() / center;
            let ring = (0.62..=0.95).contains(&d);
            let dot = d <= 0.28;
            if ring || dot {
                out.extend_from_slice(&[ACCENT.r(), ACCENT.g(), ACCENT.b(), 255]);
            } else {
                out.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    out
}

pub fn run(controller: Controller, minimized: bool) -> Result<()> {
    let tx = controller.tx.clone();
    ctrlc::set_handler(move || {
        let _ = tx.send(Request::Quit);
    })?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NZXT CAM")
            .with_app_id(crate::desktop::APP_ID)
            .with_icon(egui::IconData {
                rgba: icon_rgba(64),
                width: 64,
                height: 64,
            })
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([320.0, 200.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "NZXT CAM",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, controller, minimized)))),
    )
    .map_err(|e| anyhow::anyhow!("Interface: {e}"))
}

struct App {
    controller: Controller,
    page: Page,
    cooling: HashMap<String, ChannelCooling>,
    lighting: HashMap<String, ChannelLighting>,
    show_requests: u64,
    hidden: bool,
    mini: bool,
    #[cfg(all(feature = "tray", target_os = "linux"))]
    tray: Option<ksni::blocking::Handle<crate::tray::Tray>>,
}

fn key(device: &DeviceState, channel: &str) -> String {
    format!("{}/{channel}", device.info.id)
}

fn channel_label(channel: &str) -> String {
    match channel {
        "pump" => t("Bomba"),
        "fan" => t("Fan"),
        "ring" => t("Anel"),
        "logo" => t("Logo"),
        "led" => t("LEDs"),
        other => match other.strip_prefix("fan") {
            Some(n) => trf("Fan {n}", &[("n", n)]),
            None => other.to_owned(),
        },
    }
}

fn source_label(source: Source) -> String {
    t(match source {
        Source::Liquid => "Líquido",
        Source::Cpu => "CPU",
        Source::Gpu => "GPU",
    })
}

fn profile_label(profile: &Profile) -> &'static str {
    match profile {
        Profile::Silent => "Silent",
        Profile::Performance => "Performance",
        Profile::Fixed { .. } => "Fixed",
        Profile::Custom { .. } => "Custom",
    }
}

fn card<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(13.0).strong().color(MUTED));
            ui.add_space(4.0);
            add(ui)
        })
        .inner
}

fn metric(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).strong());
        });
    });
}

fn percent(value: Option<f64>) -> String {
    value.map_or("—".into(), |v| format!("{v:.0}%"))
}

fn rpm(value: Option<u32>) -> String {
    value.map_or("—".into(), |v| format!("{v} RPM"))
}

fn bytes_per_second(value: Option<f64>) -> String {
    match value {
        None => "—".into(),
        Some(v) if v >= 1e6 => format!("{:.1} MB/s", v / 1e6),
        Some(v) => format!("{:.0} KB/s", v / 1e3),
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, controller: Controller, minimized: bool) -> Self {
        let repaint = cc.egui_ctx.clone();
        *controller.wake.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(Box::new(move || repaint.request_repaint()));
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = CANVAS;
        style.visuals.window_fill = PANEL;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.widgets.active.bg_fill = ACCENT;
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        cc.egui_ctx.set_global_style(style);
        let state = controller
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        #[cfg(all(feature = "tray", target_os = "linux"))]
        let tray = match crate::tray::spawn(controller.tx.clone()) {
            Ok(tray) => Some(tray),
            Err(e) => {
                eprintln!("Bandeja indisponível; a janela continuará acessível: {e}");
                None
            }
        };
        #[cfg(all(feature = "tray", target_os = "linux"))]
        let hidden = minimized && tray.is_some();
        #[cfg(not(all(feature = "tray", target_os = "linux")))]
        let hidden = {
            let _ = minimized;
            false
        };
        if hidden {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        let mut app = Self {
            controller,
            page: Page::Monitoring,
            cooling: HashMap::new(),
            lighting: HashMap::new(),
            show_requests: state.show_requests,
            hidden,
            mini: false,
            #[cfg(all(feature = "tray", target_os = "linux"))]
            tray,
        };
        if state.config.ui.mini_mode {
            app.set_mini(&cc.egui_ctx, true, state.config.ui.mini_always_on_top);
        }
        app
    }

    fn state(&self) -> Snapshot {
        self.controller
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn dispatch(&self, request: Request) {
        let _ = self.controller.tx.send(request);
    }

    fn set_mini(&mut self, ctx: &egui::Context, mini: bool, on_top: bool) {
        self.mini = mini;
        let (size, level) = if mini {
            (
                Vec2::new(320.0, 210.0),
                if on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                },
            )
        } else {
            (Vec2::new(1180.0, 760.0), egui::WindowLevel::Normal)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
    }

    fn monitoring(&self, ui: &mut egui::Ui, state: &Snapshot) {
        let unit = state.config.ui.temperature_unit;
        let sensors = &state.sensors;
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(ui.available_width());
            ui.vertical(|ui| {
                ui.set_width(300.0);
                card(ui, "CPU", |ui| {
                    if !sensors.cpu.name.is_empty() {
                        ui.label(RichText::new(&sensors.cpu.name).size(11.0).color(MUTED));
                    }
                    let temperature =
                        sensors.cpu_temperature(state.config.ui.cpu_average_temperature);
                    ui.label(RichText::new(unit.format(temperature)).size(30.0).strong());
                    metric(ui, &t("Carga"), percent(sensors.cpu.load_percent));
                    metric(
                        ui,
                        &t("Clock"),
                        sensors
                            .cpu
                            .clock_mhz
                            .map_or("—".into(), |v| format!("{v:.0} MHz")),
                    );
                });
            });
            for gpu in &sensors.gpus {
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    card(ui, "GPU", |ui| {
                        ui.label(RichText::new(&gpu.name).size(11.0).color(MUTED));
                        ui.label(
                            RichText::new(unit.format(gpu.temperature))
                                .size(30.0)
                                .strong(),
                        );
                        metric(ui, &t("Carga"), percent(gpu.load_percent));
                        metric(
                            ui,
                            &t("Clock"),
                            gpu.clock_mhz.map_or("—".into(), |v| format!("{v:.0} MHz")),
                        );
                        metric(ui, &t("Fan da GPU"), percent(gpu.fan_percent));
                    });
                });
            }
            ui.vertical(|ui| {
                ui.set_width(300.0);
                card(ui, &t("RAM"), |ui| match &sensors.memory {
                    Some(memory) => {
                        let used = memory.used_bytes as f64 / 1024f64.powi(3);
                        let total = memory.total_bytes as f64 / 1024f64.powi(3);
                        ui.label(
                            RichText::new(format!("{:.0}%", used * 100.0 / total.max(1e-9)))
                                .size(30.0)
                                .strong(),
                        );
                        metric(ui, &t("Em uso"), format!("{used:.1} / {total:.1} GB"));
                    }
                    None => {
                        ui.label(RichText::new(t("Indisponível")).color(MUTED));
                    }
                });
                card(ui, &t("Rede"), |ui| {
                    metric(
                        ui,
                        &t("Recebendo"),
                        bytes_per_second(sensors.network.receive_bytes_per_second),
                    );
                    metric(
                        ui,
                        &t("Enviando"),
                        bytes_per_second(sensors.network.transmit_bytes_per_second),
                    );
                });
            });
            if !sensors.storage.is_empty() {
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    card(ui, &t("Armazenamento"), |ui| {
                        for disk in &sensors.storage {
                            metric(ui, &disk.name, unit.format(disk.temperature));
                        }
                    });
                });
            }
        });
        ui.add_space(8.0);
        for device in &state.devices {
            self.device_summary(ui, device, unit);
        }
        if state.devices.is_empty() {
            card(ui, &t("Dispositivos NZXT"), |ui| {
                ui.label(t("Nenhum dispositivo compatível encontrado. Conecte o Kraken X ou o Smart Device e clique em Atualizar."));
            });
        }
    }

    fn device_summary(&self, ui: &mut egui::Ui, device: &DeviceState, unit: TemperatureUnit) {
        card(ui, &device.name, |ui| {
            self.device_status(ui, device);
            let Some(status) = &device.last_status else {
                return;
            };
            if status.liquid_temperature.is_some() {
                metric(ui, &t("Líquido"), unit.format(status.liquid_temperature));
            }
            for fan in &status.fans {
                metric(ui, &channel_label(&fan.channel), rpm(fan.rpm));
            }
            if let Some(noise) = status.noise_db {
                metric(ui, &t("Ruído do gabinete"), format!("{noise:.0} dBA"));
            }
        });
    }

    fn device_status(&self, ui: &mut egui::Ui, device: &DeviceState) {
        let (text, color) = match &device.status {
            DeviceStatus::Ready => (t("Pronto"), GOOD),
            DeviceStatus::Initializing => (t("Inicializando…"), MUTED),
            DeviceStatus::PermissionDenied => (t("Sem permissão de acesso"), WARN),
            DeviceStatus::Conflict { processes } => (
                trf(
                    "Em uso por outro programa: {programas}",
                    &[("programas", &processes.join(", "))],
                ),
                WARN,
            ),
            DeviceStatus::Failed => (t("A comunicação com o dispositivo falhou"), WARN),
        };
        ui.colored_label(color, text);
        if let Some(error) = &device.error {
            ui.label(RichText::new(t(error)).size(11.0).color(WARN));
        }
        if device.status == DeviceStatus::PermissionDenied {
            ui.label(
                RichText::new(t(
                    "Prepare as permissões em Configurações para acessar o dispositivo sem root.",
                ))
                .size(11.0)
                .color(MUTED),
            );
        }
    }

    fn cooling_page(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        if state.devices.iter().all(|d| d.cooling.is_empty()) {
            card(ui, &t("Refrigeração"), |ui| {
                ui.label(t(
                    "Nenhum dispositivo de refrigeração compatível foi encontrado",
                ));
            });
            return;
        }
        let liquid = state
            .devices
            .iter()
            .filter_map(|d| d.last_status.as_ref()?.liquid_temperature)
            .next();
        for device in &state.devices {
            for channel in &device.cooling {
                let key = key(device, &channel.name);
                let applied = state.config.cooling.get(&key).cloned().unwrap_or_default();
                let mut draft = self
                    .cooling
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| applied.clone());
                let current = match draft.source {
                    Source::Liquid => liquid,
                    Source::Cpu => state
                        .sensors
                        .cpu_temperature(state.config.ui.cpu_average_temperature),
                    Source::Gpu => state
                        .sensors
                        .gpu_temperature(state.config.governor.gpu_index),
                };
                let title = format!("{} · {}", device.name, channel_label(&channel.name));
                card(ui, &title, |ui| {
                    self.cooling_channel(
                        ui,
                        device,
                        channel,
                        &mut draft,
                        &applied,
                        current,
                        state,
                        liquid.is_some(),
                    );
                });
                self.cooling.insert(key, draft);
                ui.add_space(6.0);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn cooling_channel(
        &self,
        ui: &mut egui::Ui,
        device: &DeviceState,
        channel: &CoolingChannel,
        draft: &mut ChannelCooling,
        applied: &ChannelCooling,
        current: Option<f64>,
        state: &Snapshot,
        has_liquid: bool,
    ) {
        let unit = state.config.ui.temperature_unit;
        let channel_state = device.channels.get(&channel.name);
        let reported = device
            .last_status
            .as_ref()
            .and_then(|s| s.fans.iter().find(|f| f.channel == channel.name));
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(rpm(reported.and_then(|f| f.rpm)))
                    .size(18.0)
                    .strong(),
            );
            if let Some(duty) = channel_state.and_then(|c| c.applied_duty) {
                ui.label(
                    RichText::new(trf("aplicado {valor}%", &[("valor", &duty.to_string())]))
                        .color(MUTED),
                );
            } else if channel_state
                .and_then(|c| c.applied_profile.as_ref())
                .is_some()
            {
                ui.label(RichText::new(t("curva gravada no dispositivo")).color(MUTED));
            }
            if let Some(safety) = channel_state.and_then(|c| c.safety.as_ref()) {
                ui.colored_label(WARN, trf("Segurança: {motivo}", &[("motivo", &t(safety))]));
            }
        });
        if let Some(error) = channel_state.and_then(|c| c.error.as_ref()) {
            ui.colored_label(WARN, t(error));
        }
        ui.horizontal(|ui| {
            for profile in [
                Profile::Silent,
                Profile::Performance,
                Profile::Fixed { duty: 50 },
                Profile::Custom { points: Vec::new() },
            ] {
                let selected =
                    std::mem::discriminant(&draft.profile) == std::mem::discriminant(&profile);
                if ui
                    .selectable_label(selected, profile_label(&profile))
                    .clicked()
                    && !selected
                {
                    draft.profile = match profile {
                        Profile::Custom { .. } => Profile::Custom {
                            points: draft.points(channel.limits).unwrap_or_else(|_| {
                                curves::SILENT
                                    .iter()
                                    .map(|&(temperature, duty)| Point { temperature, duty })
                                    .collect()
                            }),
                        },
                        other => other,
                    };
                }
            }
            ui.separator();
            ui.label(RichText::new(t("Fonte")).color(MUTED));
            egui::ComboBox::from_id_salt(("source", &device.info.id, &channel.name))
                .selected_text(source_label(draft.source))
                .show_ui(ui, |ui| {
                    for source in [Source::Liquid, Source::Cpu, Source::Gpu] {
                        let available = match source {
                            Source::Liquid => has_liquid,
                            Source::Cpu => true,
                            Source::Gpu => !state.sensors.gpus.is_empty(),
                        };
                        ui.add_enabled_ui(available, |ui| {
                            ui.selectable_value(&mut draft.source, source, source_label(source));
                        });
                    }
                });
            ui.label(RichText::new(unit.format(current)).color(MUTED));
        });
        if let Profile::Fixed { duty } = &mut draft.profile {
            ui.add(
                egui::Slider::new(duty, channel.limits.floor..=channel.limits.ceiling)
                    .suffix("%")
                    .text(t("Rotação fixa")),
            );
        }
        let editable = matches!(draft.profile, Profile::Custom { .. });
        let mut points = draft.points(channel.limits).unwrap_or_default();
        if let Profile::Custom { points: custom } = &draft.profile {
            points = custom.clone();
        }
        if curve_editor(
            ui,
            ("curve", &device.info.id, &channel.name),
            &mut points,
            current,
            editable,
            channel.limits,
            draft.source,
        ) {
            draft.profile = Profile::Custom { points };
        }
        let validation = draft.points(channel.limits);
        if let Err(e) = &validation {
            ui.colored_label(WARN, t(&e.to_string()));
        }
        if channel.kind == ChannelKind::Pump {
            ui.label(
                RichText::new(trf(
                    "A bomba nunca fica abaixo de {piso}%.",
                    &[("piso", &channel.limits.floor.to_string())],
                ))
                .size(11.0)
                .color(MUTED),
            );
        }
        if channel.internal_profile {
            ui.label(
                RichText::new(t("Com fonte Líquido a curva roda no próprio dispositivo; com CPU ou GPU o aplicativo ajusta a rotação, e o dispositivo vai a 100% se o líquido chegar a 60 °C."))
                    .size(11.0)
                    .color(MUTED),
            );
        }
        ui.horizontal(|ui| {
            let changed = draft != applied;
            if ui
                .add_enabled(
                    changed && validation.is_ok(),
                    egui::Button::new(t("Aplicar")).fill(ACCENT),
                )
                .clicked()
            {
                self.dispatch(Request::SetCooling {
                    channel: key(device, &channel.name),
                    cooling: draft.clone(),
                });
            }
            if ui
                .add_enabled(changed, egui::Button::new(t("Descartar")))
                .clicked()
            {
                *draft = applied.clone();
            }
            let others: Vec<&CoolingChannel> = device
                .cooling
                .iter()
                .filter(|c| c.name != channel.name && c.kind == channel.kind)
                .collect();
            if !others.is_empty() {
                ui.menu_button(t("Aplicar curva a outros canais"), |ui| {
                    for other in others {
                        if ui.button(channel_label(&other.name)).clicked() {
                            self.dispatch(Request::SetCooling {
                                channel: key(device, &other.name),
                                cooling: draft.clone(),
                            });
                            ui.close();
                        }
                    }
                });
            }
        });
    }

    fn lighting_page(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        if state.devices.iter().all(|d| d.lighting.is_empty()) {
            card(ui, &t("Iluminação"), |ui| {
                ui.label(t(
                    "Nenhum dispositivo de iluminação compatível foi encontrado",
                ));
            });
            return;
        }
        for device in &state.devices {
            for channel in &device.lighting {
                let key = key(device, &channel.name);
                let applied = state.config.lighting.get(&key).cloned().unwrap_or_default();
                let mut draft = self
                    .lighting
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| applied.clone());
                let title = format!("{} · {}", device.name, channel_label(&channel.name));
                card(ui, &title, |ui| {
                    self.lighting_channel(ui, device, channel, &mut draft, &applied);
                });
                self.lighting.insert(key, draft);
                ui.add_space(6.0);
            }
        }
    }

    fn lighting_channel(
        &self,
        ui: &mut egui::Ui,
        device: &DeviceState,
        channel: &LightingChannel,
        draft: &mut ChannelLighting,
        applied: &ChannelLighting,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("Modo")).color(MUTED));
            egui::ComboBox::from_id_salt(("mode", &device.info.id, &channel.name))
                .selected_text(t(draft.mode.label()))
                .show_ui(ui, |ui| {
                    for capability in &channel.capabilities {
                        if ui
                            .selectable_label(
                                draft.mode == capability.mode,
                                t(capability.mode.label()),
                            )
                            .clicked()
                        {
                            draft.mode = capability.mode;
                            draft.colors.truncate(capability.max_colors);
                            while draft.colors.len() < capability.min_colors {
                                draft.colors.push(Rgb(0x7f, 0x00, 0xff));
                            }
                            if !capability.direction {
                                draft.backwards = false;
                            }
                        }
                    }
                });
        });
        let Some(capability) = channel.capabilities.iter().find(|c| c.mode == draft.mode) else {
            ui.colored_label(WARN, t("Modo não suportado neste canal"));
            return;
        };
        if capability.max_colors > 0 {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t("Cores")).color(MUTED));
                let mut remove = None;
                for (index, color) in draft.colors.iter_mut().enumerate() {
                    let mut rgb = [color.0, color.1, color.2];
                    let response = ui.color_edit_button_srgb(&mut rgb);
                    *color = Rgb(rgb[0], rgb[1], rgb[2]);
                    if response.secondary_clicked() && index >= capability.min_colors {
                        remove = Some(index);
                    }
                }
                if let Some(index) = remove {
                    draft.colors.remove(index);
                }
                if draft.colors.len() < capability.max_colors && ui.button("+").clicked() {
                    draft.colors.push(Rgb(0xff, 0xff, 0xff));
                }
                if draft.colors.len() > capability.min_colors.max(1) && ui.button("−").clicked() {
                    draft.colors.pop();
                }
            });
        }
        if capability.speed {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("Velocidade")).color(MUTED));
                for speed in Speed::ALL {
                    ui.selectable_value(&mut draft.speed, speed, t(speed.label()));
                }
            });
        }
        if capability.direction {
            ui.checkbox(&mut draft.backwards, t("Direção inversa"));
        }
        if matches!(draft.mode, Mode::Marquee | Mode::CoveringMarquee) {
            ui.add(
                egui::Slider::new(&mut draft.led_size, 3..=6).text(t("Tamanho do grupo de LEDs")),
            );
        }
        let validation = draft.validate(&channel.capabilities);
        if let Err(e) = &validation {
            ui.colored_label(WARN, t(&e.to_string()));
        }
        ui.horizontal(|ui| {
            let changed = draft != applied;
            if ui
                .add_enabled(
                    changed && validation.is_ok(),
                    egui::Button::new(t("Aplicar")).fill(ACCENT),
                )
                .clicked()
            {
                self.dispatch(Request::SetLighting {
                    channel: key(device, &channel.name),
                    lighting: draft.clone(),
                });
            }
            if ui
                .add_enabled(changed, egui::Button::new(t("Descartar")))
                .clicked()
            {
                *draft = applied.clone();
            }
        });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let ui_config = &state.config.ui;
        card(ui, &t("Aplicativo"), |ui| {
            egui::Grid::new("settings")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label(t("Idioma"));
                    egui::ComboBox::from_id_salt("language")
                        .selected_text(&ui_config.language)
                        .show_ui(ui, |ui| {
                            for language in &state.languages {
                                if ui
                                    .selectable_label(&ui_config.language == language, language)
                                    .clicked()
                                {
                                    self.dispatch(Request::UpdateSettings {
                                        language: Some(language.clone()),
                                        temperature_unit: None,
                                        cpu_average_temperature: None,
                                        start_minimized: None,
                                        mini_mode: None,
                                        mini_always_on_top: None,
                                        gpu_index: None,
                                    });
                                }
                            }
                        });
                    ui.end_row();
                    ui.label(t("Exibição de temperatura"));
                    ui.horizontal(|ui| {
                        for (unit, label) in [
                            (TemperatureUnit::Celsius, "Celsius"),
                            (TemperatureUnit::Fahrenheit, "Fahrenheit"),
                        ] {
                            if ui
                                .selectable_label(ui_config.temperature_unit == unit, t(label))
                                .clicked()
                            {
                                self.settings(|r| {
                                    if let Request::UpdateSettings {
                                        temperature_unit, ..
                                    } = r
                                    {
                                        *temperature_unit = Some(unit);
                                    }
                                });
                            }
                        }
                    });
                    ui.end_row();
                    ui.label(t("Temperatura da CPU"));
                    ui.horizontal(|ui| {
                        for (average, label) in [
                            (false, "Temperatura do pacote"),
                            (true, "Média dos núcleos"),
                        ] {
                            if ui
                                .selectable_label(
                                    ui_config.cpu_average_temperature == average,
                                    t(label),
                                )
                                .clicked()
                            {
                                self.settings(|r| {
                                    if let Request::UpdateSettings {
                                        cpu_average_temperature,
                                        ..
                                    } = r
                                    {
                                        *cpu_average_temperature = Some(average);
                                    }
                                });
                            }
                        }
                    });
                    ui.end_row();
                    if state.sensors.gpus.len() > 1 {
                        ui.label(t("GPU de monitoramento"));
                        egui::ComboBox::from_id_salt("gpu")
                            .selected_text(
                                state
                                    .sensors
                                    .gpus
                                    .get(state.config.governor.gpu_index)
                                    .map_or("—".to_owned(), |g| g.name.clone()),
                            )
                            .show_ui(ui, |ui| {
                                for (index, gpu) in state.sensors.gpus.iter().enumerate() {
                                    if ui
                                        .selectable_label(
                                            state.config.governor.gpu_index == index,
                                            &gpu.name,
                                        )
                                        .clicked()
                                    {
                                        self.settings(|r| {
                                            if let Request::UpdateSettings { gpu_index, .. } = r {
                                                *gpu_index = Some(index);
                                            }
                                        });
                                    }
                                }
                            });
                        ui.end_row();
                    }
                });
            let mut autostart = state.autostart;
            if ui
                .checkbox(&mut autostart, t("Iniciar o NZXT CAM com a sessão"))
                .changed()
            {
                self.dispatch(Request::SetAutostart { enabled: autostart });
            }
            let mut minimized = ui_config.start_minimized;
            if ui
                .checkbox(&mut minimized, t("Iniciar minimizado na bandeja"))
                .changed()
            {
                self.settings(|r| {
                    if let Request::UpdateSettings {
                        start_minimized, ..
                    } = r
                    {
                        *start_minimized = Some(minimized);
                    }
                });
            }
            let mut on_top = ui_config.mini_always_on_top;
            if ui
                .checkbox(&mut on_top, t("Modo Mini sempre no topo"))
                .changed()
            {
                self.settings(|r| {
                    if let Request::UpdateSettings {
                        mini_always_on_top, ..
                    } = r
                    {
                        *mini_always_on_top = Some(on_top);
                    }
                });
            }
        });
        ui.add_space(6.0);
        card(ui, &t("Acesso aos dispositivos"), |ui| {
            self.permissions_section(ui, state)
        });
        ui.add_space(6.0);
        card(ui, &t("Sobre"), |ui| {
            metric(ui, &t("Versão"), env!("CARGO_PKG_VERSION").into());
            ui.label(
                RichText::new(t("Projeto independente, sem afiliação com a NZXT. Referência: NZXT CAM 4.76.8 para Windows."))
                    .size(11.0)
                    .color(MUTED),
            );
        });
    }

    fn settings(&self, set: impl FnOnce(&mut Request)) {
        let mut request = Request::UpdateSettings {
            language: None,
            temperature_unit: None,
            cpu_average_temperature: None,
            start_minimized: None,
            mini_mode: None,
            mini_always_on_top: None,
            gpu_index: None,
        };
        set(&mut request);
        self.dispatch(request);
    }

    fn permissions_section(&self, ui: &mut egui::Ui, state: &Snapshot) {
        let permissions = &state.permissions;
        if permissions.rule_installed && !permissions.rule_outdated {
            ui.colored_label(
                GOOD,
                t("Regra udev instalada: a sessão acessa os dispositivos sem root"),
            );
        } else if permissions.rule_outdated {
            ui.colored_label(
                WARN,
                t("A regra udev instalada é de outra versão; prepare novamente"),
            );
        } else {
            ui.label(t(
                "Sem a regra udev, os dispositivos só podem ser acessados como root.",
            ));
        }
        ui.label(
            RichText::new(trf(
                "Arquivo: {caminho}",
                &[("caminho", permissions::RULE_PATH)],
            ))
            .size(11.0)
            .color(MUTED),
        );
        if permissions.running {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(t("Aguardando autorização do Ubuntu"));
            });
        } else if let Some(plan) = &permissions.plan {
            ui.label(RichText::new(t("Plano revisável — nada foi alterado ainda:")).strong());
            for step in &plan.steps {
                ui.monospace(step);
            }
            if let Some(rule) = &plan.rule {
                egui::CollapsingHeader::new(t("Conteúdo da regra")).show(ui, |ui| {
                    ui.monospace(rule);
                });
            }
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new(t("Confirmar e autenticar")).fill(ACCENT))
                    .clicked()
                {
                    self.dispatch(Request::ConfirmPlan { id: plan.id });
                }
                if ui.button(t("Cancelar")).clicked() {
                    self.dispatch(Request::CancelPlan);
                }
            });
        } else {
            ui.horizontal(|ui| {
                if ui.button(t("Preparar permissões…")).clicked() {
                    self.dispatch(Request::PreparePermissions {
                        action: permissions::Action::Install,
                    });
                }
                if permissions.rule_installed && ui.button(t("Remover regra…")).clicked() {
                    self.dispatch(Request::PreparePermissions {
                        action: permissions::Action::Remove,
                    });
                }
            });
        }
        if let Some(error) = &permissions.error {
            ui.colored_label(WARN, t(error));
        }
    }

    fn mini_view(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let unit = state.config.ui.temperature_unit;
        ui.horizontal(|ui| {
            ui.label(RichText::new("NZXT CAM").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("⤢")
                    .on_hover_text(t("Sair do Modo Mini"))
                    .clicked()
                {
                    let ctx = ui.ctx().clone();
                    self.set_mini(&ctx, false, false);
                    self.settings(|r| {
                        if let Request::UpdateSettings { mini_mode, .. } = r {
                            *mini_mode = Some(false);
                        }
                    });
                }
            });
        });
        let cpu = state
            .sensors
            .cpu_temperature(state.config.ui.cpu_average_temperature);
        metric(
            ui,
            "CPU",
            format!(
                "{} · {}",
                unit.format(cpu),
                percent(state.sensors.cpu.load_percent)
            ),
        );
        for gpu in &state.sensors.gpus {
            metric(
                ui,
                "GPU",
                format!(
                    "{} · {}",
                    unit.format(gpu.temperature),
                    percent(gpu.load_percent)
                ),
            );
        }
        for device in &state.devices {
            if let Some(liquid) = device
                .last_status
                .as_ref()
                .and_then(|s| s.liquid_temperature)
            {
                metric(ui, &t("Líquido"), unit.format(Some(liquid)));
            }
        }
    }
}

/// Editor da curva: arrastar move pontos, duplo clique adiciona, botão direito remove.
#[allow(clippy::too_many_arguments)]
fn curve_editor(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    points: &mut Vec<Point>,
    current: Option<f64>,
    editable: bool,
    limits: crate::curves::Limits,
    source: Source,
) -> bool {
    let size = Vec2::new(ui.available_width().min(640.0), 180.0);
    let (response, painter) = ui.allocate_painter(size, egui::Sense::click());
    let rect = response.rect.shrink2(Vec2::new(30.0, 14.0));
    let to_screen = |p: &Point| {
        egui::pos2(
            rect.left() + rect.width() * (p.temperature as f32 / 100.0),
            rect.bottom() - rect.height() * (f32::from(p.duty) / 100.0),
        )
    };
    let from_screen = |pos: egui::Pos2| {
        let temperature = (((pos.x - rect.left()) / rect.width()) * 100.0).clamp(0.0, 100.0);
        let duty = (((rect.bottom() - pos.y) / rect.height()) * 100.0).clamp(0.0, 100.0);
        (f64::from(temperature.round()), duty.round() as u8)
    };
    painter.rect_filled(response.rect, 4, CANVAS);
    for step in 0..=5 {
        let fraction = step as f32 / 5.0;
        let y = rect.bottom() - rect.height() * fraction;
        let x = rect.left() + rect.width() * fraction;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0, BORDER),
        );
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(1.0, BORDER),
        );
        painter.text(
            egui::pos2(rect.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{}%", step * 20),
            egui::FontId::proportional(9.0),
            MUTED,
        );
        painter.text(
            egui::pos2(x, rect.bottom() + 2.0),
            egui::Align2::CENTER_TOP,
            format!("{}°", step * 20),
            egui::FontId::proportional(9.0),
            MUTED,
        );
    }
    let floor_y = rect.bottom() - rect.height() * (f32::from(limits.floor) / 100.0);
    painter.line_segment(
        [
            egui::pos2(rect.left(), floor_y),
            egui::pos2(rect.right(), floor_y),
        ],
        Stroke::new(1.0, WARN.gamma_multiply(0.5)),
    );
    let critical_x = rect.left() + rect.width() * (source.critical() as f32 / 100.0);
    painter.line_segment(
        [
            egui::pos2(critical_x, rect.top()),
            egui::pos2(critical_x, rect.bottom()),
        ],
        Stroke::new(1.0, WARN.gamma_multiply(0.5)),
    );
    let mut changed = false;
    if editable {
        let mut remove = None;
        for index in 0..points.len() {
            let center = to_screen(&points[index]);
            let handle = egui::Rect::from_center_size(center, Vec2::splat(14.0));
            let point_response = ui.interact(
                handle,
                ui.id().with((&id, index)),
                egui::Sense::click_and_drag(),
            );
            if point_response.dragged()
                && let Some(pos) = point_response.interact_pointer_pos()
            {
                let (temperature, duty) = from_screen(pos);
                let min = if index == 0 {
                    0.0
                } else {
                    points[index - 1].temperature + 1.0
                };
                let max = points.get(index + 1).map_or(100.0, |p| p.temperature - 1.0);
                points[index] = Point {
                    temperature: temperature.clamp(min, max.max(min)),
                    duty,
                };
                changed = true;
            }
            if point_response.secondary_clicked() && points.len() > 2 {
                remove = Some(index);
            }
        }
        if let Some(index) = remove {
            points.remove(index);
            changed = true;
        }
        if response.double_clicked()
            && points.len() < crate::curves::MAX_POINTS
            && let Some(pos) = response.interact_pointer_pos()
        {
            let (temperature, duty) = from_screen(pos);
            if !points
                .iter()
                .any(|p| (p.temperature - temperature).abs() < 1.0)
            {
                points.push(Point { temperature, duty });
                points.sort_by(|a, b| a.temperature.total_cmp(&b.temperature));
                changed = true;
            }
        }
    }
    let line: Vec<egui::Pos2> = points.iter().map(&to_screen).collect();
    painter.add(egui::Shape::line(line.clone(), Stroke::new(2.0, ACCENT)));
    for position in line {
        painter.circle_filled(position, if editable { 5.0 } else { 3.0 }, TEXT);
    }
    if let Some(temperature) = current.filter(|t| t.is_finite()) {
        let duty = curves::duty_at(points, temperature);
        let position = to_screen(&Point {
            temperature: temperature.clamp(0.0, 100.0),
            duty,
        });
        painter.circle_stroke(position, 7.0, Stroke::new(2.0, GOOD));
    }
    if editable {
        ui.label(
            RichText::new(t(
                "Arraste os pontos; duplo clique adiciona, botão direito remove.",
            ))
            .size(11.0)
            .color(MUTED),
        );
    }
    changed
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let state = self.state();
        if state.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if state.show_requests != self.show_requests {
            self.show_requests = state.show_requests;
            self.hidden = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if state.config.ui.mini_mode != self.mini {
            self.set_mini(
                ctx,
                state.config.ui.mini_mode,
                state.config.ui.mini_always_on_top,
            );
        }
        #[cfg(all(feature = "tray", target_os = "linux"))]
        if let Some(tray) = &self.tray {
            let language = state.config.ui.language.clone();
            tray.update(|tray| tray.language = language);
        }
        if ctx.input(|i| i.viewport().close_requested()) && !state.quit {
            #[cfg(all(feature = "tray", target_os = "linux"))]
            if self.tray.is_some() {
                // Com bandeja, fechar esconde a janela e mantém o governador ativo.
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.hidden = true;
                return;
            }
            self.dispatch(Request::Quit);
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        let state = self.state();
        if self.mini {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(CANVAS).inner_margin(10))
                .show(root, |ui| self.mini_view(ui, &state));
            return;
        }
        egui::Panel::top("title")
            .frame(egui::Frame::new().fill(CANVAS).inner_margin(14))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("NZXT CAM").size(18.0).strong());
                    ui.label(RichText::new("Linux").size(11.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("⟳")
                            .on_hover_text(t("Atualizar dispositivos"))
                            .clicked()
                        {
                            self.dispatch(Request::Refresh);
                        }
                        if ui.button("▣").on_hover_text(t("Modo Mini")).clicked() {
                            self.settings(|r| {
                                if let Request::UpdateSettings { mini_mode, .. } = r {
                                    *mini_mode = Some(true);
                                }
                            });
                        }
                    });
                });
            });
        egui::Panel::bottom("status")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(8))
            .show(root, |ui| {
                if let Some(error) = &state.error {
                    ui.colored_label(WARN, t(error));
                } else {
                    ui.label(RichText::new(t(&state.message)).size(11.0).color(MUTED));
                }
            });
        egui::Panel::left("navigation")
            .exact_size(190.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(10))
            .show(root, |ui| {
                for page in Page::ALL {
                    let selected = self.page == page;
                    let button = egui::Button::new(RichText::new(t(page.label())).size(14.0))
                        .fill(if selected {
                            ACCENT
                        } else {
                            Color32::TRANSPARENT
                        })
                        .min_size(Vec2::new(ui.available_width(), 34.0));
                    if ui.add(button).clicked() {
                        self.page = page;
                    }
                }
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(CANVAS).inner_margin(14))
            .show(root, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Monitoring => self.monitoring(ui, &state),
                        Page::Cooling => self.cooling_page(ui, &state),
                        Page::Lighting => self.lighting_page(ui, &state),
                        Page::Settings => self.settings_page(ui, &state),
                    });
            });
    }
}
