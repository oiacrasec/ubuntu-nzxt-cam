//! Profile Sync observado no CAM 4.76.8. Desenho e implementação próprios.
use super::*;
use crate::profiles::{AutoLaunch, Edit, SyncProfile};

pub(super) enum ProfileDialog {
    Help,
    Name {
        existing: Option<String>,
        name: String,
    },
    Programs {
        profile: String,
        connected: Vec<String>,
        search: String,
        browsing: bool,
        directory: String,
    },
    Time {
        profile: String,
        start: u16,
        end: u16,
        days: [bool; 7],
        all_day: bool,
    },
    Device {
        lighting: bool,
        name: String,
        duplicate: bool,
        source: String,
    },
}

/// Altura fixa das colunas do diálogo de programas: o diálogo nunca passa da janela.
const PROGRAM_LIST_HEIGHT: f32 = 320.0;
const DIALOG_BUTTON: Vec2 = Vec2::new(120.0, 40.0);

pub(super) fn selection(state: &Snapshot) -> Option<(String, String, String)> {
    let name = state.config.profiles.active.as_ref()?;
    let profile = state.config.profiles.sync.get(name)?;
    Some((
        name.clone(),
        profile.lighting.clone(),
        profile.cooling.clone(),
    ))
}

fn condition_label(condition: &AutoLaunch) -> String {
    t(match condition {
        AutoLaunch::None => "Sem ativação automática",
        AutoLaunch::Program { .. } => "Com programa",
        AutoLaunch::Schedule { .. } => "Em horário específico",
    })
}

/// Botão de diálogo de tamanho único; `primary` usa o roxo do tema.
fn dialog_button(ui: &mut egui::Ui, text: &str, primary: bool, enabled: bool) -> bool {
    let mut button = egui::Button::new(RichText::new(text).size(15.0));
    if primary {
        button = button.fill(PURPLE);
    }
    ui.add_enabled_ui(enabled, |ui| ui.add_sized(DIALOG_BUTTON, button))
        .inner
        .clicked()
}

fn name_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(label);
            ui.add(
                egui::TextEdit::singleline(value)
                    .char_limit(50)
                    .font(FontId::proportional(23.0))
                    .frame(egui::Frame::NONE)
                    .desired_width(ui.available_width()),
            );
        });
}

fn destination_link(ui: &mut egui::Ui, label: &str) -> bool {
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(22.0, 24.0), Sense::click());
        let center = rect.center();
        ui.painter().line_segment(
            [center - Vec2::new(6.0, 0.0), center + Vec2::new(7.0, 0.0)],
            Stroke::new(1.5, ORCHID),
        );
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center - Vec2::new(7.0, 0.0),
                center + Vec2::new(-1.0, -4.0),
                center + Vec2::new(-1.0, 4.0),
            ],
            ORCHID,
            Stroke::NONE,
        ));
        let arrow = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked();
        ui.add(egui::Label::new(RichText::new(label).size(20.0)).sense(Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            || arrow
    })
    .inner
}

/// Quadro com título, usado nas seções dos diálogos.
fn section<R>(
    ui: &mut egui::Ui,
    title: &str,
    stroke: Color32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, stroke))
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong());
            ui.add_space(14.0);
            add(ui)
        })
        .inner
}

impl App {
    fn condition_dialog(&mut self, state: &Snapshot, profile: &str, program: bool) {
        let Some(current) = state.config.profiles.sync.get(profile) else {
            return;
        };
        self.profile_dialog = Some(if program {
            self.dispatch(Request::RefreshPrograms);
            ProfileDialog::Programs {
                profile: profile.into(),
                connected: match &current.auto_launch {
                    AutoLaunch::Program { executables } => executables.clone(),
                    _ => Vec::new(),
                },
                search: String::new(),
                browsing: false,
                directory: "/usr/bin".into(),
            }
        } else {
            let (start, end, days) = match current.auto_launch {
                AutoLaunch::Schedule { start, end, days } => (start, end, days),
                _ => {
                    let minute = state.local_time.map_or(0, |time| time.minute);
                    (minute, (minute + 60) % 1440, crate::profiles::all_days())
                }
            };
            ProfileDialog::Time {
                profile: profile.into(),
                start,
                end,
                days,
                all_day: start == 0 && end == 0,
            }
        });
    }

