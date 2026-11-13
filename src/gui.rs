//! Interface no layout do NZXT CAM 4.76.8: barra superior, menu lateral, coluna
//! central de cartões. Cores e medidas vêm do tema do CAM (`lib/colors.ts`) e das
//! telas capturadas; os ícones são desenhos próprios, sem recursos da NZXT.
#[cfg(unix)]
use crate::ipc::Controller;
#[cfg(not(unix))]
use crate::preview::Controller;
use crate::{
    config::TemperatureUnit,
    curves::{self, ChannelCooling, Point, Profile, Source},
    driver::{ChannelKind, CoolingChannel, LightingChannel},
    engine::{DeviceState, DeviceStatus, Request, Snapshot},
    i18n::{tr as t, trf},
    lighting::{ChannelLighting, Mode, Rgb, Speed},
    permissions,
};
use anyhow::Result;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::{HashMap, HashSet};
#[path = "gui_profiles.rs"]
mod profile_ui;
use profile_ui::ProfileDialog;

// Tema escuro do CAM.
const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
const DARKEST: Color32 = Color32::from_rgb(18, 18, 20);
const BACKGROUND: Color32 = Color32::from_rgb(26, 28, 30);
const BORDER: Color32 = Color32::from_rgb(52, 56, 60);
const CONTROL: Color32 = Color32::from_rgb(73, 72, 81);
const MEDIUM: Color32 = Color32::from_rgb(104, 103, 117);
const NAV: Color32 = Color32::from_rgb(94, 92, 110);
const SECONDARY: Color32 = Color32::from_rgb(159, 169, 178);
const TEXT: Color32 = Color32::from_rgb(232, 235, 237);
const PURPLE: Color32 = Color32::from_rgb(100, 10, 146);
const ORCHID: Color32 = Color32::from_rgb(191, 76, 191);
const GREEN: Color32 = Color32::from_rgb(33, 211, 115);
const ORANGE: Color32 = Color32::from_rgb(250, 165, 11);

const CONTENT_WIDTH: f32 = 968.0;
const DROPDOWN_WIDTH: f32 = 210.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Monitoring,
    Specs,
    Lighting,
    Cooling,
    Settings,
}

impl Page {
    const MAIN: [Self; 4] = [Self::Monitoring, Self::Specs, Self::Lighting, Self::Cooling];

    fn label(self) -> &'static str {
        match self {
            Self::Monitoring => "PC Monitoring",
            Self::Specs => "Especificações do sistema",
            Self::Lighting => "Iluminação",
            Self::Cooling => "Resfriando",
            Self::Settings => "Configurações",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Monitoring => Icon::Chart,
            Self::Specs => Icon::Eye,
            Self::Lighting => Icon::Sun,
            Self::Cooling => Icon::Fan,
            Self::Settings => Icon::Gear,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Profiles,
    Firmware,
    Access,
    About,
}

impl Section {
    const ALL: [Self; 5] = [
        Self::General,
        Self::Firmware,
        Self::Access,
        Self::Profiles,
        Self::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::General => "Geral",
            Self::Profiles => "Profile Sync",
            Self::Firmware => "NZXT Firmware",
            Self::Access => "Acesso aos dispositivos",
            Self::About => "Sobre",
        }
    }
}

fn page_visible(page: Page, panels: &crate::config::Panels) -> bool {
    match page {
        Page::Monitoring => panels.monitoring,
        Page::Specs => panels.specs,
        Page::Lighting => panels.lighting,
        Page::Cooling => panels.cooling,
        Page::Settings => true,
    }
}

fn backup_path() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
        .join("nzxt-cam-backup.json")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Icon {
    Chart,
    Eye,
    Sun,
    Fan,
    Gear,
    Pump,
    Ring,
    Logo,
    Strip,
    Refresh,
    ArrowUp,
    ArrowDown,
    ChevronUp,
    ChevronDown,
}

/// Ícones próprios, traçados com primitivas (sem arte da NZXT).
fn draw_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) / 2.0;
    let stroke = Stroke::new((r / 7.0).max(1.2), color);
    let arc = |radius: f32, from: f32, to: f32| -> Vec<Pos2> {
        (0..=24)
            .map(|i| {
                let a = from + (to - from) * i as f32 / 24.0;
                c + Vec2::new(a.cos(), a.sin()) * radius
            })
            .collect()
    };
    match icon {
        Icon::ArrowUp | Icon::ArrowDown | Icon::ChevronUp | Icon::ChevronDown => {
            let down = matches!(icon, Icon::ArrowDown | Icon::ChevronDown);
            let direction = if down { 1.0 } else { -1.0 };
            let tip = c + Vec2::new(0.0, r * 0.65 * direction);
            painter.add(egui::Shape::line(
                vec![
                    c + Vec2::new(-r * 0.55, -r * 0.1 * direction),
                    tip,
                    c + Vec2::new(r * 0.55, -r * 0.1 * direction),
                ],
                stroke,
            ));
            if matches!(icon, Icon::ArrowUp | Icon::ArrowDown) {
                painter.line_segment([tip, c - Vec2::new(0.0, r * 0.8 * direction)], stroke);
            }
        }
        Icon::Chart => {
            let p = |x: f32, y: f32| c + Vec2::new(x, y) * r;
            painter.line_segment([p(-0.9, -0.9), p(-0.9, 0.9)], stroke);
            painter.line_segment([p(-0.9, 0.9), p(0.9, 0.9)], stroke);
            painter.add(egui::Shape::line(
                vec![p(-0.7, 0.4), p(-0.2, -0.2), p(0.2, 0.2), p(0.85, -0.6)],
                stroke,
            ));
        }
        Icon::Eye => {
            let mut top: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let x = -1.0 + 2.0 * i as f32 / 24.0;
                    c + Vec2::new(x * r, -(1.0 - x * x) * r * 0.6)
                })
                .collect();
            let bottom: Vec<Pos2> = top
                .iter()
                .rev()
                .map(|p| Pos2::new(p.x, 2.0 * c.y - p.y))
                .collect();
            top.extend(bottom);
            painter.add(egui::Shape::closed_line(top, stroke));
            painter.circle_stroke(c, r * 0.28, stroke);
        }
        Icon::Sun => {
            painter.circle_stroke(c, r * 0.42, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = Vec2::new(a.cos(), a.sin());
                painter.line_segment([c + d * r * 0.62, c + d * r * 0.95], stroke);
            }
        }
        Icon::Fan => {
            painter.circle_stroke(c, r * 0.92, stroke);
            for i in 0..3 {
                let a = i as f32 * std::f32::consts::TAU / 3.0;
                painter.add(egui::Shape::line(arc(r * 0.55, a, a + 1.4), stroke));
                painter.line_segment([c, c + Vec2::new(a.cos(), a.sin()) * r * 0.55], stroke);
            }
        }
        Icon::Gear => {
            painter.circle_stroke(c, r * 0.55, stroke);
            painter.circle_stroke(c, r * 0.2, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = Vec2::new(a.cos(), a.sin());
                painter.line_segment([c + d * r * 0.55, c + d * r * 0.92], stroke);
            }
        }
        Icon::Pump => {
            painter.circle_stroke(c, r * 0.9, stroke);
            painter.circle_stroke(c, r * 0.5, stroke);
            painter.circle_filled(c, r * 0.18, color);
        }
        Icon::Ring => {
            painter.circle_stroke(c, r * 0.75, Stroke::new(r * 0.3, color));
        }
        Icon::Logo => {
            painter.circle_stroke(c, r * 0.85, stroke);
            painter.rect_filled(Rect::from_center_size(c, Vec2::new(r, r * 0.35)), 1, color);
        }
        Icon::Strip => {
            for i in 0..6 {
                let x = -0.8 + i as f32 * 0.32;
                painter.circle_filled(c + Vec2::new(x * r, 0.0), r * 0.1, color);
            }
        }
        Icon::Refresh => {
            let points = arc(r * 0.75, 0.5, 5.6);
            let end = *points.last().unwrap_or(&c);
            painter.add(egui::Shape::line(points, stroke));
            painter.line_segment([end, end + Vec2::new(r * 0.35, -r * 0.1)], stroke);
            painter.line_segment([end, end + Vec2::new(r * 0.05, r * 0.38)], stroke);
        }
    }
}

/// Ícone do aplicativo e da bandeja: anel e ponto no roxo do tema.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    let center = (size as f32 - 1.0) / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt() / center;
            if (0.62..=0.95).contains(&d) || d <= 0.28 {
                out.extend_from_slice(&[ORCHID.r(), ORCHID.g(), ORCHID.b(), 255]);
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
            .with_title(if cfg!(unix) {
                "NZXT CAM"
            } else {
                "NZXT CAM — Prévia (dados simulados)"
            })
            .with_app_id(crate::desktop::APP_ID)
            .with_icon(egui::IconData {
                rgba: icon_rgba(64),
                width: 64,
                height: 64,
            })
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([300.0, 190.0]),
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
    section: Section,
    cooling: HashMap<String, ChannelCooling>,
    lighting: HashMap<String, ChannelLighting>,
    expanded: Option<String>,
    expanded_lighting: HashSet<String>,
    spec_detail: String,
    process_sort: usize,
    process_descending: bool,
    profile_dialog: Option<ProfileDialog>,
    last_profile: Option<(String, String, String)>,
    show_requests: u64,
    #[cfg(all(feature = "tray", target_os = "linux"))]
    tray: Option<ksni::blocking::Handle<crate::tray::Tray>>,
}