    fn change_condition(&self, state: &Snapshot, name: &str, condition: AutoLaunch) {
        if let Some(profile) = state.config.profiles.sync.get(name) {
            let mut profile = profile.clone();
            profile.auto_launch = condition;
            // Alterar a condição não requer reenviar iluminação nem curvas.
            self.dispatch(Request::EditProfile {
                edit: Edit::Save {
                    name: name.into(),
                    profile,
                },
            });
        }
    }

    pub(super) fn profiles_page(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let profiles = &state.config.profiles;
        let active = profiles.active.as_deref().unwrap_or("Default");
        let Some(profile) = profiles.sync.get(active) else {
            return;
        };
        ui.horizontal(|ui| {
            page_title(ui, "Profile Sync");
            dropdown(ui, "sync-profile", active, 240.0, |ui| {
                for name in profiles.sync.keys() {
                    if ui.selectable_label(active == name, name).clicked() {
                        self.dispatch(Request::ActivateProfile { name: name.clone() });
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button(t("+ Adicionar perfil")).clicked() {
                    self.profile_dialog = Some(ProfileDialog::Name {
                        existing: None,
                        name: String::new(),
                    });
                    ui.close();
                }
            });
            if active != "Default" && self.icon_button(ui, Icon::Gear, &t("Editar Profile Sync")) {
                self.profile_dialog = Some(ProfileDialog::Name {
                    existing: Some(active.into()),
                    name: active.into(),
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.link(t("O que é o Profile Sync?")).clicked() {
                    self.profile_dialog = Some(ProfileDialog::Help);
                }
            });
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.add_enabled_ui(active != "Default", |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(40.0);
                    ui.label(RichText::new(t("Ativação automática")).size(20.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let current = condition_label(&profile.auto_launch);
                        dropdown(ui, "auto-launch", &current, 240.0, |ui| {
                            for (index, label) in [
                                t("Sem ativação automática"),
                                t("Com programa"),
                                t("Em horário específico"),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                if ui.selectable_label(current == label, &label).clicked() {
                                    match index {
                                        0 => self.change_condition(state, active, AutoLaunch::None),
                                        1 => self.condition_dialog(state, active, true),
                                        _ => self.condition_dialog(state, active, false),
                                    }
                                    ui.close();
                                }
                            }
                        });
                        if profile.auto_launch != AutoLaunch::None
                            && self.icon_button(ui, Icon::Gear, &t("Editar ativação automática"))
                        {
                            self.condition_dialog(
                                state,
                                active,
                                matches!(profile.auto_launch, AutoLaunch::Program { .. }),
                            );
                        }
                    });
                });
                if active == "Default" {
                    ui.label(
                        RichText::new(t(
                            "A ativação automática não está disponível para o perfil Default.",
                        ))
                        .size(13.0)
                        .color(SECONDARY),
                    );
                }
            });
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.label(RichText::new(t("Perfis selecionados")).size(15.0).strong());
            ui.add_space(12.0);
            for lighting in [true, false] {
                let label = t(if lighting {
                    "Iluminação"
                } else {
                    "Resfriando"
                });
                let selected = if lighting {
                    &profile.lighting
                } else {
                    &profile.cooling
                };
                let choices: Vec<_> = if lighting {
                    profiles.lighting.keys().collect()
                } else {
                    profiles.cooling.keys().collect()
                };
                ui.horizontal(|ui| {
                    ui.set_min_height(56.0);
                    ui.vertical(|ui| {
                        if destination_link(ui, &label) {
                            self.page = if lighting {
                                Page::Lighting
                            } else {
                                Page::Cooling
                            };
                        }
                        let mut seen = HashSet::new();
                        let devices: Vec<_> = state
                            .devices
                            .iter()
                            .filter(|d| {
                                if lighting {
                                    !d.lighting.is_empty()
                                } else {
                                    !d.cooling.is_empty()
                                }
                            })
                            .map(|d| d.name.as_str())
                            .filter(|name| seen.insert(*name))
                            .collect();
                        ui.horizontal(|ui| {
                            ui.add_space(30.0);
                            ui.label(RichText::new(devices.join(", ")).size(13.0));
                        });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        dropdown(ui, ("selected-profile", lighting), selected, 240.0, |ui| {
                            for name in choices {
                                if ui.selectable_label(selected == name, name).clicked() {
                                    let mut profile = profile.clone();
                                    if lighting {
                                        profile.lighting = name.clone();
                                    } else {
                                        profile.cooling = name.clone();
                                    }
                                    self.dispatch(Request::SaveProfileAndActivate {
                                        name: active.into(),
                                        profile,
                                    });
                                    ui.close();
                                }
                            }
                        });
                    });
                });
            }
        });
    }

    pub(super) fn device_profile_selector(
        &mut self,
        ui: &mut egui::Ui,
        state: &Snapshot,
        lighting: bool,
    ) {
        let profiles = &state.config.profiles;
        let active = profiles.active.as_deref().unwrap_or("Default");
        let Some(profile) = profiles.sync.get(active) else {
            return;
        };
        let selected = if lighting {
            &profile.lighting
        } else {
            &profile.cooling
        };
        let choices: Vec<_> = if lighting {
            profiles.lighting.keys().collect()
        } else {
            profiles.cooling.keys().collect()
        };
        dropdown(ui, ("device-profiles", lighting), selected, 240.0, |ui| {
            if ui.button(t("Adicionar perfil")).clicked() {
                self.profile_dialog = Some(ProfileDialog::Device {
                    lighting,
                    name: String::new(),
                    duplicate: false,
                    source: selected.clone(),
                });
                ui.close();
            }
            for name in choices {
                if ui.selectable_label(name == selected, name).clicked() {
                    let mut profile = profile.clone();
                    if lighting {
                        profile.lighting = name.clone();
                    } else {
                        profile.cooling = name.clone();
                    }
                    self.dispatch(Request::SaveProfileAndActivate {
                        name: active.into(),
                        profile,
                    });
                    ui.close();
                }
            }
        });
    }