fn key(device: &DeviceState, channel: &str) -> String {
    format!("{}/{channel}", device.info.id)
}

fn channel_label(channel: &str) -> String {
    match channel {
        "pump" => t("Bomba"),
        "fan" => t("Ventoinha"),
        "ring" => t("Anel"),
        "logo" => t("Logótipo"),
        "led" => t("Fita"),
        other => match other.strip_prefix("fan") {
            Some(n) => trf("Conector da ventoinha {n}", &[("n", n)]),
            None => other.to_owned(),
        },
    }
}

fn channel_icon(channel: &str) -> Icon {
    match channel {
        "pump" => Icon::Pump,
        "ring" => Icon::Ring,
        "logo" => Icon::Logo,
        "led" => Icon::Strip,
        _ => Icon::Fan,
    }
}

fn source_label(source: Source) -> String {
    t(match source {
        Source::Liquid => "Líquido",
        Source::Cpu => "CPU",
        Source::Gpu => "GPU",
    })
}

fn profile_label(profile: &Profile) -> String {
    t(match profile {
        Profile::Silent => "Silencioso",
        Profile::Performance => "Desempenho",
        Profile::Fixed { .. } => "Fixo",
        Profile::Custom { .. } => "Personalizado",
    })
}

fn mode_label(mode: Mode) -> String {
    // Nomes do menu do CAM 4.76.8 em português.
    t(match mode {
        Mode::Off => "Desligado",
        Mode::Fixed => "Fixo",
        Mode::Breathing => "Respirar",
        Mode::Fading => "Desvanecer",
        Mode::Marquee => "Painel",
        Mode::CoveringMarquee => "Painel rolante",
        Mode::Pulse => "Pulsar",
        Mode::SpectrumWave => "Onda de espectro",
        Mode::Alternating => "Alternado",
        Mode::Wings => "Asas",
        Mode::TaiChi => "Tai Chi",
        Mode::WaterCooler => "Refrigerador de água",
        Mode::Loading => "A carregar",
        Mode::Candle => "Vela",
        other => other.label(),
    })
}

/// Cartão com borda de 1 px, como os do CAM.
fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(BACKGROUND)
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(18.0).strong().color(TEXT));
}

fn page_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(24.0).color(TEXT));
}

fn label_value(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(14.0).color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(14.0).color(TEXT));
        });
    });
}

/// Barra fina com rótulo e valor, como CPU/GPU no PC Monitoring.
fn meter(ui: &mut egui::Ui, label: &str, value: &str, fraction: Option<f64>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, BORDER);
    if let Some(fraction) = fraction {
        let mut fill = rect;
        fill.set_width(rect.width() * fraction.clamp(0.0, 1.0) as f32);
        ui.painter().rect_filled(fill, 0, MEDIUM);
    }
    label_value(ui, label, value);
    ui.add_space(6.0);
}

/// Medidor circular de 270°, com o valor no centro e o rótulo abaixo.
fn gauge(ui: &mut egui::Ui, radius: f32, fraction: Option<f64>, label: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(radius * 2.0 + 8.0), Sense::hover());
    let c = rect.center();
    let start = 0.75 * std::f32::consts::PI;
    let sweep = 1.5 * std::f32::consts::PI;
    let points = |to: f32| -> Vec<Pos2> {
        (0..=48)
            .map(|i| {
                let a = start + (to - start) * i as f32 / 48.0;
                c + Vec2::new(a.cos(), a.sin()) * radius
            })
            .collect()
    };
    let painter = ui.painter();
    painter.add(egui::Shape::line(
        points(start + sweep),
        Stroke::new(6.0, BORDER),
    ));
    if let Some(fraction) = fraction {
        let end = start + sweep * fraction.clamp(0.0, 1.0) as f32;
        painter.add(egui::Shape::line(points(end), Stroke::new(6.0, MEDIUM)));
    }
    let text = fraction.map_or("—".into(), |f| format!("{:.0}%", f * 100.0));
    painter.text(
        c,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(30.0),
        TEXT,
    );
    painter.text(
        c + Vec2::new(0.0, radius * 0.75),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(15.0),
        SECONDARY,
    );
}

fn rate(value: Option<f64>) -> String {
    match value {
        None => "—".into(),
        Some(v) if v >= 1e6 => format!("{:.1}MB/s", v / 1e6),
        Some(v) if v >= 1e3 => format!("{:.0}KB/s", v / 1e3),
        Some(v) => format!("{v:.0}B/s"),
    }
}

fn size(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 1000.0 {
        format!("{:.1}TB", gb / 1000.0)
    } else {
        format!("{gb:.0}GB")
    }
}