    pub(super) fn profile_dialog(&mut self, ctx: &egui::Context, state: &Snapshot) {
        let Some(mut dialog) = self.profile_dialog.take() else {
            return;
        };
        let mut close = false;
        let preferred: f32 = match dialog {
            ProfileDialog::Programs { .. } => 760.0,
            ProfileDialog::Time { .. } => 728.0,
            _ => 536.0,
        };
        let width = preferred.min((ctx.content_rect().width() - 72.0).max(240.0));
        let response = egui::Modal::new(egui::Id::new("profile-dialog"))
            .backdrop_color(Color32::from_black_alpha(200))
            .frame(
                egui::Frame::new()
                    .fill(DARKEST)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(32),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                let heading = t(match &dialog {
                    ProfileDialog::Help => "O que é o Profile Sync?",
                    ProfileDialog::Name { existing: Some(_), .. } => "Editar Profile Sync",
                    ProfileDialog::Name { .. } => "Adicionar Profile Sync",
                    ProfileDialog::Programs { .. } => "Programas da ativação automática",
                    ProfileDialog::Time { .. } => "Configurar horário de ativação",
                    ProfileDialog::Device { .. } => "Adicionar perfil",
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(heading).size(24.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([32.0, 32.0], egui::Button::new("×")).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(24.0);
                match &mut dialog {
                    ProfileDialog::Help => {
                        ui.label(t("Profile Sync combina perfis de iluminação e refrigeração. Selecione um perfil na barra superior ou configure sua ativação por programa ou horário."));
                        ui.add_space(24.0);
                        close |= dialog_button(ui, "OK", true, true);
                    }
                    ProfileDialog::Name { existing, name } => {
                        name_field(ui, &t("Nome"), name);
                        ui.add_space(40.0);
                        let valid = !name.trim().is_empty()
                            && (existing.as_ref() == Some(name)
                                || !state.config.profiles.sync.contains_key(name.trim()));
                        ui.horizontal(|ui| {
                            let secondary = t(if existing.is_some() { "Excluir" } else { "Cancelar" });
                            if dialog_button(ui, &secondary, false, true) {
                                if let Some(existing) = existing {
                                    self.dispatch(Request::EditProfile {
                                        edit: Edit::Delete { name: existing.clone() },
                                    });
                                }
                                close = true;
                            }
                            if dialog_button(ui, &t("Salvar"), true, valid) {
                                if let Some(existing) = existing {
                                    self.dispatch(Request::EditProfile {
                                        edit: Edit::Rename {
                                            name: existing.clone(),
                                            new_name: name.trim().into(),
                                        },
                                    });
                                } else {
                                    self.dispatch(Request::SaveProfileAndActivate {
                                        name: name.trim().into(),
                                        profile: SyncProfile::default(),
                                    });
                                }
                                close = true;
                            }
                        });
                    }
                    ProfileDialog::Programs {
                        profile,
                        connected,
                        search,
                        browsing,
                        directory,
                    } => {
                        self.program_dialog_content(ui, state, connected, search, browsing, directory);
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if dialog_button(ui, &t("Salvar"), true, true) {
                                self.change_condition(
                                    state,
                                    profile,
                                    AutoLaunch::Program {
                                        executables: connected.clone(),
                                    },
                                );
                                close = true;
                            }
                            if dialog_button(ui, &t("Cancelar"), false, true) {
                                close = true;
                            }
                        });
                    }
                    ProfileDialog::Time {
                        profile,
                        start,
                        end,
                        days,
                        all_day,
                    } => {
                        let valid = days.contains(&true);
                        section(ui, &t("Dias"), if valid { BORDER } else { Color32::RED }, |ui| {
                            let labels = [
                                "Domingo",
                                "Segunda-feira",
                                "Terça-feira",
                                "Quarta-feira",
                                "Quinta-feira",
                                "Sexta-feira",
                                "Sábado",
                            ];
                            // Quatro colunas iguais ocupando a largura do quadro.
                            let column = ui.available_width() / 4.0;
                            for (row_index, names) in labels.chunks(4).enumerate() {
                                ui.horizontal(|ui| {
                                    for (offset, name) in names.iter().enumerate() {
                                        let index = row_index * 4 + offset;
                                        ui.allocate_ui(Vec2::new(column, 28.0), |ui| {
                                            ui.set_width(column);
                                            ui.checkbox(&mut days[index], t(name));
                                        });
                                    }
                                });
                                ui.add_space(8.0);
                            }
                            if !valid {
                                ui.colored_label(ORANGE, t("Selecione pelo menos um dia"));
                            }
                        });
                        ui.add_space(16.0);
                        section(ui, &t("Horário"), BORDER, |ui| {
                            egui::Grid::new("schedule-time")
                                .num_columns(3)
                                .spacing([24.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label(t("Início"));
                                    ui.horizontal(|ui| {
                                        ui.label(t("Fim"));
                                        if *end < *start || *all_day {
                                            ui.colored_label(ORCHID, t("+1 dia"));
                                        }
                                    });
                                    ui.label("");
                                    ui.end_row();
                                    ui.add_enabled_ui(!*all_day, |ui| time_input(ui, "start", start));
                                    ui.add_enabled_ui(!*all_day, |ui| time_input(ui, "end", end));
                                    if ui.checkbox(all_day, t("Dia inteiro")).changed() {
                                        if *all_day {
                                            *start = 0;
                                            *end = 0;
                                        } else {
                                            *start = state.local_time.map_or(0, |time| time.minute);
                                            *end = (*start + 60) % 1440;
                                        }
                                    }
                                    ui.end_row();
                                });
                        });
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            if dialog_button(ui, &t("Salvar"), true, valid) {
                                self.change_condition(
                                    state,
                                    profile,
                                    AutoLaunch::Schedule {
                                        start: *start,
                                        end: *end,
                                        days: *days,
                                    },
                                );
                                close = true;
                            }
                            if dialog_button(ui, &t("Cancelar"), false, true) {
                                close = true;
                            }
                        });
                    }
                    ProfileDialog::Device {
                        lighting,
                        name,
                        duplicate,
                        source,
                    } => {
                        name_field(ui, &t(if *lighting { "Nome do tema" } else { "Nome" }), name);
                        ui.add_space(24.0);
                        ui.separator();
                        ui.add_space(12.0);
                        let choices: Vec<String> = if *lighting {
                            state.config.profiles.lighting.keys().cloned().collect()
                        } else {
                            state.config.profiles.cooling.keys().cloned().collect()
                        };
                        ui.horizontal(|ui| {
                            ui.checkbox(duplicate, t("Duplicar perfil existente:"));
                            ui.add_enabled_ui(*duplicate, |ui| {
                                dropdown(ui, "duplicate-profile", &source.clone(), 240.0, |ui| {
                                    for choice in &choices {
                                        if ui.selectable_label(source == choice, choice).clicked() {
                                            *source = choice.clone();
                                            ui.close();
                                        }
                                    }
                                });
                            });
                        });
                        ui.add_space(32.0);
                        ui.horizontal(|ui| {
                            if dialog_button(ui, &t("Cancelar"), false, true) {
                                close = true;
                            }
                            let valid = !name.trim().is_empty()
                                && !choices.iter().any(|choice| choice.as_str() == name.trim());
                            if dialog_button(ui, &t("Salvar"), true, valid) {
                                let source = if *duplicate { source.clone() } else { "Padrão".into() };
                                let name = name.trim().to_owned();
                                self.dispatch(Request::EditProfile {
                                    edit: if *lighting {
                                        Edit::AddLighting { name: name.clone(), source }
                                    } else {
                                        Edit::AddCooling { name: name.clone(), source }
                                    },
                                });
                                let active = state.config.profiles.active.as_deref().unwrap_or("Default");
                                if let Some(profile) = state.config.profiles.sync.get(active) {
                                    let mut profile = profile.clone();
                                    if *lighting {
                                        profile.lighting = name;
                                    } else {
                                        profile.cooling = name;
                                    }
                                    self.dispatch(Request::SaveProfileAndActivate {
                                        name: active.into(),
                                        profile,
                                    });
                                }
                                close = true;
                            }
                        });
                    }
                }
            });
        if !close && !response.should_close() {
            self.profile_dialog = Some(dialog);
        }
    }

    fn program_dialog_content(
        &self,
        ui: &mut egui::Ui,
        state: &Snapshot,
        connected: &mut Vec<String>,
        search: &mut String,
        browsing: &mut bool,
        directory: &mut String,
    ) {
        let total = ui.available_width();
        let left = (total * 0.62).max(220.0);
        let right = (total - left - 25.0).max(140.0);
        ui.horizontal_top(|ui| {
            // Coluna esquerda: pesquisa ou navegação de pastas, altura fixa.
            ui.allocate_ui(Vec2::new(left, PROGRAM_LIST_HEIGHT + 56.0), |ui| {
                ui.set_width(left);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(search)
                            .hint_text(t("Pesquisar programas"))
                            .desired_width((left - 170.0).max(80.0)),
                    );
                    if ui
                        .add_sized([160.0, 36.0], egui::Button::new(t("Procurar programas")))
                        .clicked()
                    {
                        *browsing = true;
                        self.dispatch(Request::BrowsePrograms {
                            directory: (&*directory).into(),
                        });
                    }
                });
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .id_salt("program-results")
                    .max_height(PROGRAM_LIST_HEIGHT)
                    .min_scrolled_height(PROGRAM_LIST_HEIGHT)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if *browsing {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(directory)
                                        .desired_width((left - 100.0).max(80.0)),
                                );
                                if ui.button(t("Abrir")).clicked() {
                                    self.dispatch(Request::BrowsePrograms {
                                        directory: (&*directory).into(),
                                    });
                                }
                            });
                            if let Some(parent) = state.program_browser.directory.parent()
                                && ui.button(t("Pasta anterior")).clicked()
                            {
                                *directory = parent.to_string_lossy().into_owned();
                                self.dispatch(Request::BrowsePrograms {
                                    directory: parent.into(),
                                });
                            }
                            for entry in &state.program_browser.entries {
                                let name =
                                    entry.path.file_name().unwrap_or_default().to_string_lossy();
                                let label = if entry.directory {
                                    format!("{name}/")
                                } else {
                                    name.into_owned()
                                };
                                if ui.selectable_label(false, label).clicked() {
                                    if entry.directory {
                                        *directory = entry.path.to_string_lossy().into_owned();
                                        self.dispatch(Request::BrowsePrograms {
                                            directory: entry.path.clone(),
                                        });
                                    } else {
                                        let path = entry.path.to_string_lossy().into_owned();
                                        if !connected.contains(&path) {
                                            connected.push(path);
                                        }
                                        *browsing = false;
                                    }
                                }
                            }
                            if ui.button(t("Voltar à pesquisa")).clicked() {
                                *browsing = false;
                            }
                        } else if search.trim().is_empty() {
                            ui.add_space(PROGRAM_LIST_HEIGHT / 2.0 - 12.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(t("Pesquise um jogo ou programa"))
                                        .color(SECONDARY),
                                );
                            });
                        } else {
                            let needle = search.to_lowercase();
                            let mut found = false;
                            for program in state.programs.iter().filter(|p| {
                                p.name.to_lowercase().contains(&needle)
                                    || p.executable.to_lowercase().contains(&needle)
                            }) {
                                found = true;
                                let selected = connected.contains(&program.executable);
                                egui::Frame::new()
                                    .fill(BACKGROUND)
                                    .stroke(Stroke::new(
                                        1.0,
                                        if selected { ORCHID } else { BORDER },
                                    ))
                                    .inner_margin(12)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&program.name).size(17.0))
                                                .on_hover_text(&program.executable);
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    let text = t(if selected {
                                                        "Remover"
                                                    } else {
                                                        "Conectar"
                                                    });
                                                    if dialog_button(ui, &text, !selected, true) {
                                                        if selected {
                                                            connected.retain(|exe| {
                                                                exe != &program.executable
                                                            });
                                                        } else {
                                                            connected
                                                                .push(program.executable.clone());
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(8.0);
                            }
                            if !found {
                                ui.label(t("Nenhum programa encontrado"));
                            }
                        }
                    });
            });
            // Divisória com a mesma altura das colunas (não ocupa a janela inteira).
            let (line, _) =
                ui.allocate_exact_size(Vec2::new(25.0, PROGRAM_LIST_HEIGHT + 56.0), Sense::hover());
            ui.painter().line_segment(
                [line.center_top(), line.center_bottom()],
                Stroke::new(1.0, BORDER),
            );
            ui.allocate_ui(Vec2::new(right, PROGRAM_LIST_HEIGHT + 56.0), |ui| {
                ui.set_width(right);
                ui.label(RichText::new(t("Programas conectados")).strong());
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .id_salt("connected-programs")
                    .max_height(PROGRAM_LIST_HEIGHT)
                    .show(ui, |ui| {
                        let mut remove = None;
                        for (index, executable) in connected.iter().enumerate() {
                            let name = state
                                .programs
                                .iter()
                                .find(|p| &p.executable == executable)
                                .map(|p| p.name.as_str())
                                .unwrap_or_else(|| {
                                    executable.rsplit('/').next().unwrap_or(executable)
                                });
                            egui::Frame::new()
                                .fill(PURPLE)
                                .inner_margin(10)
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(name).on_hover_text(executable);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button("×").clicked() {
                                                    remove = Some(index);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(8.0);
                        }
                        if let Some(index) = remove {
                            connected.remove(index);
                        }
                        if connected.is_empty() {
                            ui.label(
                                RichText::new(t("Nenhum programa conectado")).color(SECONDARY),
                            );
                        }
                    });
            });
        });
    }
}

fn time_input(ui: &mut egui::Ui, id: &str, value: &mut u16) {
    let (mut hour, mut minute) = (*value / 60, *value % 60);
    ui.push_id(id, |ui| {
        egui::Frame::new()
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(8)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut hour)
                            .range(0..=23)
                            .min_decimals(0)
                            .custom_formatter(|n, _| format!("{:02}", n as u16)),
                    );
                    ui.label(":");
                    ui.add(
                        egui::DragValue::new(&mut minute)
                            .range(0..=59)
                            .min_decimals(0)
                            .custom_formatter(|n, _| format!("{:02}", n as u16)),
                    );
                });
            });
    });
    *value = hour * 60 + minute;
}