/// Seletor no estilo do CAM: retângulo `CONTROL`, texto à esquerda, seta à direita.
fn dropdown<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    text: &str,
    width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(RichText::new(text).size(13.0).color(TEXT))
        .show_ui(ui, add)
        .inner
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, controller: Controller, minimized: bool) -> Self {
        let repaint = cc.egui_ctx.clone();
        *controller.wake.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(Box::new(move || repaint.request_repaint()));
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BACKGROUND;
        style.visuals.window_fill = DARKEST;
        style.visuals.extreme_bg_color = DARKEST;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = PURPLE;
        style.visuals.hyperlink_color = ORCHID;
        for widgets in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widgets.corner_radius = egui::CornerRadius::same(2);
            widgets.bg_stroke = Stroke::NONE;
        }
        style.visuals.widgets.inactive.weak_bg_fill = CONTROL;
        style.visuals.widgets.inactive.bg_fill = CONTROL;
        style.visuals.widgets.hovered.weak_bg_fill = MEDIUM;
        style.visuals.widgets.hovered.bg_fill = MEDIUM;
        style.visuals.widgets.active.weak_bg_fill = PURPLE;
        style.visuals.widgets.active.bg_fill = PURPLE;
        style.visuals.widgets.open.weak_bg_fill = CONTROL;
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.spacing.interact_size.y = 36.0;
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
        let app = Self {
            controller,
            page: Page::Monitoring,
            section: Section::General,
            cooling: HashMap::new(),
            lighting: HashMap::new(),
            expanded: None,
            expanded_lighting: HashSet::new(),
            spec_detail: "cpu".into(),
            process_sort: 1,
            process_descending: true,
            profile_dialog: None,
            last_profile: profile_ui::selection(&state),
            show_requests: state.show_requests,
            #[cfg(all(feature = "tray", target_os = "linux"))]
            tray,
        };
        // Campos antigos de Mini Mode continuam legíveis, mas não ativam a interface removida.
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

    fn settings(&self, set: impl FnOnce(&mut Request)) {
        let mut request = Request::UpdateSettings {
            language: None,
            temperature_unit: None,
            cpu_average_temperature: None,
            start_minimized: None,
            mini_mode: None,
            mini_always_on_top: None,
            gpu_index: None,
            close_to_tray: None,
        };
        set(&mut request);
        self.dispatch(request);
    }

    fn update_ui(&self, state: &Snapshot, change: impl FnOnce(&mut crate::config::UiConfig)) {
        let mut ui = state.config.ui.clone();
        change(&mut ui);
        self.dispatch(Request::UpdateUi { ui });
    }

    fn icon_button(&self, ui: &mut egui::Ui, icon: Icon, hint: &str) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
        let color = if response.hovered() { TEXT } else { SECONDARY };
        draw_icon(ui.painter(), rect.shrink(5.0), icon, color);
        response.on_hover_text(hint).clicked()
    }

    fn top_bar(&mut self, root: &mut egui::Ui) {
        let state = self.state();
        egui::Panel::top("title")
            .exact_size(42.0)
            .frame(
                egui::Frame::new()
                    .fill(BLACK)
                    .inner_margin(egui::Margin::symmetric(22, 0)),
            )
            .show(root, |ui| {
                let rect = ui.max_rect();
                ui.painter().text(
                    Pos2::new(rect.left(), rect.center().y),
                    Align2::LEFT_CENTER,
                    "NZXT CAM",
                    FontId::proportional(22.0),
                    TEXT,
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(Rect::from_center_size(
                        rect.center(),
                        Vec2::new(330.0, 36.0),
                    )),
                    |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(t("Perfil ativo:"));
                            let active = state
                                .config
                                .profiles
                                .active
                                .as_deref()
                                .unwrap_or("Personalizado");
                            dropdown(ui, "active-profile", active, 190.0, |ui| {
                                for name in state.config.profiles.sync.keys() {
                                    if ui.selectable_label(active == name, name).clicked() {
                                        self.dispatch(Request::ActivateProfile {
                                            name: name.clone(),
                                        });
                                    }
                                }
                                ui.separator();
                                if ui.button(t("Adicionar / editar perfis")).clicked() {
                                    self.page = Page::Settings;
                                    self.section = Section::Profiles;
                                    ui.close();
                                }
                            });
                        });
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.icon_button(ui, Icon::Refresh, &t("Atualizar dispositivos")) {
                        self.dispatch(Request::Refresh);
                    }
                });
            });
    }

    fn nav_item(
        &self,
        ui: &mut egui::Ui,
        icon: Option<Icon>,
        text: &str,
        selected: bool,
        color: Color32,
    ) -> bool {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::click());
        let color = if selected || response.hovered() {
            TEXT
        } else {
            color
        };
        if let Some(icon) = icon {
            let icon_rect = Rect::from_center_size(
                Pos2::new(rect.left() + 26.0, rect.center().y),
                Vec2::splat(22.0),
            );
            draw_icon(ui.painter(), icon_rect, icon, color);
            if selected {
                ui.painter().circle_filled(
                    Pos2::new(rect.left() + 26.0, rect.center().y + 18.0),
                    1.8,
                    TEXT,
                );
            }
        } else if selected {
            ui.painter()
                .circle_filled(Pos2::new(rect.left() + 26.0, rect.center().y), 2.2, TEXT);
        }
        let font = FontId::proportional(13.0);
        let galley = ui
            .painter()
            .layout(text.to_owned(), font, color, rect.width() - 58.0);
        ui.painter().galley(
            Pos2::new(rect.left() + 54.0, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        response.clicked()
    }

    fn sidebar(&mut self, root: &mut egui::Ui, state: &Snapshot) {
        let panels = &state.config.ui.panels;
        if !page_visible(self.page, panels) {
            self.page = Page::MAIN
                .into_iter()
                .find(|p| page_visible(*p, panels))
                .unwrap_or(Page::Settings);
        }
        egui::Panel::left("navigation")
            .exact_size(180.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(DARKEST)
                    .inner_margin(egui::Margin::symmetric(0, 8)),
            )
            .show(root, |ui| {
                if self.page == Page::Settings {
                    if self.nav_item(ui, None, &format!("‹  {}", t("Voltar")), false, ORCHID) {
                        self.page = Page::Monitoring;
                    }
                    for section in Section::ALL {
                        if self.nav_item(
                            ui,
                            None,
                            &t(section.label()),
                            self.section == section,
                            NAV,
                        ) {
                            self.section = section;
                        }
                    }
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!("version {}", env!("CARGO_PKG_VERSION")))
                                    .size(12.0)
                                    .color(ORCHID),
                            );
                        });
                    });
                    return;
                }
                for page in Page::MAIN.into_iter().filter(|p| page_visible(*p, panels)) {
                    if self.nav_item(
                        ui,
                        Some(page.icon()),
                        &t(page.label()),
                        self.page == page,
                        NAV,
                    ) {
                        self.page = page;
                        self.expanded = None;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if self.nav_item(ui, Some(Icon::Gear), &t("Configurações"), false, NAV) {
                        self.page = Page::Settings;
                    }
                    let (line, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().rect_filled(line, 0, BORDER);
                });
            });
    }

    // ---------- PC Monitoring ----------

    fn monitoring(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let unit = state.config.ui.temperature_unit;
        let sensors = &state.sensors;
        let pump_rpm = state
            .devices
            .iter()
            .filter_map(|d| d.last_status.as_ref())
            .flat_map(|s| s.fans.iter())
            .find(|f| f.channel == "pump")
            .and_then(|f| f.rpm);
        let half = (ui.available_width() - 16.0) / 2.0;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(half);
                card(ui, |ui| {
                    title(ui, "CPU");
                    ui.horizontal(|ui| {
                        ui.add_space(50.0);
                        gauge(
                            ui,
                            62.0,
                            sensors.cpu.load_percent.map(|v| v / 100.0),
                            &t("Carga"),
                        );
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            ui.add_space(14.0);
                            ui.set_width(150.0);
                            let temperature =
                                sensors.cpu_temperature(state.config.ui.cpu_average_temperature);
                            meter(
                                ui,
                                &t("Temperatura"),
                                &unit.format(temperature),
                                temperature.map(|v| v / 100.0),
                            );
                            meter(
                                ui,
                                &t("Clock"),
                                &sensors
                                    .cpu
                                    .clock_mhz
                                    .map_or("—".into(), |v| format!("{v:.0}MHz")),
                                sensors
                                    .cpu
                                    .clock_mhz
                                    .zip(state.specs.cpu.max_mhz)
                                    .map(|(v, m)| v / m),
                            );
                            meter(
                                ui,
                                &t("Ventilador"),
                                &pump_rpm.map_or("—".into(), |v| format!("{v}RPM")),
                                pump_rpm.map(|v| f64::from(v) / 3000.0),
                            );
                        });
                    });
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_width(half);
                card(ui, |ui| {
                    title(ui, "GPU");
                    let gpu = sensors.gpus.get(state.config.governor.gpu_index);
                    ui.horizontal(|ui| {
                        ui.add_space(50.0);
                        gauge(
                            ui,
                            62.0,
                            gpu.and_then(|g| g.load_percent).map(|v| v / 100.0),
                            &t("Carga"),
                        );
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            ui.add_space(14.0);
                            ui.set_width(150.0);
                            let temperature = gpu.and_then(|g| g.temperature);
                            meter(
                                ui,
                                &t("Temperatura"),
                                &unit.format(temperature),
                                temperature.map(|v| v / 100.0),
                            );
                            let clock = gpu.and_then(|g| g.clock_mhz);
                            meter(
                                ui,
                                &t("Clock"),
                                &clock.map_or("—".into(), |v| format!("{v:.0}MHz")),
                                clock.map(|v| v / 3000.0),
                            );
                            let fan = gpu.and_then(|g| g.fan_percent);
                            meter(
                                ui,
                                &t("Ventilador"),
                                &fan.map_or("—".into(), |v| format!("{v:.0}%")),
                                fan.map(|v| v / 100.0),
                            );
                        });
                    });
                });
            });
        });
        ui.add_space(8.0);
        let third = (ui.available_width() - 32.0) / 3.0;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(third);
                card(ui, |ui| {
                    title(ui, &t("RAM"));
                    ui.horizontal(|ui| {
                        ui.add_space(60.0);
                        let fraction = sensors
                            .memory
                            .as_ref()
                            .map(|m| m.used_bytes as f64 / m.total_bytes.max(1) as f64);
                        gauge(ui, 50.0, fraction, &t("Carga"));
                    });
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_width(third);
                card(ui, |ui| {
                    title(ui, &t("Rede"));
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        ui.add_space(8.0);
                        for (value, icon) in [
                            (sensors.network.transmit_bytes_per_second, Icon::ArrowUp),
                            (sensors.network.receive_bytes_per_second, Icon::ArrowDown),
                        ] {
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(18.0, 28.0),
                                            Sense::hover(),
                                        );
                                        draw_icon(ui.painter(), rect, icon, MEDIUM);
                                        ui.label(RichText::new(rate(value)).size(26.0));
                                    },
                                );
                            });
                        }
                        ui.add_space(14.0);
                    });
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_width(third);
                card(ui, |ui| {
                    title(ui, &t("Armazenamento"));
                    for disk in sensors.disks.iter().take(4) {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&disk.mount).size(16.0).color(SECONDARY));
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 24.0),
                                Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 0, DARKEST);
                            let mut fill = rect;
                            fill.set_width(
                                rect.width()
                                    * (disk.used_bytes as f32 / disk.total_bytes.max(1) as f32),
                            );
                            ui.painter().rect_filled(fill, 0, MEDIUM);
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                format!("{} / {}", size(disk.used_bytes), size(disk.total_bytes)),
                                FontId::proportional(11.0),
                                TEXT,
                            );
                        });
                    }
                    if sensors.disks.is_empty() {
                        ui.label(RichText::new("—").color(SECONDARY));
                    }
                });
            });
        });
        ui.add_space(8.0);
        let mut processes: Vec<_> = sensors.processes.iter().collect();
        processes.sort_by(|a, b| {
            let order = match self.process_sort {
                0 => a.name.cmp(&b.name),
                1 => a.cpu_percent.total_cmp(&b.cpu_percent),
                2 => a
                    .gpu_percent
                    .partial_cmp(&b.gpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal),
                3 => a.ram_bytes.cmp(&b.ram_bytes),
                4 => a
                    .download_bytes_per_second
                    .partial_cmp(&b.download_bytes_per_second)
                    .unwrap_or(std::cmp::Ordering::Equal),
                _ => a
                    .upload_bytes_per_second
                    .partial_cmp(&b.upload_bytes_per_second)
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            if self.process_descending {
                order.reverse()
            } else {
                order
            }
        });
        let widths =
            [0.43, 0.10, 0.10, 0.12, 0.125, 0.125].map(|fraction| fraction * ui.available_width());
        egui::Frame::new()
            .stroke(Stroke::new(1.0, BORDER))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let headings = [
                    t("Principais processos"),
                    "CPU".into(),
                    "GPU".into(),
                    "RAM".into(),
                    t("Baixar"),
                    t("Fazer upload"),
                ];
                let mut chosen = None;
                ui.horizontal(|ui| {
                    for (index, heading) in headings.iter().enumerate() {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(widths[index], 34.0), Sense::click());
                        response
                            .clone()
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let right = index != 0;
                        let sorted = self.process_sort == index;
                        let x = if right {
                            rect.right() - 10.0 - if sorted { 16.0 } else { 0.0 }
                        } else {
                            rect.left() + 10.0
                        };
                        ui.painter().text(
                            Pos2::new(x, rect.center().y),
                            if right {
                                Align2::RIGHT_CENTER
                            } else {
                                Align2::LEFT_CENTER
                            },
                            heading,
                            FontId::proportional(13.0),
                            TEXT,
                        );
                        if sorted {
                            draw_icon(
                                ui.painter(),
                                Rect::from_center_size(
                                    Pos2::new(rect.right() - 12.0, rect.center().y),
                                    Vec2::splat(12.0),
                                ),
                                if self.process_descending {
                                    Icon::ChevronDown
                                } else {
                                    Icon::ChevronUp
                                },
                                TEXT,
                            );
                        }
                        ui.painter().line_segment(
                            [rect.right_top(), rect.right_bottom()],
                            Stroke::new(1.0, BORDER),
                        );
                        if response.clicked() {
                            chosen = Some(index);
                        }
                    }
                });
                if let Some(index) = chosen {
                    self.process_descending = if self.process_sort == index {
                        !self.process_descending
                    } else {
                        index != 0
                    };
                    self.process_sort = index;
                }
                for process in processes.iter().take(5) {
                    let values = [
                        process.name.clone(),
                        format!("{:.0}%", process.cpu_percent),
                        process
                            .gpu_percent
                            .map_or("—".into(), |v| format!("{v:.0}%")),
                        format!("{}MB", process.ram_bytes / 1_000_000),
                        rate(process.download_bytes_per_second),
                        rate(process.upload_bytes_per_second),
                    ];
                    ui.horizontal(|ui| {
                        for (index, value) in values.iter().enumerate() {
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(widths[index], 30.0),
                                Sense::hover(),
                            );
                            let right = index != 0;
                            let x = if right {
                                rect.right() - 10.0
                            } else {
                                rect.left() + 10.0
                            };
                            let text = ui.painter().layout(
                                value.clone(),
                                FontId::proportional(13.0),
                                SECONDARY,
                                rect.width() - 18.0,
                            );
                            let pos = Pos2::new(
                                if right { x - text.size().x } else { x },
                                rect.center().y - text.size().y / 2.0,
                            );
                            ui.painter()
                                .with_clip_rect(rect)
                                .galley(pos, text, SECONDARY);
                            ui.painter().line_segment(
                                [rect.right_top(), rect.right_bottom()],
                                Stroke::new(1.0, BORDER),
                            );
                        }
                    });
                }
            });
    }
    // ---------- Especificações do sistema ----------

    fn specs(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        page_title(ui, &t("Especificações do sistema"));
        ui.add_space(12.0);
        let specs = &state.specs;
        let mut groups = vec![
            ("CPU", vec![("cpu".to_owned(), specs.cpu.name.clone())]),
            (
                "GPU",
                state
                    .sensors
                    .gpus
                    .iter()
                    .enumerate()
                    .map(|(i, g)| (format!("gpu:{i}"), g.name.clone()))
                    .collect(),
            ),
            (
                "Placa-mãe",
                specs
                    .motherboard
                    .iter()
                    .map(|name| ("board".to_owned(), name.clone()))
                    .collect(),
            ),
        ];
        for (label, prefix, items) in [
            ("RAM", "ram", &specs.memory_modules),
            ("Armazenamento", "storage", &specs.storage),
            ("Áudio", "audio", &specs.audio),
            ("Rede", "network", &specs.network),
        ] {
            groups.push((
                label,
                items
                    .iter()
                    .enumerate()
                    .map(|(i, name)| (format!("{prefix}:{i}"), name.clone()))
                    .collect(),
            ));
        }
        let width = ui.available_width();
        let detail_width = (width * 0.40).clamp(240.0, 388.0);
        let list_width = (width - detail_width - 20.0).max(120.0);
        let mut fields = specs
            .details
            .get(&self.spec_detail)
            .cloned()
            .unwrap_or_default();
        let mut heading = "CPU";
        for (group, items) in &groups {
            if let Some((_, name)) = items.iter().find(|(key, _)| key == &self.spec_detail) {
                heading = group;
                if fields.is_empty() {
                    fields.push(("Nome".into(), name.clone()));
                }
            }
        }
        if self.spec_detail == "cpu" {
            if fields.is_empty() {
                fields = vec![
                    ("Fabricante".into(), specs.cpu.vendor.clone()),
                    ("Nome do código".into(), "—".into()),
                    (
                        "Velocidade padrão do clock".into(),
                        specs
                            .cpu
                            .base_mhz
                            .map_or("—".into(), |v| format!("{v:.0} MHz")),
                    ),
                    ("Tipo de soquete".into(), "—".into()),
                    ("TDP".into(), "—".into()),
                ];
            }
            fields.insert(
                3.min(fields.len()),
                (
                    "Velocidade atual do clock".into(),
                    state
                        .sensors
                        .cpu
                        .clock_mhz
                        .map_or("—".into(), |v| format!("{v:.0} MHz")),
                ),
            );
            fields.insert(
                fields.len().saturating_sub(1),
                ("Energia".into(), "—".into()),
            );
        } else if let Some(index) = self
            .spec_detail
            .strip_prefix("gpu:")
            .and_then(|n| n.parse::<usize>().ok())
            && let Some(gpu) = state.sensors.gpus.get(index)
        {
            fields = vec![
                ("Modelo".into(), gpu.name.clone()),
                (
                    "Velocidade atual do clock".into(),
                    gpu.clock_mhz.map_or("—".into(), |v| format!("{v:.0} MHz")),
                ),
                ("Energia".into(), "—".into()),
            ];
        }
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(list_width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(list_width);
                    ui.set_max_width(list_width);
                    ui.spacing_mut().interact_size.y = 18.0;
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label(
                        RichText::new(t("Lista de dispositivos"))
                            .size(16.0)
                            .strong(),
                    );
                    ui.add_space(14.0);
                    for (group, items) in &groups {
                        ui.label(RichText::new(t(group)).size(16.0).strong());
                        for (key, name) in items {
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                let color = if &self.spec_detail == key {
                                    ORCHID
                                } else {
                                    SECONDARY
                                };
                                if ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(name).size(15.0).color(color),
                                        )
                                        .wrap()
                                        .sense(Sense::click()),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.spec_detail = key.clone();
                                }
                            });
                        }
                        if items.is_empty() {
                            ui.label(RichText::new("—").color(SECONDARY));
                        }
                        ui.add_space(14.0);
                    }
                },
            );
            egui::Frame::new()
                .fill(DARKEST)
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width((detail_width - 32.0).max(100.0));
                        ui.set_max_width((detail_width - 32.0).max(100.0));
                        ui.set_min_height(650.0);
                        ui.label(
                            RichText::new(trf(
                                "{dispositivo} Detalhes:",
                                &[("dispositivo", &t(heading))],
                            ))
                            .size(20.0)
                            .color(ORCHID),
                        );
                        ui.add_space(14.0);
                        for (label, value) in &fields {
                            ui.label(RichText::new(t(label)).size(15.0).strong());
                            ui.add(
                                egui::Label::new(
                                    RichText::new(if value.is_empty() { "—" } else { value })
                                        .size(15.0)
                                        .color(SECONDARY),
                                )
                                .wrap(),
                            );
                            ui.add_space(12.0);
                        }
                    });
                });
        });
    }
    // ---------- Iluminação ----------

    fn lighting_page(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.horizontal(|ui| {
            page_title(ui, &t("Iluminação"));
            self.device_profile_selector(ui, state, true);
        });
        ui.add_space(10.0);
        if state.devices.iter().all(|d| d.lighting.is_empty()) {
            card(ui, |ui| {
                ui.label(t(
                    "Nenhum dispositivo de iluminação compatível foi encontrado",
                ));
            });
            return;
        }
        card(ui, |ui| {
            for device in state.devices.iter().filter(|d| !d.lighting.is_empty()) {
                ui.label(RichText::new(&device.name).size(16.0).strong());
                self.device_status(ui, device);
                for channel in &device.lighting {
                    let key = key(device, &channel.name);
                    let applied = state.config.lighting.get(&key).cloned().unwrap_or_default();
                    let mut draft = self
                        .lighting
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| applied.clone());
                    self.lighting_row(ui, device, channel, &key, &mut draft, &applied);
                    self.lighting.insert(key, draft);
                }
                ui.add_space(10.0);
            }
        });
    }

    fn channel_box(&self, ui: &mut egui::Ui, icon: Icon) {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(50.0), Sense::hover());
        ui.painter().rect_filled(rect, 0, DARKEST);
        draw_icon(ui.painter(), rect.shrink(11.0), icon, TEXT);
    }

    fn lighting_row(
        &mut self,
        ui: &mut egui::Ui,
        device: &DeviceState,
        channel: &LightingChannel,
        key: &str,
        draft: &mut ChannelLighting,
        applied: &ChannelLighting,
    ) {
        let expanded = self.expanded_lighting.contains(key);
        let response = ui
            .horizontal(|ui| {
                ui.set_min_height(70.0);
                self.channel_box(ui, channel_icon(&channel.name));
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new(channel_label(&channel.name)).size(18.0));
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            channel_label(&channel.name),
                            mode_label(applied.mode)
                        ))
                        .size(13.0)
                        .color(SECONDARY),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let modes: Vec<Mode> = channel.capabilities.iter().map(|c| c.mode).collect();
                    let mut chosen = None;
                    dropdown(
                        ui,
                        ("mode", key),
                        &mode_label(draft.mode),
                        DROPDOWN_WIDTH,
                        |ui| {
                            for mode in modes {
                                if ui
                                    .selectable_label(draft.mode == mode, mode_label(mode))
                                    .clicked()
                                {
                                    chosen = Some(mode);
                                }
                            }
                        },
                    );
                    if let Some(mode) = chosen
                        && let Some(capability) =
                            channel.capabilities.iter().find(|c| c.mode == mode)
                    {
                        draft.mode = mode;
                        draft.colors.truncate(capability.max_colors);
                        while draft.colors.len() < capability.min_colors {
                            draft.colors.push(Rgb(0x51, 0x00, 0x7a));
                        }
                        if !capability.direction {
                            draft.backwards = false;
                        }
                        if !capability.per_led {
                            draft.per_led = false;
                        }
                    }
                    ui.add_space(8.0);
                    ui.scope(|ui| {
                        ui.spacing_mut().slider_width = 90.0;
                        ui.spacing_mut().interact_size.y = 20.0;
                        let sun = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover()).0;
                        draw_icon(ui.painter(), sun.shrink(2.0), Icon::Sun, SECONDARY);
                        ui.add(egui::Slider::new(&mut draft.brightness, 0..=100).show_value(false))
                            .on_hover_text(format!("{}%", draft.brightness));
                        let dot = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover()).0;
                        ui.painter()
                            .circle_stroke(dot.center(), 5.0, Stroke::new(1.0, SECONDARY));
                    });
                });
            })
            .response
            .interact(Sense::click());
        if response.clicked() {
            if expanded {
                self.expanded_lighting.remove(key);
            } else {
                self.expanded_lighting.insert(key.to_owned());
            }
        }
        if self.expanded_lighting.contains(key) {
            ui.push_id(key, |ui| self.lighting_detail(ui, channel, draft));
        }
        // Como no CAM, cada mudança é aplicada; aqui, ao soltar o mouse.
        let released = ui.input(|i| !i.pointer.any_down());
        if released && draft != applied && draft.validate(&channel.capabilities).is_ok() {
            self.dispatch(Request::SetLighting {
                channel: key.to_owned(),
                lighting: draft.clone(),
            });
        }
        let (line, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0, BORDER);
        let _ = device;
    }

    fn lighting_detail(
        &self,
        ui: &mut egui::Ui,
        channel: &LightingChannel,
        draft: &mut ChannelLighting,
    ) {
        let Some(capability) = channel
            .capabilities
            .iter()
            .find(|c| c.mode == draft.mode)
            .cloned()
        else {
            return;
        };
        egui::Frame::new()
            .fill(DARKEST)
            .inner_margin(16)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let per_led_available = channel.leds > 1;
                ui.horizontal_wrapped(|ui| {
                    if draft.per_led {
                        ui.label(
                            RichText::new(t("Seleciona qualquer LED abaixo")).color(SECONDARY),
                        );
                    } else {
                        let mut remove = None;
                        for (index, color) in draft.colors.iter_mut().enumerate() {
                            let response = if capability.max_colors > 1 {
                                let mut rgb = [color.0, color.1, color.2];
                                let response = ui.color_edit_button_srgb(&mut rgb);
                                *color = Rgb(rgb[0], rgb[1], rgb[2]);
                                response
                            } else {
                                color_chip(ui, color)
                            };
                            if response.secondary_clicked() && index >= capability.min_colors {
                                remove = Some(index);
                            }
                        }
                        if let Some(index) = remove {
                            draft.colors.remove(index);
                        }
                        if capability.max_colors > 1
                            && draft.colors.len() < capability.max_colors
                            && ui.button(t("Adicionar cor")).clicked()
                        {
                            draft
                                .colors
                                .push(draft.colors.last().copied().unwrap_or(Rgb(0x51, 0, 0x7a)));
                        }
                    }
                    if per_led_available {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let changed = ui
                                .add_enabled_ui(capability.per_led, |ui| {
                                    toggle_switch(ui, &mut draft.per_led)
                                })
                                .inner;
                            ui.label(
                                RichText::new(t("Personalizar cores LED"))
                                    .size(15.0)
                                    .color(SECONDARY),
                            );
                            if !capability.per_led {
                                ui.label(
                                    RichText::new(t("Este modo não aceita cores por LED"))
                                        .size(11.0)
                                        .color(SECONDARY),
                                );
                            }
                            if changed && draft.per_led && draft.led_colors.len() < channel.leds {
                                let base = draft.colors.first().copied().unwrap_or_default();
                                draft.led_colors.resize(channel.leds, base);
                            }
                        });
                    }
                });
                lighting_preview(ui, channel, draft);
                if capability.speed {
                    let margin = ((ui.available_width() - 120.0) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.allocate_ui_with_layout(
                            Vec2::new(120.0, 36.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_max_width(120.0);
                                ui.spacing_mut().slider_width = 120.0;
                                let mut index = draft.speed.index();
                                ui.add(
                                    egui::Slider::new(&mut index, 0..=4)
                                        .show_value(false)
                                        .step_by(1.0),
                                );
                                draft.speed = Speed::ALL[usize::from(index)];
                            },
                        );
                    });
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(trf(
                                "Velocidade {velocidade}",
                                &[("velocidade", &t(draft.speed.label()))],
                            ))
                            .color(SECONDARY),
                        );
                    });
                }
                if capability.direction {
                    ui.checkbox(&mut draft.backwards, t("Direção inversa"));
                }
                if matches!(draft.mode, Mode::Marquee | Mode::CoveringMarquee) {
                    ui.add(egui::Slider::new(&mut draft.led_size, 3..=6).text(t("Tamanho do LED")));
                }
                let validation = draft.validate(&channel.capabilities);
                if let Err(e) = &validation {
                    ui.colored_label(ORANGE, t(&e.to_string()));
                }
            });
    }

    // ---------- Resfriando ----------

    fn cooling_page(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let unit = state.config.ui.temperature_unit;
        let liquid = state
            .devices
            .iter()
            .filter_map(|d| d.last_status.as_ref()?.liquid_temperature)
            .next();
        ui.horizontal(|ui| {
            page_title(ui, &t("Resfriando"));
            self.device_profile_selector(ui, state, false);
        });
        ui.add_space(10.0);
        let third = (ui.available_width() - 32.0) / 3.0;
        let gpu = state.sensors.gpus.get(state.config.governor.gpu_index);
        ui.horizontal_top(|ui| {
            for (name, value, note) in [
                (
                    t("CPU Temperature"),
                    state
                        .sensors
                        .cpu_temperature(state.config.ui.cpu_average_temperature),
                    None,
                ),
                (
                    t("GPU Temperature"),
                    gpu.and_then(|g| g.temperature),
                    gpu.map(|g| g.name.clone()),
                ),
                (t("Liquid Temperature"), liquid, None),
            ] {
                ui.vertical(|ui| {
                    ui.set_width(third);
                    card(ui, |ui| {
                        ui.label(RichText::new(name).size(14.0).strong());
                        ui.vertical_centered(|ui| {
                            let text = value.map_or("—".to_owned(), |v| match unit {
                                TemperatureUnit::Celsius => format!("{v:.0}°"),
                                TemperatureUnit::Fahrenheit => {
                                    format!("{:.0}°", v * 9.0 / 5.0 + 32.0)
                                }
                            });
                            ui.label(RichText::new(text).size(46.0));
                            ui.label(
                                RichText::new(note.unwrap_or_default())
                                    .size(12.0)
                                    .color(SECONDARY),
                            );
                        });
                    });
                });
                ui.add_space(8.0);
            }
        });
        ui.add_space(8.0);
        if state.devices.iter().all(|d| d.cooling.is_empty()) {
            card(ui, |ui| {
                ui.label(t(
                    "Nenhum dispositivo de refrigeração compatível foi encontrado",
                ));
            });
            return;
        }
        card(ui, |ui| {
            for device in state.devices.iter().filter(|d| !d.cooling.is_empty()) {
                ui.label(RichText::new(&device.name).size(16.0).strong());
                self.device_status(ui, device);
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
                    self.cooling_row(
                        ui,
                        device,
                        channel,
                        &key,
                        &mut draft,
                        &applied,
                        current,
                        state,
                        liquid.is_some(),
                    );
                    self.cooling.insert(key, draft);
                }
                ui.add_space(10.0);
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn cooling_row(
        &mut self,
        ui: &mut egui::Ui,
        device: &DeviceState,
        channel: &CoolingChannel,
        key: &str,
        draft: &mut ChannelCooling,
        applied: &ChannelCooling,
        current: Option<f64>,
        state: &Snapshot,
        has_liquid: bool,
    ) {
        let expanded = self.expanded.as_deref() == Some(key);
        let reported = device
            .last_status
            .as_ref()
            .and_then(|s| s.fans.iter().find(|f| f.channel == channel.name));
        let channel_state = device.channels.get(&channel.name);
        let response = ui
            .horizontal(|ui| {
                ui.set_min_height(76.0);
                self.channel_box(ui, channel_icon(&channel.name));
                ui.add_space(6.0);
                ui.label(RichText::new(channel_label(&channel.name)).size(19.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut chosen = None;
                    dropdown(
                        ui,
                        ("profile", key),
                        &profile_label(&applied.profile),
                        DROPDOWN_WIDTH,
                        |ui| {
                            for profile in [
                                Profile::Silent,
                                Profile::Performance,
                                Profile::Fixed { duty: 50 },
                                Profile::Custom { points: Vec::new() },
                            ] {
                                let selected = std::mem::discriminant(&applied.profile)
                                    == std::mem::discriminant(&profile);
                                if ui
                                    .selectable_label(selected, profile_label(&profile))
                                    .clicked()
                                {
                                    chosen = Some(profile);
                                }
                            }
                        },
                    );
                    if let Some(profile) = chosen {
                        match profile {
                            Profile::Silent | Profile::Performance => {
                                draft.profile = profile;
                                self.dispatch(Request::SetCooling {
                                    channel: key.to_owned(),
                                    cooling: draft.clone(),
                                });
                            }
                            Profile::Custom { .. } => {
                                draft.profile = Profile::Custom {
                                    points: draft.points(channel.limits).unwrap_or_else(|_| {
                                        curves::SILENT
                                            .iter()
                                            .map(|&(temperature, duty)| Point { temperature, duty })
                                            .collect()
                                    }),
                                };
                                self.expanded = Some(key.to_owned());
                            }
                            fixed => {
                                draft.profile = fixed;
                                self.expanded = Some(key.to_owned());
                            }
                        }
                    }
                    ui.add_space(170.0);
                    // Pílula da fonte de temperatura.
                    let mut source = None;
                    egui::ComboBox::from_id_salt(("source", key))
                        .width(64.0)
                        .selected_text(RichText::new(source_label(applied.source)).size(13.0))
                        .show_ui(ui, |ui| {
                            for option in [Source::Liquid, Source::Cpu, Source::Gpu] {
                                let available = match option {
                                    Source::Liquid => has_liquid,
                                    Source::Cpu => true,
                                    Source::Gpu => !state.sensors.gpus.is_empty(),
                                };
                                if ui
                                    .add_enabled(
                                        available,
                                        egui::Button::selectable(
                                            applied.source == option,
                                            source_label(option),
                                        ),
                                    )
                                    .clicked()
                                {
                                    source = Some(option);
                                }
                            }
                        });
                    if let Some(option) = source {
                        draft.source = option;
                        if draft.points(channel.limits).is_ok() {
                            self.dispatch(Request::SetCooling {
                                channel: key.to_owned(),
                                cooling: draft.clone(),
                            });
                        }
                    }
                    ui.label(
                        RichText::new(
                            reported
                                .and_then(|f| f.rpm)
                                .map_or("— RPM".into(), |v| format!("{v} RPM")),
                        )
                        .size(13.0),
                    );
                    if let Some(safety) = channel_state.and_then(|c| c.safety.as_ref()) {
                        ui.colored_label(
                            ORANGE,
                            trf("Segurança: {motivo}", &[("motivo", &t(safety))]),
                        );
                    }
                });
            })
            .response
            .interact(Sense::click());
        if response.clicked() {
            self.expanded = if expanded { None } else { Some(key.to_owned()) };
        }
        if expanded || self.expanded.as_deref() == Some(key) {
            let sources: Vec<Source> = [Source::Liquid, Source::Cpu, Source::Gpu]
                .into_iter()
                .filter(|s| match s {
                    Source::Liquid => has_liquid,
                    Source::Cpu => true,
                    Source::Gpu => !state.sensors.gpus.is_empty(),
                })
                .collect();
            self.cooling_detail(
                ui,
                channel,
                key,
                draft,
                applied,
                current,
                channel_state,
                &sources,
            );
        }
        let (line, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0, BORDER);
    }

    #[allow(clippy::too_many_arguments)]
    fn cooling_detail(
        &self,
        ui: &mut egui::Ui,
        channel: &CoolingChannel,
        key: &str,
        draft: &mut ChannelCooling,
        applied: &ChannelCooling,
        current: Option<f64>,
        channel_state: Option<&crate::governor::ChannelState>,
        sources: &[Source],
    ) {
        egui::Frame::new().fill(DARKEST).inner_margin(16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Seletor de fonte centralizado acima do gráfico, como no CAM.
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 330.0).max(0.0) / 2.0);
                ui.label(RichText::new(t("Fonte de temperatura:")).size(15.0));
                dropdown(ui, ("detail-source", key), &source_label(draft.source), 150.0, |ui| {
                    for source in sources {
                        if ui.selectable_label(draft.source == *source, source_label(*source)).clicked() {
                            draft.source = *source;
                            ui.close();
                        }
                    }
                });
            });
            ui.add_space(4.0);
            if let Some(error) = channel_state.and_then(|c| c.error.as_ref()) {
                ui.colored_label(ORANGE, t(error));
            }
            if let Profile::Fixed { duty } = &mut draft.profile {
                ui.add(egui::Slider::new(duty, channel.limits.floor..=channel.limits.ceiling).suffix("%").text(t("Rotação fixa")));
            }
            let editable = matches!(draft.profile, Profile::Custom { .. });
            let mut points = match &draft.profile {
                Profile::Custom { points } => points.clone(),
                _ => draft.points(channel.limits).unwrap_or_default(),
            };
            if curve_editor(ui, ("curve", key), &mut points, current, editable, channel.limits, draft.source) {
                draft.profile = Profile::Custom { points };
            }
            let validation = draft.points(channel.limits);
            if let Err(e) = &validation {
                ui.colored_label(ORANGE, t(&e.to_string()));
            }
            if channel.kind == ChannelKind::Pump {
                ui.label(RichText::new(trf("A bomba nunca fica abaixo de {piso}%.", &[("piso", &channel.limits.floor.to_string())])).size(11.0).color(SECONDARY));
            }
            if channel.internal_profile {
                ui.label(RichText::new(t("Com fonte Líquido a curva roda no próprio dispositivo; com CPU ou GPU o aplicativo ajusta a rotação, e o dispositivo vai a 100% se o líquido chegar a 60 °C.")).size(11.0).color(SECONDARY));
            }
            ui.horizontal(|ui| {
                let changed = draft != applied;
                if ui.add_enabled(changed && validation.is_ok(), egui::Button::new(t("Aplicar")).fill(PURPLE)).clicked() {
                    self.dispatch(Request::SetCooling { channel: key.to_owned(), cooling: draft.clone() });
                }
                if ui.add_enabled(changed, egui::Button::new(t("Descartar"))).clicked() {
                    *draft = applied.clone();
                }
            });
        });
    }

    fn device_status(&self, ui: &mut egui::Ui, device: &DeviceState) {
        let (text, color) = match &device.status {
            DeviceStatus::Ready => return,
            DeviceStatus::Initializing => (t("Inicializando…"), SECONDARY),
            DeviceStatus::PermissionDenied => (t("Sem permissão de acesso"), ORANGE),
            DeviceStatus::Conflict { processes } => (
                trf(
                    "Em uso por outro programa: {programas}",
                    &[("programas", &processes.join(", "))],
                ),
                ORANGE,
            ),
            DeviceStatus::Failed => (t("A comunicação com o dispositivo falhou"), ORANGE),
        };
        ui.colored_label(color, text);
        if let Some(error) = &device.error {
            ui.label(RichText::new(t(error)).size(11.0).color(ORANGE));
        }
        if device.status == DeviceStatus::PermissionDenied {
            ui.label(
                RichText::new(t(
                    "Prepare as permissões em Configurações para acessar o dispositivo sem root.",
                ))
                .size(11.0)
                .color(SECONDARY),
            );
        }
    }

    // ---------- Configurações ----------

    fn settings_page(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let ui_config = &state.config.ui;
        let field = |ui: &mut egui::Ui, text: &str| {
            ui.label(RichText::new(text).size(13.0));
        };
        match self.section {
            Section::General => {
                card(ui, |ui| {
                    title(ui, &t("Geral"));
                    field(ui, &t("Idioma"));
                    let mut language = None;
                    dropdown(ui, "language", &ui_config.language, 400.0, |ui| {
                        for tag in &state.languages {
                            if ui
                                .selectable_label(&ui_config.language == tag, tag)
                                .clicked()
                            {
                                language = Some(tag.clone());
                            }
                        }
                    });
                    if let Some(tag) = language {
                        self.settings(|r| {
                            if let Request::UpdateSettings { language, .. } = r {
                                *language = Some(tag);
                            }
                        });
                    }
                    field(ui, &t("Exibição da temperatura"));
                    let unit_label = |unit: TemperatureUnit| {
                        t(match unit {
                            TemperatureUnit::Celsius => "Celsius",
                            TemperatureUnit::Fahrenheit => "Fahrenheit",
                        })
                    };
                    let mut unit = None;
                    dropdown(
                        ui,
                        "unit",
                        &unit_label(ui_config.temperature_unit),
                        400.0,
                        |ui| {
                            for option in [TemperatureUnit::Celsius, TemperatureUnit::Fahrenheit] {
                                if ui
                                    .selectable_label(
                                        ui_config.temperature_unit == option,
                                        unit_label(option),
                                    )
                                    .clicked()
                                {
                                    unit = Some(option);
                                }
                            }
                        },
                    );
                    if let Some(option) = unit {
                        self.settings(|r| {
                            if let Request::UpdateSettings {
                                temperature_unit, ..
                            } = r
                            {
                                *temperature_unit = Some(option);
                            }
                        });
                    }
                    field(ui, &t("Exibição de temperatura do CPU"));
                    let cpu_label = |average: bool| {
                        t(if average {
                            "Média dos núcleos do CPU"
                        } else {
                            "Temperatura do pacote do CPU"
                        })
                    };
                    let mut average = None;
                    dropdown(
                        ui,
                        "cpu-temp",
                        &cpu_label(ui_config.cpu_average_temperature),
                        400.0,
                        |ui| {
                            for option in [false, true] {
                                if ui
                                    .selectable_label(
                                        ui_config.cpu_average_temperature == option,
                                        cpu_label(option),
                                    )
                                    .clicked()
                                {
                                    average = Some(option);
                                }
                            }
                        },
                    );
                    if let Some(option) = average {
                        self.settings(|r| {
                            if let Request::UpdateSettings {
                                cpu_average_temperature,
                                ..
                            } = r
                            {
                                *cpu_average_temperature = Some(option);
                            }
                        });
                    }
                    ui.add_space(6.0);
                    let mut close = ui_config.close_to_tray;
                    if ui
                        .checkbox(
                            &mut close,
                            RichText::new(t("Minimizar o NZXT CAM na bandeja ao fechar"))
                                .size(15.0),
                        )
                        .changed()
                    {
                        self.settings(|r| {
                            if let Request::UpdateSettings { close_to_tray, .. } = r {
                                *close_to_tray = Some(close);
                            }
                        });
                    }
                    let mut autostart = state.autostart;
                    if ui
                        .checkbox(
                            &mut autostart,
                            RichText::new(t("Iniciar o NZXT CAM ao iniciar a sessão")).size(15.0),
                        )
                        .changed()
                    {
                        self.dispatch(Request::SetAutostart { enabled: autostart });
                    }
                    let mut minimized = ui_config.start_minimized;
                    if ui
                        .checkbox(
                            &mut minimized,
                            RichText::new(t("Iniciar o NZXT CAM minimizado")).size(15.0),
                        )
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
                });
                ui.add_space(8.0);
                card(ui, |ui| {
                    title(ui, &t("Backup das configurações"));
                    ui.label(
                        RichText::new(t("Exporte e importe iluminação, refrigeração e configurações dos dispositivos."))
                            .size(13.0)
                            .italics()
                            .color(SECONDARY),
                    );
                    let path = backup_path();
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Button::new(t("Exportar")).fill(PURPLE))
                            .clicked()
                        {
                            self.dispatch(Request::ExportConfig { path: path.clone() });
                        }
                        if ui
                            .add(egui::Button::new(t("Importar")).fill(PURPLE))
                            .clicked()
                        {
                            self.dispatch(Request::ImportConfig { path: path.clone() });
                        }
                    });
                    ui.label(
                        RichText::new(trf(
                            "Arquivo: {caminho}",
                            &[("caminho", &path.display().to_string())],
                        ))
                        .size(11.0)
                        .color(SECONDARY),
                    );
                });
                ui.add_space(8.0);
                card(ui, |ui| {
                    title(ui, &t("Painéis"));
                    let mut panels = ui_config.panels.clone();
                    let mut changed = false;
                    for (value, page) in [
                        (&mut panels.monitoring, Page::Monitoring),
                        (&mut panels.specs, Page::Specs),
                        (&mut panels.lighting, Page::Lighting),
                        (&mut panels.cooling, Page::Cooling),
                    ] {
                        changed |= ui
                            .checkbox(value, RichText::new(t(page.label())).size(15.0))
                            .changed();
                    }
                    if changed {
                        self.update_ui(state, |c| c.panels = panels);
                    }
                });
            }
            Section::Firmware => {
                page_title(ui, "NZXT Firmware");
                ui.add_space(8.0);
                if state.devices.is_empty() {
                    card(ui, |ui| {
                        ui.label(t("Nenhum dispositivo compatível encontrado"));
                    });
                }
                for device in &state.devices {
                    card(ui, |ui| {
                        title(ui, &device.name);
                        let version = device
                            .last_status
                            .as_ref()
                            .and_then(|s| s.firmware.clone())
                            .unwrap_or_else(|| "—".into());
                        ui.label(
                            RichText::new(trf(
                                "Versão do firmware {versao}",
                                &[("versao", &version)],
                            ))
                            .size(15.0)
                            .color(SECONDARY),
                        );
                    });
                    ui.add_space(8.0);
                }
                ui.label(
                    RichText::new(t(
                        "O porte não atualiza firmware. Para atualizar, use o NZXT CAM no Windows.",
                    ))
                    .size(12.0)
                    .color(SECONDARY),
                );
            }
            Section::Access => card(ui, |ui| {
                title(ui, &t("Acesso aos dispositivos"));
                self.permissions_section(ui, state);
            }),
            Section::Profiles => self.profiles_page(ui, state),
            Section::About => card(ui, |ui| {
                title(ui, &t("Sobre"));
                label_value(ui, &t("Versão"), env!("CARGO_PKG_VERSION"));
                ui.add_space(14.0);
                title(ui, &t("Atualizações do aplicativo"));
                use crate::updater::Phase;
                let (label, request) = match state.updates.phase {
                    Phase::Available if state.updates.installed => {
                        ("Atualizar agora", Request::InstallUpdate)
                    }
                    Phase::RestartRequired => ("Reiniciar", Request::RestartAfterUpdate),
                    _ => ("Verificar atualizações", Request::CheckUpdates),
                };
                if ui
                    .add_enabled(!state.updates.busy(), egui::Button::new(t(label)))
                    .clicked()
                {
                    self.dispatch(request);
                }
                if state.updates.busy() {
                    ui.spinner();
                }
                let revision = &state.updates.build.revision;
                ui.label(
                    RichText::new(format!(
                        "{}{}",
                        revision.get(..8).unwrap_or(revision),
                        if state.updates.build.dirty {
                            " (dev)"
                        } else {
                            ""
                        }
                    ))
                    .size(12.0)
                    .color(SECONDARY),
                )
                .on_hover_text(revision);
                if !state.updates.message.is_empty() {
                    ui.label(RichText::new(t(&state.updates.message)).color(
                        if state.updates.phase == Phase::Failed {
                            ORANGE
                        } else {
                            SECONDARY
                        },
                    ));
                }
                if state.updates.phase == Phase::Available && !state.updates.installed {
                    ui.label(t("Use o instalador desktop para habilitar atualizações"));
                }
                if let Some(candidate) = &state.updates.candidate {
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            candidate.branch,
                            candidate.revision.get(..8).unwrap_or(&candidate.revision)
                        ))
                        .size(12.0)
                        .color(SECONDARY),
                    );
                }
                if let Some(log) = &state.updates.log {
                    ui.monospace(log.display().to_string());
                }
                ui.hyperlink_to(
                    t("Repositório do projeto"),
                    "https://github.com/oiacrasec/ubuntu-nzxt-cam",
                );
                ui.label(RichText::new(t("Projeto independente, sem afiliação com a NZXT. Referência: NZXT CAM 4.76.8 para Windows.")).size(12.0).color(SECONDARY));
            }),
        }
    }

    fn permissions_section(&self, ui: &mut egui::Ui, state: &Snapshot) {
        let permissions = &state.permissions;
        if permissions.rule_installed && !permissions.rule_outdated {
            ui.colored_label(
                GREEN,
                t("Regra udev instalada: a sessão acessa os dispositivos sem root"),
            );
        } else if permissions.rule_outdated {
            ui.colored_label(
                ORANGE,
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
            .color(SECONDARY),
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
                    .add(egui::Button::new(t("Confirmar e autenticar")).fill(PURPLE))
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
                if ui
                    .add(egui::Button::new(t("Preparar permissões…")).fill(PURPLE))
                    .clicked()
                {
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
            ui.colored_label(ORANGE, t(error));
        }
    }
}

/// Chip de cor como o do CAM: amostra + valor hexadecimal; clicar abre o seletor.
fn color_chip(ui: &mut egui::Ui, color: &mut Rgb) -> egui::Response {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut rgb = [color.0, color.1, color.2];
                let response = ui.color_edit_button_srgb(&mut rgb);
                *color = Rgb(rgb[0], rgb[1], rgb[2]);
                ui.label(
                    RichText::new(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))
                        .size(14.0),
                );
                response
            })
            .inner
        })
        .inner
}

/// Interruptor em pílula (trilho cinza e bolinha clara; roxo quando ligado).
fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> bool {
    let size = Vec2::new(50.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
    }
    let progress = ui.ctx().animate_bool_responsive(response.id, *on);
    let track = if *on { PURPLE } else { CONTROL };
    ui.painter().rect_filled(rect, 2, track);
    let knob_width = rect.height() + 6.0;
    let x = egui::lerp(rect.left()..=rect.right() - knob_width, progress);
    let knob = Rect::from_min_size(
        Pos2::new(x, rect.top()),
        Vec2::new(knob_width, rect.height()),
    );
    ui.painter().rect_filled(knob.shrink(2.0), 2, SECONDARY);
    response.clicked()
}

/// Prévia própria do canal com as cores atuais: anel de 8 LEDs, logo ou fita.
fn lighting_preview(ui: &mut egui::Ui, channel: &LightingChannel, lighting: &mut ChannelLighting) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 170.0), Sense::hover());
    let painter = ui.painter().clone();
    let color = |i: usize| -> Color32 {
        let rgb = if lighting.per_led {
            lighting.led_colors.get(i).copied()
        } else {
            lighting
                .colors
                .get(i % lighting.colors.len().max(1))
                .copied()
        }
        .unwrap_or(Rgb(60, 60, 60));
        let scale = f32::from(lighting.brightness.min(100)) / 100.0;
        Color32::from_rgb(
            (f32::from(rgb.0) * scale) as u8,
            (f32::from(rgb.1) * scale) as u8,
            (f32::from(rgb.2) * scale) as u8,
        )
    };
    let c = rect.center();
    let mut led_targets = Vec::new();
    match channel.name.as_str() {
        "ring" => {
            for i in 0..8 {
                let a0 =
                    i as f32 * std::f32::consts::TAU / 8.0 - std::f32::consts::FRAC_PI_2 + 0.08;
                let a1 = a0 + std::f32::consts::TAU / 8.0 - 0.16;
                let points: Vec<Pos2> = (0..=10)
                    .map(|k| {
                        let a = a0 + (a1 - a0) * k as f32 / 10.0;
                        c + Vec2::new(a.cos(), a.sin()) * 52.0
                    })
                    .collect();
                led_targets.push((i, Rect::from_points(&points).expand(6.0)));
                painter.add(egui::Shape::line(points, Stroke::new(12.0, color(i))));
            }
        }
        "logo" => {
            painter.circle_stroke(c, 52.0, Stroke::new(10.0, Color32::from_gray(230)));
            painter.circle_filled(c, 26.0, color(0));
            led_targets.push((0, Rect::from_center_size(c, Vec2::splat(52.0))));
        }
        _ => {
            let leds = channel.leds.min(40);
            let width = (rect.width() - 40.0).min(800.0);
            for i in 0..leds {
                let x = c.x - width / 2.0 + width * (i as f32 + 0.5) / leds as f32;
                painter.circle_filled(Pos2::new(x, c.y), 5.0, color(i));
                led_targets.push((
                    i,
                    Rect::from_center_size(Pos2::new(x, c.y), Vec2::splat(12.0)),
                ));
            }
        }
    }
    if lighting.per_led {
        for (index, target) in led_targets {
            let Some(rgb) = lighting.led_colors.get_mut(index) else {
                continue;
            };
            let label = trf("LED {numero}", &[("numero", &(index + 1).to_string())]);
            let response = ui
                .interact(target, ui.id().with(("led", index)), Sense::click())
                .on_hover_text(&label);
            if response.hovered() {
                painter.rect_stroke(
                    target,
                    2,
                    Stroke::new(1.0, ORCHID),
                    egui::StrokeKind::Outside,
                );
            }
            egui::Popup::from_toggle_button_response(&response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.label(&label);
                    let mut color = Color32::from_rgb(rgb.0, rgb.1, rgb.2);
                    if egui::color_picker::color_picker_color32(
                        ui,
                        &mut color,
                        egui::color_picker::Alpha::Opaque,
                    ) {
                        *rgb = Rgb(color.r(), color.g(), color.b());
                    }
                });
        }
    }
}

/// Editor da curva no estilo do gráfico do CAM: linhas horizontais suaves, eixo de
/// 10° a 100°, linha e pontos verdes e marcador tracejado na temperatura atual.
/// Arrastar move pontos, duplo clique adiciona, botão direito remove.
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
    const DOMAIN: f32 = 105.0;
    let grid = Color32::from_rgb(40, 42, 46);
    let size = Vec2::new(ui.available_width().min(900.0), 270.0);
    let (response, painter) = ui.allocate_painter(size, Sense::click());
    // Margens: rótulos de % à esquerda, de temperatura embaixo e da leitura atual em cima.
    let rect = Rect::from_min_max(
        response.rect.min + Vec2::new(64.0, 34.0),
        response.rect.max - Vec2::new(28.0, 30.0),
    );
    let x_of = |temperature: f64| rect.left() + rect.width() * (temperature as f32 / DOMAIN);
    let y_of = |duty: f32| rect.bottom() - rect.height() * (duty / 100.0);
    let to_screen = |p: &Point| Pos2::new(x_of(p.temperature), y_of(f32::from(p.duty)));
    let from_screen = |pos: Pos2| {
        let temperature = (((pos.x - rect.left()) / rect.width()) * DOMAIN).clamp(0.0, 100.0);
        let duty = (((rect.bottom() - pos.y) / rect.height()) * 100.0).clamp(0.0, 100.0);
        (
            f64::from(temperature.round()),
            (duty.round() as u8).max(limits.floor),
        )
    };
    for duty in [0.0, 25.0, 50.0, 75.0, 100.0] {
        let y = y_of(duty);
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0, grid),
        );
        painter.text(
            Pos2::new(rect.left() - 12.0, y),
            Align2::RIGHT_CENTER,
            format!("{duty:.0}%"),
            FontId::proportional(11.0),
            SECONDARY,
        );
    }
    for temperature in (10..=100).step_by(10) {
        painter.text(
            Pos2::new(x_of(f64::from(temperature)), rect.bottom() + 10.0),
            Align2::CENTER_TOP,
            format!("{temperature}°"),
            FontId::proportional(11.0),
            SECONDARY,
        );
    }
    if let Some(temperature) = current.filter(|t| t.is_finite()) {
        let x = x_of(temperature.clamp(0.0, 100.0));
        painter.extend(egui::Shape::dashed_line(
            &[Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, MEDIUM),
            3.0,
            3.0,
        ));
        painter.text(
            Pos2::new(x, rect.top() - 8.0),
            Align2::CENTER_BOTTOM,
            format!("{}: {temperature:.0}°", source_label(source)),
            FontId::proportional(12.0),
            TEXT,
        );
    }
    let mut changed = false;
    if editable {
        let mut remove = None;
        for index in 0..points.len() {
            let center = to_screen(&points[index]);
            let handle = Rect::from_center_size(center, Vec2::splat(16.0));
            let point_response =
                ui.interact(handle, ui.id().with((&id, index)), Sense::click_and_drag());
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
    let line: Vec<Pos2> = points.iter().map(&to_screen).collect();
    // Depois do último ponto a rotação se mantém; o traço segue até 100°, como no CAM.
    let mut drawn = line.clone();
    if let Some(last) = points.last().filter(|p| p.temperature < 100.0) {
        drawn.push(Pos2::new(x_of(100.0), y_of(f32::from(last.duty))));
    }
    painter.add(egui::Shape::line(drawn, Stroke::new(2.0, GREEN)));
    for position in line {
        painter.circle_filled(position, if editable { 5.5 } else { 4.5 }, GREEN);
    }
    if editable {
        ui.label(
            RichText::new(t(
                "Arraste os pontos; duplo clique adiciona, botão direito remove.",
            ))
            .size(11.0)
            .color(SECONDARY),
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
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        #[cfg(all(feature = "tray", target_os = "linux"))]
        if let Some(tray) = &self.tray {
            let language = state.config.ui.language.clone();
            tray.update(|tray| tray.language = language);
        }
        if ctx.input(|i| i.viewport().close_requested()) && !state.quit {
            #[cfg(all(feature = "tray", target_os = "linux"))]
            if self.tray.is_some() && state.config.ui.close_to_tray {
                // Fechar esconde a janela; o governador continua ativo.
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                return;
            }
            self.dispatch(Request::Quit);
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        let state = self.state();
        if self.last_profile != profile_ui::selection(&state) {
            self.lighting.clear();
            self.cooling.clear();
            self.last_profile = profile_ui::selection(&state);
        }
        self.top_bar(root);
        self.sidebar(root, &state);
        if let Some(error) = &state.error {
            egui::Panel::bottom("status")
                .frame(egui::Frame::new().fill(DARKEST).inner_margin(8))
                .show(root, |ui| {
                    ui.colored_label(ORANGE, t(error));
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(16, 14)),
            )
            .show(root, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Coluna central de largura fixa, como no CAM.
                        let width = ui.available_width().min(CONTENT_WIDTH);
                        let margin = ((ui.available_width() - width) / 2.0).max(0.0);
                        ui.horizontal_top(|ui| {
                            ui.add_space(margin);
                            ui.vertical(|ui| {
                                ui.set_width(width);
                                match self.page {
                                    Page::Monitoring => self.monitoring(ui, &state),
                                    Page::Specs => self.specs(ui, &state),
                                    Page::Lighting => self.lighting_page(ui, &state),
                                    Page::Cooling => self.cooling_page(ui, &state),
                                    Page::Settings => self.settings_page(ui, &state),
                                }
                            });
                        });
                    });
            });
        self.profile_dialog(root.ctx(), &state);
    }
}
