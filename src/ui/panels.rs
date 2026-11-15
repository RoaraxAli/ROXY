//! Models and Settings.

use crate::app::{App, SettingsTab};
use crate::catalog;
use crate::cli::{self, JobKind, State};
use crate::model::*;
use crate::store;
use crate::theme::*;
use crate::ui::widgets::*;
use egui::{Align, CornerRadius, Layout, Margin, RichText, Sense, Stroke, Ui, Vec2};

fn page(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("page_scroll")
        .show(ui, |ui| {
            let full = ui.available_width();
            let w = (full - 64.0).clamp(320.0, 900.0);
            ui.horizontal(|ui| {
                ui.add_space(((full - w) * 0.5).max(24.0));
                ui.vertical(|ui| {
                    ui.set_width(w);
                    ui.add_space(26.0);
                    add(ui);
                    ui.add_space(40.0);
                });
            });
        });
}

fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = (ui.available_width() - 34.0).max(120.0);
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(CornerRadius::same(13))
        .inner_margin(Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_min_width(w);
            add(ui)
        })
        .inner
}

fn header(ui: &mut Ui, title: &str, sub: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(font_semi(25.0)).color(TEXT));
            ui.add_space(3.0);
            ui.label(RichText::new(sub).font(font(13.0)).color(TEXT_DIM));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(2.0);
            right(ui);
        });
    });
    ui.add_space(18.0);
}

fn hair(ui: &mut Ui) {
    ui.add_space(10.0);
    hairline(ui, BORDER_SOFT);
    ui.add_space(10.0);
}

pub fn view(app: &mut App, ui: &mut Ui) {
    match app.view {
        crate::app::View::Models => models(app, ui),
        crate::app::View::Settings => settings(app, ui),
        crate::app::View::Chat => {}
    }
}

// ------------------------------------------------------------------ models

fn models(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    let mut changed = false;
    let mut refresh = false;
    let mut goto_api = false;

    page(ui, |ui| {
        header(
            ui,
            "Models",
            "Install an agent on this machine, or plug in a key you already own. Every source shares one session.",
            |ui| {
                if small_button(ui, "Refresh", Some(Icon::Refresh), TEXT_DIM).clicked() {
                    refresh = true;
                }
                if small_button(ui, "Add provider", Some(Icon::Plus), TEXT_DIM).clicked() {
                    goto_api = true;
                }
            },
        );

        let labels = ["CLI agents", "API models"];
        if let Some(t) = segmented(ui, &labels, app.models_tab as usize, accent) {
            app.models_tab = t as u8;
        }
        ui.add_space(16.0);

        if app.models_tab == 0 {
            cli_section(app, ui, &mut changed, &mut refresh);
        } else {
            gateway(app, ui);
        }

        ui.add_space(14.0);
        handoff_card(ui, accent);
    });

    if refresh {
        app.refresh_all(ui.ctx());
    }
    if goto_api {
        app.models_tab = 1;
    }
    if changed {
        app.persist();
    }
}

fn cli_section(app: &mut App, ui: &mut Ui, changed: &mut bool, refresh: &mut bool) {
    let accent = app.accent;
    let ready = app
        .cfg
        .providers
        .iter()
        .filter(|p| p.is_cli() && p.ready)
        .count();

    ui.horizontal(|ui| {
        section_label(ui, "LOCAL AGENTS", TEXT_FAINT);
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("{ready}/{} installed", agent_count()))
                .font(mono(9.5))
                .color(TEXT_FAINT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !app.node_ok && small_button(ui, "Get Node.js", Some(Icon::Arrow), WARN).clicked() {
                cli::open_url("https://nodejs.org/en/download");
            }
            if app.node_ok {
                ui.label(
                    RichText::new(format!("node {}", app.node_version))
                        .font(mono(9.5))
                        .color(OK.gamma_multiply(0.85)),
                );
            } else {
                ui.label(
                    RichText::new("Node.js not found")
                        .font(mono(9.5))
                        .color(WARN),
                );
            }
        });
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "ROXY installs each CLI into its own folder inside your ROXY data directory — no admin rights, nothing added to your PATH, nothing shared with anyone else.",
        )
        .font(font(11.5))
        .color(TEXT_FAINT),
    );
    ui.add_space(12.0);

    for spec in cli::SPECS.iter().filter(|s| !s.server) {
        let idx = app.cfg.providers.iter().position(|p| p.cli == spec.id);
        let detect = app.clis.get(spec.id).cloned();
        let job_idx = app.jobs.iter().position(|j| j.cli == spec.id);
        let open = app.open_cli.as_deref() == Some(spec.id);

        card(ui, |ui| {
            ui.horizontal(|ui| {
                provider_logo(ui, spec.id, spec.vendor, accent_of(spec.color), 36.0);
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(spec.name).font(font_semi(14.5)).color(TEXT));
                        ui.add_space(2.0);
                        ui.label(RichText::new(spec.vendor).font(mono(9.5)).color(TEXT_FAINT));
                        ui.add_space(3.0);
                        if let Some(d) = &detect {
                            match d.state {
                                State::Ready => pill(
                                    ui,
                                    if d.version.is_empty() {
                                        "installed"
                                    } else {
                                        &d.version
                                    },
                                    Some(OK),
                                    OK,
                                    BG_DEEP,
                                ),
                                State::Missing => {}
                                State::Checking => pill(ui, "checking", Some(WARN), WARN, BG_DEEP),
                            }
                        }
                        if let Some(j) = job_idx.map(|i| &app.jobs[i]) {
                            if !j.done {
                                pill(ui, "installing…", Some(accent), accent, BG_DEEP);
                            } else if !j.ok {
                                pill(ui, "failed", Some(DANGER), DANGER, BG_DEEP);
                            }
                        }
                    });
                    ui.label(RichText::new(spec.blurb).font(font(11.5)).color(TEXT_FAINT));
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut toggle = false;
                    if icon_btn(
                        ui,
                        if open { Icon::Close } else { Icon::Arrow },
                        24.0,
                        TEXT_FAINT,
                    )
                    .on_hover_text(if open { "Hide details" } else { "Details" })
                    .clicked()
                    {
                        toggle = true;
                    }
                    let busy = job_idx.map(|i| !app.jobs[i].done).unwrap_or(false);
                    let installed = detect
                        .as_ref()
                        .map(|d| d.state == State::Ready)
                        .unwrap_or(false);
                    if busy {
                        ui.label(RichText::new("working…").font(mono(10.0)).color(TEXT_FAINT));
                    } else if installed {
                        if small_button(ui, "Sign in", None, TEXT_DIM)
                            .on_hover_text(format!("Run `{}` in a terminal", spec.login))
                            .clicked()
                        {
                            if let Some(i) = idx {
                                let path = app.cfg.providers[i].cli.clone();
                                let p = app.clis.get(&path).and_then(|d| d.path.clone());
                                cli::open_login(spec, p.as_deref());
                            } else {
                                cli::open_login(spec, None);
                            }
                        }
                        if small_button(ui, "Update", None, TEXT_DIM).clicked() {
                            app.start_install(spec, JobKind::Update, ui.ctx());
                        }
                    } else if !spec.pkg.is_empty() {
                        if action_button(ui, "Install", accent, true, Some(86.0)).clicked() {
                            if app.node_ok {
                                app.start_install(spec, JobKind::Install, ui.ctx());
                            } else {
                                app.toast = Some((
                                    format!(
                                        "{} needs Node.js — opening the download page.",
                                        spec.name
                                    ),
                                    5.0,
                                ));
                                cli::open_url("https://nodejs.org/en/download");
                            }
                        }
                    }
                    if toggle {
                        app.open_cli = if open {
                            None
                        } else {
                            Some(spec.id.to_string())
                        };
                    }
                });
            });

            // install log
            if let Some(j) = job_idx.map(|i| &app.jobs[i]) {
                if !j.lines.is_empty() {
                    ui.add_space(8.0);
                    let tail: Vec<String> = j.lines.iter().rev().take(6).rev().cloned().collect();
                    code_block(ui, &tail.join("\n"), 92.0);
                }
            }

            if open {
                hair(ui);
                if let Some(i) = idx {
                    let before = app.cfg.providers[i].model.clone();
                    let mut new_model = before.clone();
                    let mut models = app.cfg.providers[i].models.clone();
                    models.retain(|m| !m.eq_ignore_ascii_case("auto"));
                    let mut on = app.cfg.providers[i].enabled;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            section_label(ui, "MODEL", TEXT_FAINT);
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt(("cli-model", spec.id))
                                    .selected_text(
                                        RichText::new(
                                            if new_model == "auto" || new_model.is_empty() {
                                                "CLI default"
                                            } else {
                                                &new_model
                                            },
                                        )
                                        .font(mono(11.5))
                                        .color(TEXT),
                                    )
                                    .width(210.0)
                                    .show_ui(ui, |ui| {
                                        for m in &models {
                                            ui.selectable_value(
                                                &mut new_model,
                                                m.clone(),
                                                RichText::new(m).font(mono(11.5)),
                                            );
                                        }
                                    });
                            });
                        });
                        ui.add_space(18.0);
                        ui.vertical(|ui| {
                            section_label(ui, "IN THE PICKER", TEXT_FAINT);
                            switch(ui, &mut on, accent);
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if small_button(ui, "Uninstall", None, TEXT_FAINT).clicked() {
                                app.start_install(spec, JobKind::Uninstall, ui.ctx());
                            }
                            if small_button(ui, "Docs", Some(Icon::Info), TEXT_FAINT).clicked() {
                                cli::open_url(spec.docs);
                            }
                            if small_button(ui, "Copy sign-in", None, TEXT_FAINT).clicked() {
                                ui.ctx().copy_text(spec.login.to_string());
                            }
                        });
                    });
                    if new_model != before || on != app.cfg.providers[i].enabled {
                        let p = &mut app.cfg.providers[i];
                        p.model = new_model;
                        p.enabled = on;
                        *changed = true;
                    }
                    ui.add_space(8.0);
                    let detect = app.clis.get(spec.id);
                    let path = detect
                        .and_then(|d| d.path.clone())
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "not found on PATH".to_string());
                    let source = match detect {
                        Some(d) if d.state == State::Ready && d.managed => "installed by ROXY",
                        Some(d) if d.state == State::Ready => "found on PATH",
                        Some(_) => "not installed",
                        None => "",
                    };
                    ui.label(
                        RichText::new(format!("{}  ·  {}  ·  {}", spec.pkg, path, source))
                            .font(mono(9.5))
                            .color(TEXT_FAINT),
                    );
                } else {
                    ui.label(
                        RichText::new(
                            "Install this agent to route sessions through it. ROXY drives it headlessly and replays the full transcript on every turn.",
                        )
                        .font(font(11.5))
                        .color(TEXT_DIM),
                    );
                }
            }
        });
        ui.add_space(9.0);
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if small_button(
            ui,
            "Re-scan installed CLIs",
            Some(Icon::Refresh),
            TEXT_FAINT,
        )
        .clicked()
        {
            for spec in cli::SPECS {
                app.redetect(spec.id, ui.ctx());
            }
            *refresh = true;
        }
        ui.label(
            RichText::new(
                "Sign-in happens in your own terminal — ROXY never asks for CLI credentials.",
            )
            .font(font(10.5))
            .color(TEXT_FAINT),
        );
    });
}

/// Decode a bundled, official provider mark once and keep its texture in egui's
/// temporary cache. Known CLI vendors use their real logo files.
fn provider_logo(ui: &mut Ui, identity: &str, name: &str, color: egui::Color32, size: f32) {
    let key = format!("{identity} {name}").to_lowercase();
    let (asset, bytes): (&str, &[u8]) = if key.contains("codex") || key.contains("openai") {
        ("openai", include_bytes!("../../assets/logos/openai.png"))
    } else if key.contains("claude") || key.contains("anthropic") {
        (
            "anthropic",
            include_bytes!("../../assets/logos/anthropic.png"),
        )
    } else if key.contains("gemini") || key.contains("google") {
        ("google", include_bytes!("../../assets/logos/google.png"))
    } else if key.contains("qwen") || key.contains("alibaba") {
        (
            "alibabacloud",
            include_bytes!("../../assets/logos/alibabacloud.png"),
        )
    } else if key.contains("copilot") || key.contains("github") {
        ("github", include_bytes!("../../assets/logos/github.png"))
    } else if key.contains("opencode") || key.contains("sst") {
        (
            "opencode",
            include_bytes!("../../assets/logos/opencode.png"),
        )
    } else if key.contains("amp") || key.contains("sourcegraph") {
        (
            "sourcegraph",
            include_bytes!("../../assets/logos/sourcegraph.png"),
        )
    } else if key.contains("charm") {
        ("charm", include_bytes!("../../assets/logos/charm.png"))
    } else if key.contains("deepseek") {
        (
            "deepseek",
            include_bytes!("../../assets/logos/deepseek.png"),
        )
    } else if key.contains("meta") || key.contains("llama") {
        ("meta", include_bytes!("../../assets/logos/meta.png"))
    } else if key.contains("openrouter") {
        (
            "openrouter",
            include_bytes!("../../assets/logos/openrouter.png"),
        )
    } else if key.contains("xai") || key.contains("grok") {
        ("x", include_bytes!("../../assets/logos/x.png"))
    } else if key.contains("mistral") {
        (
            "mistralai",
            include_bytes!("../../assets/logos/mistralai.png"),
        )
    } else if key.contains("perplexity") {
        (
            "perplexity",
            include_bytes!("../../assets/logos/perplexity.png"),
        )
    } else if key.contains("hugging face") || key.contains("huggingface") {
        (
            "huggingface",
            include_bytes!("../../assets/logos/huggingface.png"),
        )
    } else if key.contains("nvidia") {
        ("nvidia", include_bytes!("../../assets/logos/nvidia.png"))
    } else {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same((size * 0.28) as u8), SURFACE_HI);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            name.chars()
                .find(|ch| ch.is_alphanumeric())
                .unwrap_or('?')
                .to_uppercase()
                .to_string(),
            font_semi(size * 0.43),
            color,
        );
        return;
    };
    let id = egui::Id::new(("provider-logo", asset));
    let texture = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id));
    let texture = texture.unwrap_or_else(|| {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().expect("bundled logo PNG is valid");
        let mut buffer = vec![
            0;
            reader
                .output_buffer_size()
                .expect("bundled logo size is valid")
        ];
        let info = reader
            .next_frame(&mut buffer)
            .expect("bundled logo PNG decodes");
        let mut rgba = vec![0; (info.width * info.height * 4) as usize];
        match info.color_type {
            png::ColorType::Rgba => {
                let len = rgba.len();
                rgba.copy_from_slice(&buffer[..len]);
            }
            png::ColorType::Rgb => {
                for (src, dst) in buffer[..(info.width * info.height * 3) as usize]
                    .chunks_exact(3)
                    .zip(rgba.chunks_exact_mut(4))
                {
                    dst[..3].copy_from_slice(src);
                    dst[3] = 255;
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for (src, dst) in buffer[..(info.width * info.height * 2) as usize]
                    .chunks_exact(2)
                    .zip(rgba.chunks_exact_mut(4))
                {
                    dst[..3].fill(src[0]);
                    dst[3] = src[1];
                }
            }
            png::ColorType::Grayscale => {
                for (src, dst) in buffer[..(info.width * info.height) as usize]
                    .iter()
                    .zip(rgba.chunks_exact_mut(4))
                {
                    dst[..3].fill(*src);
                    dst[3] = 255;
                }
            }
            _ => unreachable!(),
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [info.width as usize, info.height as usize],
            &rgba,
        );
        let handle = ui.ctx().load_texture(
            format!("provider-{asset}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        ui.ctx().data_mut(|d| d.insert_temp(id, handle.clone()));
        handle
    });
    ui.add(
        egui::Image::new((texture.id(), Vec2::splat(size))).fit_to_exact_size(Vec2::splat(size)),
    );
}

fn handoff_card(ui: &mut Ui, accent: egui::Color32) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            section_label(ui, "HOW SWITCHING WORKS", accent.gamma_multiply(0.85));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("provider-agnostic transcript")
                        .font(mono(9.5))
                        .color(TEXT_FAINT),
                );
            });
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Sessions are stored once, in neutral form. Choosing another model only changes where the next turn is sent, and every request carries the entire conversation — so a new model starts fully briefed. Handoff markers stay in the transcript but are never sent to a model.",
            )
            .font(font(12.5))
            .color(TEXT_DIM)
            .line_height(Some(19.0)),
        );
    });
}

fn agent_count() -> usize {
    cli::SPECS.iter().filter(|s| !s.server).count()
}

fn accent_of(name: &str) -> egui::Color32 {
    let p = Provider::new("", "", "", "", "", name);
    p.accent()
}

// ---------------------------------------------------------------- gateway

/// 9Router: the local priority router. Users order the models once; ROXY
/// falls through that order whenever a turn fails or is rate-limited.
fn router_card(app: &mut App, ui: &mut Ui, idx: usize, changed: &mut bool) {
    let accent = app.accent;
    let spec = cli::spec(cli::ROUTER_ID);
    let detect = app.clis.get(cli::ROUTER_ID).cloned();
    let installed = detect
        .as_ref()
        .map(|d| d.state == State::Ready)
        .unwrap_or(false);
    let job_idx = app.jobs.iter().position(|j| j.cli == cli::ROUTER_ID);
    let job = job_idx.map(|i| (app.jobs[i].done, app.jobs[i].ok, app.jobs[i].lines.len()));
    let online = app.cfg.providers[idx].status;

    let ordered = app.cfg.providers[idx].ordered_models();
    let mut order = ordered.clone();
    let mut start = false;
    let mut install = false;
    let mut dashboard = false;
    let mut refresh = false;
    let mut clear = false;
    let mut move_up: Option<usize> = None;
    let mut move_down: Option<usize> = None;

    card(ui, |ui| {
        ui.horizontal(|ui| {
            provider_logo(ui, "9router router", "9Router", accent_of("violet"), 36.0);
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("9Router").font(font_semi(14.5)).color(TEXT));
                    ui.add_space(2.0);
                    ui.label(RichText::new("local router").font(mono(9.5)).color(TEXT_FAINT));
                    ui.add_space(3.0);
                    match online {
                        Some(true) => pill(ui, "online", Some(OK), OK, BG_DEEP),
                        Some(false) => pill(ui, "offline", Some(WARN), WARN, BG_DEEP),
                        None => pill(ui, "not started", Some(TEXT_FAINT), TEXT_FAINT, BG_DEEP),
                    }
                    if let Some((done, ok, _)) = job {
                        if !done {
                            pill(ui, "installing…", Some(accent), accent, BG_DEEP);
                        } else if !ok {
                            pill(ui, "install failed", Some(DANGER), DANGER, BG_DEEP);
                        }
                    }
                });
                ui.label(
                    RichText::new(
                        "Rank your models once. Requests walk this order and fall through automatically when one is down or rate-limited.",
                    )
                    .font(font(11.5))
                    .color(TEXT_FAINT),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if installed {
                    if small_button(ui, "Start server", None, TEXT_DIM).clicked() {
                        start = true;
                    }
                } else if small_button(ui, "Install", None, TEXT_DIM).clicked() {
                    install = true;
                }
                if small_button(ui, "Dashboard", Some(Icon::Arrow), TEXT_DIM).clicked() {
                    dashboard = true;
                }
                if small_button(ui, "Refresh", Some(Icon::Refresh), TEXT_DIM).clicked() {
                    refresh = true;
                }
            });
        });

        if let (Some(i), Some((_, _, n))) = (job_idx, job) {
            if n > 0 {
                ui.add_space(8.0);
                let tail: Vec<String> = app.jobs[i]
                    .lines
                    .iter()
                    .rev()
                    .take(4)
                    .rev()
                    .cloned()
                    .collect();
                code_block(ui, &tail.join("\n"), 70.0);
            }
        }

        hair(ui);
        ui.horizontal(|ui| {
            section_label(ui, "MODEL PRIORITY", TEXT_FAINT);
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("{} ranked", order.len()))
                    .font(mono(9.5))
                    .color(TEXT_FAINT),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !order.is_empty() && small_button(ui, "Reset order", None, TEXT_FAINT).clicked()
                {
                    clear = true;
                }
            });
        });
        ui.add_space(4.0);

        if order.is_empty() {
            ui.label(
                RichText::new(if online == Some(false) {
                    "Start 9Router and press Refresh to pull its model list."
                } else {
                    "No models yet. Add providers in the 9Router dashboard, then press Refresh."
                })
                .font(font(11.5))
                .color(TEXT_FAINT),
            );
        } else {
            let shown: Vec<(usize, String)> = order.iter().cloned().enumerate().take(40).collect();
            egui::ScrollArea::vertical()
                .id_salt("router-priority")
                .max_height(250.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (rank, name) in shown {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:02}", rank + 1))
                                    .font(mono(9.5))
                                    .color(TEXT_FAINT),
                            );
                            ui.add_space(6.0);
                            ui.label(RichText::new(name.clone()).font(mono(11.5)).color(TEXT));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new("↓").font(mono(11.0)))
                                            .frame(false),
                                    )
                                    .on_hover_text("Lower priority")
                                    .clicked()
                                {
                                    move_down = Some(rank);
                                }
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new("↑").font(mono(11.0)))
                                            .frame(false),
                                    )
                                    .on_hover_text("Higher priority")
                                    .clicked()
                                {
                                    move_up = Some(rank);
                                }
                            });
                        });
                    }
                });
            if order.len() > 40 {
                ui.label(
                    RichText::new(format!("+{} more", order.len() - 40))
                        .font(mono(9.0))
                        .color(TEXT_FAINT),
                );
            }
        }
    });

    if let Some(i) = move_up {
        if i > 0 {
            order.swap(i, i - 1);
        }
    }
    if let Some(i) = move_down {
        if i + 1 < order.len() {
            order.swap(i, i + 1);
        }
    }
    if clear {
        order.clear();
    }
    if order != ordered {
        app.cfg.providers[idx].priority = order;
        *changed = true;
    }

    if install {
        if let Some(spec) = spec {
            if !app.node_ok {
                app.toast = Some((
                    "Install Node.js first — 9Router is an npm package.".into(),
                    5.0,
                ));
                cli::open_url("https://nodejs.org/en/download");
            } else {
                app.start_install(spec, JobKind::Install, ui.ctx());
            }
        }
    }
    if start {
        if let Some(spec) = spec {
            let path = detect.as_ref().and_then(|d| d.path.clone());
            cli::open_login(spec, path.as_deref());
            app.toast = Some(("9Router is starting — it opens on port 20128.".into(), 5.0));
        }
    }
    if dashboard {
        cli::open_url(cli::ROUTER_DASHBOARD);
    }
    if refresh {
        app.refresh_router(ui.ctx());
    }
}

fn gateway(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    let mut changed = false;
    let mut test: Option<usize> = None;
    let mut remove: Option<usize> = None;

    if let Some(ri) = app.cfg.providers.iter().position(|p| p.is_router()) {
        router_card(app, ui, ri, &mut changed);
        ui.add_space(16.0);
    }

    ui.horizontal(|ui| {
        section_label(ui, "YOUR INTEGRATIONS", TEXT_FAINT);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if small_button(ui, "Re-test all", Some(Icon::Refresh), TEXT_DIM).clicked() {
                app.refresh_all(ui.ctx());
            }
        });
    });
    ui.add_space(8.0);

    let ids: Vec<usize> = (0..app.cfg.providers.len())
        .filter(|i| !app.cfg.providers[*i].is_cli() && !app.cfg.providers[*i].is_router())
        .collect();

    if ids.is_empty() {
        card(ui, |ui| {
            ui.label(
                RichText::new("Nothing connected yet.")
                    .font(font_med(13.5))
                    .color(TEXT),
            );
            ui.add_space(3.0);
            ui.label(
                    RichText::new(
                        "Pick a provider below, paste a key, and it becomes available in the model picker. Or install a local agent in Models → CLI agents.",
                    )
                    .font(font(12.0))
                    .color(TEXT_DIM),
                );
        });
        ui.add_space(10.0);
    }

    for i in ids {
        let (
            name,
            short,
            color,
            host,
            model,
            mut models,
            key_ok,
            key_present,
            env,
            note,
            status,
            latency,
        ) = {
            let p = &app.cfg.providers[i];
            (
                p.name.clone(),
                p.short.clone(),
                p.accent(),
                p.host(),
                p.model.clone(),
                p.visible_models(),
                p.key_present() || p.local(),
                p.key_present(),
                p.key_env.clone(),
                p.note.clone(),
                p.status,
                p.latency,
            )
        };
        models.sort();
        card(ui, |ui| {
            ui.horizontal(|ui| {
                provider_logo(ui, &format!("{name} {short} {host}"), &name, color, 30.0);
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&name).font(font_semi(14.0)).color(TEXT));
                        ui.add_space(2.0);
                        pill(ui, &host, None, TEXT_FAINT, BG_DEEP);
                        if key_ok {
                            pill(
                                ui,
                                if key_present { "key set" } else { "local" },
                                Some(OK),
                                OK,
                                BG_DEEP,
                            );
                        } else {
                            pill(ui, "needs key", Some(WARN), WARN, BG_DEEP);
                        }
                    });
                    ui.add_space(1.0);
                    ui.label(RichText::new(&note).font(mono(9.5)).color(TEXT_FAINT));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_btn(ui, Icon::Trash, 22.0, TEXT_FAINT)
                        .on_hover_text("Remove integration")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                    if icon_btn(ui, Icon::Refresh, 22.0, TEXT_FAINT)
                        .on_hover_text("Test endpoint")
                        .clicked()
                    {
                        test = Some(i);
                    }
                    let (s, c) = match status {
                        Some(true) => (format!("{} ms", latency.unwrap_or(0)), OK),
                        Some(false) => ("unreachable".into(), DANGER),
                        None => ("idle".into(), TEXT_FAINT),
                    };
                    ui.label(RichText::new(s).font(mono(9.5)).color(c));
                });
            });

            ui.add_space(11.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    section_label(ui, "API KEY", TEXT_FAINT);
                    ui.horizontal(|ui| {
                        let resp = egui::Frame::new()
                            .fill(BG_DEEP)
                            .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(9, 5))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut app.cfg.providers[i].api_key)
                                        .password(true)
                                        .frame(false)
                                        .desired_width(240.0)
                                        .hint_text(
                                            RichText::new(if env.is_empty() {
                                                "paste your key".to_string()
                                            } else {
                                                format!("paste, or leave blank to use {env}")
                                            })
                                            .font(mono(11.0))
                                            .color(TEXT_FAINT),
                                        )
                                        .font(mono(11.5))
                                        .text_color(TEXT)
                                        .margin(Margin::ZERO),
                                )
                            })
                            .inner;
                        if resp.changed() {
                            *&mut changed = true;
                        }
                    });
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    section_label(ui, "DEFAULT MODEL", TEXT_FAINT);
                    let mut sel = model.clone();
                    egui::ComboBox::from_id_salt(("gw-model", i))
                        .selected_text(
                            RichText::new(if sel.is_empty() {
                                "fetch or type a model"
                            } else {
                                &sel
                            })
                            .font(mono(11.0))
                            .color(TEXT),
                        )
                        .width(210.0)
                        .show_ui(ui, |ui| {
                            for m in &models {
                                ui.selectable_value(
                                    &mut sel,
                                    m.clone(),
                                    RichText::new(m).font(mono(11.0)),
                                );
                            }
                        });
                    if sel != model {
                        app.cfg.providers[i].model = sel;
                        changed = true;
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut on = app.cfg.providers[i].enabled;
                    if switch(ui, &mut on, accent) {
                        app.cfg.providers[i].enabled = on;
                        changed = true;
                    }
                    ui.label(RichText::new("enabled").font(font(11.0)).color(TEXT_FAINT));
                });
            });
        });
        ui.add_space(9.0);
    }

    ui.add_space(10.0);
    section_label(ui, "ADD AN INTEGRATION", TEXT_FAINT);
    ui.add_space(8.0);
    card(ui, |ui| {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
            draw_icon(ui.painter(), r.center(), 13.0, Icon::Search, TEXT_FAINT);
            ui.add(
                egui::TextEdit::singleline(&mut app.cat_search)
                    .hint_text(
                        RichText::new(
                            "Search 200+ providers — openai, anthropic, openrouter, ollama…",
                        )
                        .font(font(12.5))
                        .color(TEXT_FAINT),
                    )
                    .frame(false)
                    .desired_width(ui.available_width() - 120.0)
                    .font(font(12.5))
                    .text_color(TEXT),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let total = catalog::all().len();
                let label = if app.cat_all {
                    "Show less".to_string()
                } else {
                    format!("Show all {total}")
                };
                if small_button(ui, &label, None, TEXT_FAINT).clicked() {
                    app.cat_all = !app.cat_all;
                }
            });
        });
        ui.add_space(8.0);
        hairline(ui, BORDER_SOFT);
        ui.add_space(8.0);

        let q = app.cat_search.trim().to_lowercase();
        let added: Vec<String> = app
            .cfg
            .providers
            .iter()
            .filter(|p| !p.is_cli())
            .map(|p| p.catalog.clone())
            .collect();
        let mut picks: Vec<&catalog::Entry> = catalog::all()
            .iter()
            .filter(|e| {
                q.is_empty()
                    || e.id.contains(&q)
                    || e.name.to_lowercase().contains(&q)
                    || e.api.to_lowercase().contains(&q)
            })
            .collect();
        let limit = if app.cat_all || !q.is_empty() {
            240
        } else {
            12
        };
        let shown = picks.len().min(limit);
        picks.truncate(shown);

        let full = ui.available_width();
        let gap = 8.0;
        let tw = ((full - gap * 2.0 - 2.0) / 3.0).max(140.0);
        ui.horizontal_wrapped(|ui| {
            for e in picks {
                let is_added = added.iter().any(|a| a == &e.id);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(tw, 46.0), Sense::click());
                let p = ui.painter();
                let hov = resp.hovered();
                p.rect(
                    rect,
                    CornerRadius::same(9),
                    if hov { SURFACE_HI } else { SURFACE },
                    Stroke::new(1.0_f32, if hov { BORDER } else { BORDER_SOFT }),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    egui::pos2(rect.left() + 10.0, rect.top() + 9.0),
                    egui::Align2::LEFT_TOP,
                    truncate(&e.name, 22),
                    font_med(11.5),
                    if is_added { TEXT_FAINT } else { TEXT },
                );
                p.text(
                    egui::pos2(rect.left() + 10.0, rect.top() + 27.0),
                    egui::Align2::LEFT_TOP,
                    truncate(&e.api, 34),
                    mono(8.5),
                    TEXT_FAINT,
                );
                if is_added {
                    p.circle_filled(egui::pos2(rect.right() - 12.0, rect.center().y), 3.0, OK);
                } else if hov {
                    draw_icon(
                        p,
                        egui::pos2(rect.right() - 13.0, rect.center().y),
                        12.0,
                        Icon::Plus,
                        accent,
                    );
                }
                if hov {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() && !is_added {
                    app.cfg.providers.push(catalog::provider(e));
                    app.cat_open = Some(e.id.clone());
                    changed = true;
                    app.toast = Some((format!("{} added — paste its key", e.name), 4.0));
                }
            }
        });
        if !app.cat_all && q.is_empty() {
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Featured providers. Search to find the rest, or type any host below.",
                )
                .font(font(10.5))
                .color(TEXT_FAINT),
            );
        }
    });

    ui.add_space(12.0);
    custom_form(app, ui, &mut changed);

    ui.add_space(12.0);
    usage_card(app, ui);
    if let Some(i) = test {
        if let (Some(p), Some(tx)) = (app.cfg.providers.get(i).cloned(), app.disc_tx.clone()) {
            crate::llm::spawn_discovery(p, tx, ui.ctx().clone());
            app.pending_discovery += 1;
        }
    }
    if let Some(i) = remove {
        if i < app.cfg.providers.len() {
            let gone = app.cfg.providers[i].id.clone();
            app.cfg.providers.remove(i);
            app.route_after_removal(&gone);
            changed = true;
        }
    }
    if changed {
        app.persist();
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

fn custom_form(app: &mut App, ui: &mut Ui, changed: &mut bool) {
    let accent = app.accent;
    let open = app.cat_open.as_deref() == Some("__custom");
    card(ui, |ui| {
        ui.horizontal(|ui| {
            avatar(ui, "API", TEXT_FAINT, 26.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Any other OpenAI-compatible endpoint")
                        .font(font_med(13.0))
                        .color(TEXT),
                );
                ui.label(
                    RichText::new("A private gateway, a local server, a new host — name it, point at it, done.")
                        .font(font(11.0))
                        .color(TEXT_FAINT),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if small_button(
                    ui,
                    if open { "Close" } else { "Add custom" },
                    Some(Icon::Plus),
                    TEXT_DIM,
                )
                .clicked()
                {
                    if open {
                        app.cat_open = None;
                    } else {
                        app.cat_open = Some("__custom".into());
                        app.adding = Some(Provider::new(
                            "",
                            "",
                            "",
                            "https://",
                            "",
                            "violet",
                        ));
                    }
                }
            });
        });

        if open {
            let Some(form) = app.adding.as_mut() else {
                return;
            };
            hair(ui);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    section_label(ui, "NAME", TEXT_FAINT);
                    field_ui(ui, &mut form.name, 150.0);
                    section_label(ui, "BASE URL", TEXT_FAINT);
                    field_ui(ui, &mut form.base_url, 260.0);
                });
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    section_label(ui, "API KEY (OPTIONAL)", TEXT_FAINT);
                    password_ui(ui, &mut form.api_key, 200.0);
                    section_label(ui, "FIRST MODEL ID", TEXT_FAINT);
                    field_ui(ui, &mut form.model, 200.0);
                });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if action_button(ui, "Add provider", accent, true, None).clicked() {
                    let name = if form.name.trim().is_empty() {
                        "Custom".to_string()
                    } else {
                        form.name.trim().to_string()
                    };
                    let mut p = form.clone();
                    p.id = format!("api-{}", slug(&name));
                    if app.cfg.providers.iter().any(|x| x.id == p.id) {
                        p.id = format!("api-{}-{}", slug(&name), short_id());
                    }
                    p.name = name.clone();
                    p.short = initials(&name);
                    p.catalog = "custom".into();
                    p.enabled = true;
                    p.ready = true;
                    p.models = if p.model.trim().is_empty() {
                        Vec::new()
                    } else {
                        vec![p.model.trim().to_string()]
                    };
                    p.model = p.models.first().cloned().unwrap_or_default();
                    let key_ok = !p.api_key.trim().is_empty() || p.local();
                    app.cfg.providers.push(p.clone());
                    app.cat_open = None;
                    *changed = true;
                    app.toast = Some((
                        if key_ok {
                            format!("{name} connected")
                        } else {
                            format!("{name} added — paste its key above")
                        },
                        3.5,
                    ));
                    if key_ok {
                        if let Some(tx) = app.disc_tx.clone() {
                            crate::llm::spawn_discovery(p, tx, ui.ctx().clone());
                            app.pending_discovery += 1;
                        }
                    }
                }
                ui.label(
                    RichText::new("ROXY speaks the OpenAI chat-completions protocol.")
                        .font(font(10.5))
                        .color(TEXT_FAINT),
                );
            });
        }
    });
}

fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn initials(s: &str) -> String {
    let mut out = String::new();
    for w in s.split_whitespace().take(3) {
        if let Some(c) = w.chars().next() {
            out.extend(c.to_uppercase());
        }
    }
    if out.is_empty() {
        s.chars().take(2).collect::<String>().to_uppercase()
    } else {
        out
    }
}

fn field_ui(ui: &mut Ui, value: &mut String, w: f32) -> egui::Response {
    egui::Frame::new()
        .fill(BG_DEEP)
        .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .frame(false)
                    .desired_width(w)
                    .font(mono(11.5))
                    .text_color(TEXT)
                    .margin(Margin::ZERO),
            )
        })
        .inner
}

fn password_ui(ui: &mut Ui, value: &mut String, w: f32) -> egui::Response {
    egui::Frame::new()
        .fill(BG_DEEP)
        .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .password(true)
                    .frame(false)
                    .desired_width(w)
                    .hint_text(RichText::new("sk-…").font(mono(11.0)).color(TEXT_FAINT))
                    .font(mono(11.5))
                    .text_color(TEXT)
                    .margin(Margin::ZERO),
            )
        })
        .inner
}

fn usage_card(app: &App, ui: &mut Ui) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            section_label(ui, "THIS RUN", TEXT_FAINT);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if small_button(ui, "Open data folder", Some(Icon::Arrow), TEXT_FAINT).clicked() {
                    cli::reveal_dir(&store::dir());
                }
            });
        });
        ui.add_space(10.0);
        let tin: u64 = app.sessions.iter().map(|s| s.tokens_in).sum();
        let tout: u64 = app.sessions.iter().map(|s| s.tokens_out).sum();
        ui.horizontal(|ui| {
            stat(ui, "PROMPT", &fmt_tokens(tin));
            ui.add_space(26.0);
            stat(ui, "COMPLETION", &fmt_tokens(tout));
            ui.add_space(26.0);
            stat(ui, "SESSIONS", &app.sessions.len().to_string());
            ui.add_space(26.0);
            stat(ui, "READY", &format!("{} routes", app.usable_count()));
        });
        ui.add_space(12.0);
        ui.label(
            RichText::new(store::dir().display().to_string())
                .font(mono(10.0))
                .color(TEXT_FAINT),
        );
    });
}

fn stat(ui: &mut Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        section_label(ui, label, TEXT_FAINT);
        ui.add_space(2.0);
        ui.label(RichText::new(value).font(mono(15.0)).color(TEXT));
    });
}

// ---------------------------------------------------------------- settings

fn settings(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    let mut changed = false;
    let mut refresh = false;

    page(ui, |ui| {
        header(
            ui,
            "Settings",
            "Every knob in ROXY, in one place. Saved the moment you change it.",
            |_ui| {},
        );

        let tabs = ["General", "Models", "CLI", "Appearance", "Data", "About"];
        let active = match app.settings_tab {
            SettingsTab::General => 0,
            SettingsTab::Models => 1,
            SettingsTab::Cli => 2,
            SettingsTab::Appearance => 3,
            SettingsTab::Data => 4,
            SettingsTab::About => 5,
        };
        if let Some(t) = segmented(ui, &tabs, active, accent) {
            app.settings_tab = match t {
                0 => SettingsTab::General,
                1 => SettingsTab::Models,
                2 => SettingsTab::Cli,
                3 => SettingsTab::Appearance,
                4 => SettingsTab::Data,
                _ => SettingsTab::About,
            };
        }
        ui.add_space(16.0);

        match app.settings_tab {
            SettingsTab::General => general(app, ui, &mut changed),
            SettingsTab::Models => models_settings(app, ui, &mut changed),
            SettingsTab::Cli => cli_settings(app, ui, &mut changed, &mut refresh),
            SettingsTab::Appearance => appearance(app, ui, &mut changed),
            SettingsTab::Data => data(app, ui, &mut changed),
            SettingsTab::About => about(app, ui, accent),
        }
    });

    if refresh {
        app.refresh_all(ui.ctx());
    }
    if changed {
        app.persist();
    }
}

fn general(app: &mut App, ui: &mut Ui, changed: &mut bool) {
    let accent = app.accent;
    card(ui, |ui| {
        section_label(ui, "BEHAVIOUR", TEXT_FAINT);
        ui.add_space(12.0);
        let mut auto_title = app.cfg.auto_title;
        setting_row(
            ui,
            "Title sessions automatically",
            "Use the first message as the session name.",
            |ui| {
                if switch(ui, &mut auto_title, accent) {
                    app.cfg.auto_title = auto_title;
                    *changed = true;
                }
            },
        );
        ui.add_space(12.0);
        let mut confirm = app.cfg.confirm_delete;
        setting_row(
            ui,
            "Confirm before deleting",
            "Ask first — sessions hold the only copy of their transcript.",
            |ui| {
                if switch(ui, &mut confirm, accent) {
                    app.cfg.confirm_delete = confirm;
                    *changed = true;
                }
            },
        );
        ui.add_space(12.0);
        let mut show = app.cfg.show_tokens;
        setting_row(
            ui,
            "Show token counts",
            "Estimate prompt and reply size next to each turn.",
            |ui| {
                if switch(ui, &mut show, accent) {
                    app.cfg.show_tokens = show;
                    *changed = true;
                }
            },
        );
        ui.add_space(12.0);
        let mut motion = app.cfg.motion;
        setting_row(
            ui,
            "Interface motion",
            "Splash animation and streaming caret.",
            |ui| {
                if switch(ui, &mut motion, accent) {
                    app.cfg.motion = motion;
                    *changed = true;
                }
            },
        );
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        section_label(ui, "KEYBOARD", TEXT_FAINT);
        ui.add_space(10.0);
        for (k, v) in [
            ("Ctrl + N", "New session"),
            ("Ctrl + K", "Search every model"),
            ("Ctrl + 2", "Models"),
            ("Ctrl + 3", "Add provider"),
            ("Ctrl + ,", "Settings"),
            ("Enter", "Send"),
            ("Shift + Enter", "New line"),
            ("Esc", "Stop, or close a panel"),
        ] {
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(v).font(font(12.5)).color(TEXT_DIM));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    keycap(ui, k);
                });
            });
            ui.add_space(5.0);
        }
    });
}

fn models_settings(app: &mut App, ui: &mut Ui, changed: &mut bool) {
    let accent = app.accent;
    card(ui, |ui| {
        section_label(ui, "DEFAULT ROUTE", TEXT_FAINT);
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Where a new session starts. You can switch mid-conversation at any time.",
            )
            .font(font(11.5))
            .color(TEXT_FAINT),
        );
        ui.add_space(10.0);
        let usable: Vec<(String, String, Vec<String>)> = app
            .cfg
            .providers
            .iter()
            .filter(|p| p.usable())
            .map(|p| (p.id.clone(), p.name.clone(), p.visible_models()))
            .collect();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                section_label(ui, "PROVIDER", TEXT_FAINT);
                let current = if app.cfg.default_provider.is_empty() {
                    "first available".to_string()
                } else {
                    usable
                        .iter()
                        .find(|(id, _, _)| id == &app.cfg.default_provider)
                        .map(|(_, n, _)| n.clone())
                        .unwrap_or_else(|| app.cfg.default_provider.clone())
                };
                egui::ComboBox::from_id_salt("default-provider")
                    .selected_text(RichText::new(current).font(font(12.0)).color(TEXT))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (id, name, _) in &usable {
                            if ui
                                .selectable_value(
                                    &mut app.cfg.default_provider,
                                    id.clone(),
                                    RichText::new(name).font(font(12.0)),
                                )
                                .clicked()
                            {
                                *changed = true;
                            }
                        }
                    });
            });
            ui.add_space(18.0);
            ui.vertical(|ui| {
                section_label(ui, "MODEL", TEXT_FAINT);
                let models = usable
                    .iter()
                    .find(|(id, _, _)| id == &app.cfg.default_provider)
                    .map(|(_, _, m)| m.clone())
                    .unwrap_or_default();
                let sel = if app.cfg.default_model.is_empty() {
                    "provider default".to_string()
                } else {
                    app.cfg.default_model.clone()
                };
                egui::ComboBox::from_id_salt("default-model")
                    .selected_text(RichText::new(sel).font(mono(11.0)).color(TEXT))
                    .width(230.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.cfg.default_model,
                            String::new(),
                            RichText::new("provider default").font(mono(11.0)),
                        );
                        for m in &models {
                            if ui
                                .selectable_value(
                                    &mut app.cfg.default_model,
                                    m.clone(),
                                    RichText::new(m).font(mono(11.0)),
                                )
                                .clicked()
                            {
                                *changed = true;
                            }
                        }
                    });
            });
        });
        if usable.is_empty() {
            ui.add_space(6.0);
            ui.label(
                RichText::new("No usable route yet — install a CLI or add a key first.")
                    .font(font(11.0))
                    .color(WARN),
            );
        }
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        section_label(ui, "SAMPLING", TEXT_FAINT);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                section_label(ui, "TEMPERATURE", TEXT_FAINT);
                if ui
                    .add(
                        egui::Slider::new(&mut app.cfg.temperature, 0.0..=2.0)
                            .fixed_decimals(2)
                            .show_value(true),
                    )
                    .changed()
                {
                    *changed = true;
                }
            });
            ui.add_space(24.0);
            ui.vertical(|ui| {
                section_label(ui, "MAX REPLY TOKENS", TEXT_FAINT);
                let mut v = app.cfg.max_tokens as f32;
                if ui
                    .add(
                        egui::Slider::new(&mut v, 0.0..=32768.0)
                            .step_by(256.0)
                            .show_value(true),
                    )
                    .changed()
                {
                    app.cfg.max_tokens = v as u32;
                    *changed = true;
                }
                ui.label(
                    RichText::new("0 = let the model decide")
                        .font(font(10.0))
                        .color(TEXT_FAINT),
                );
            });
        });
        ui.add_space(12.0);
        let mut hist = if app.cfg.history_limit == 0 {
            80.0
        } else {
            app.cfg.history_limit as f32
        };
        let label = if app.cfg.history_limit == 0 {
            "Replay the whole conversation".to_string()
        } else {
            format!("Replay the last {} messages", app.cfg.history_limit)
        };
        ui.label(RichText::new(label).font(font_med(12.5)).color(TEXT));
        ui.label(
            RichText::new(
                "Full context is what makes switching seamless. Trim only for very long chats.",
            )
            .font(font(11.0))
            .color(TEXT_FAINT),
        );
        ui.add_space(4.0);
        if ui
            .add(
                egui::Slider::new(&mut hist, 0.0..=80.0)
                    .step_by(2.0)
                    .custom_formatter(|v, _| {
                        if v >= 80.0 {
                            "all".to_string()
                        } else {
                            format!("{} msg", v as i64)
                        }
                    })
                    .show_value(true),
            )
            .changed()
        {
            app.cfg.history_limit = if hist >= 80.0 { 0 } else { hist as usize };
            *changed = true;
        }
        ui.add_space(10.0);
        let mut streaming = app.cfg.streaming;
        setting_row(
            ui,
            "Stream tokens",
            "Show the reply as it is written (HTTP providers).",
            |ui| {
                if switch(ui, &mut streaming, accent) {
                    app.cfg.streaming = streaming;
                    *changed = true;
                }
            },
        );
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        section_label(ui, "SYSTEM PROMPT", TEXT_FAINT);
        ui.add_space(8.0);
        let resp = egui::Frame::new()
            .fill(BG_DEEP)
            .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
            .corner_radius(CornerRadius::same(9))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut app.cfg.system_prompt)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .frame(false)
                        .font(font(12.5))
                        .text_color(TEXT_DIM)
                        .margin(Margin::ZERO),
                )
            })
            .inner;
        if resp.changed() {
            *changed = true;
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new("Prepended to every request, whichever model serves it.")
                .font(font(10.5))
                .color(TEXT_FAINT),
        );
    });
}

fn cli_settings(app: &mut App, ui: &mut Ui, changed: &mut bool, refresh: &mut bool) {
    let accent = app.accent;
    let full = app.cfg.cli_permissions == "full";

    card(ui, |ui| {
        section_label(ui, "PERMISSIONS", TEXT_FAINT);
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Agent CLIs can read files and run commands. Safe keeps each CLI's own approval prompts; Full hands them a bypass flag so long tasks never stop to ask.",
            )
            .font(font(11.5))
            .color(TEXT_DIM),
        );
        ui.add_space(10.0);
        let labels = ["Safe", "Full access"];
        if let Some(t) = segmented(ui, &labels, if full { 1 } else { 0 }, accent) {
            app.cfg.cli_permissions = if t == 1 { "full".into() } else { "safe".into() };
            *changed = true;
        }
        if full {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Full access only changes the flags for CLIs that support it. They still run in the folder below.")
                    .font(font(10.5))
                    .color(WARN),
            );
        }
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        section_label(ui, "WORKING FOLDER", TEXT_FAINT);
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Agents start here, so file reads and edits stay in one place. Leave blank to use ROXY's own workspace folder.",
            )
            .font(font(11.5))
            .color(TEXT_FAINT),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if field_ui(ui, &mut app.cfg.cli_dir, 380.0).changed() {
                *changed = true;
            }
            if small_button(ui, "Open folder", Some(Icon::Arrow), TEXT_FAINT).clicked() {
                let d = cli::working_dir(&app.cfg.cli_dir);
                cli::reveal_dir(&d);
            }
            if small_button(ui, "Use default", None, TEXT_FAINT).clicked() {
                app.cfg.cli_dir.clear();
                *changed = true;
            }
        });
        ui.add_space(10.0);
        let mut t = app.cfg.cli_timeout as f32;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Turn timeout")
                    .font(font_med(12.5))
                    .color(TEXT),
            );
            ui.add_space(12.0);
            if ui
                .add(
                    egui::Slider::new(&mut t, 60.0..=3600.0)
                        .step_by(60.0)
                        .suffix(" s")
                        .show_value(true),
                )
                .changed()
            {
                app.cfg.cli_timeout = t as u64;
                *changed = true;
            }
        });
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        ui.horizontal(|ui| {
            section_label(ui, "RUNTIME", TEXT_FAINT);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if small_button(ui, "Re-scan", Some(Icon::Refresh), TEXT_FAINT).clicked() {
                    *refresh = true;
                }
            });
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if app.node_ok {
                pill(
                    ui,
                    &format!("node {}", app.node_version),
                    Some(OK),
                    OK,
                    BG_DEEP,
                );
                ui.add_space(6.0);
                pill(
                    ui,
                    if cli::npm().is_some() {
                        "npm ready"
                    } else {
                        "npm missing"
                    },
                    Some(if cli::npm().is_some() { OK } else { WARN }),
                    TEXT_FAINT,
                    BG_DEEP,
                );
            } else {
                pill(ui, "node not found", Some(WARN), WARN, BG_DEEP);
                if small_button(ui, "Install Node.js", Some(Icon::Arrow), WARN).clicked() {
                    cli::open_url("https://nodejs.org/en/download");
                }
            }
        });
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!("Tools live in {}", cli::tools_dir().display()))
                .font(mono(10.0))
                .color(TEXT_FAINT),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Installs are per-user and isolated: no admin prompt, no global npm, no PATH changes.",
            )
            .font(font(10.5))
            .color(TEXT_FAINT),
        );
    });
}

fn appearance(app: &mut App, ui: &mut Ui, changed: &mut bool) {
    let accent = app.accent;
    card(ui, |ui| {
        section_label(ui, "ACCENT", TEXT_FAINT);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for name in ["lime", "violet", "cyan", "amber", "coral"] {
                let c = accent_color(name);
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(rect, CornerRadius::same(9), c);
                if app.cfg.accent == name {
                    p.rect_stroke(
                        rect.expand(3.0),
                        CornerRadius::same(11),
                        Stroke::new(1.5_f32, c),
                        egui::StrokeKind::Inside,
                    );
                }
                if resp.clicked() {
                    app.cfg.accent = name.to_string();
                    *changed = true;
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                ui.add_space(8.0);
            }
        });
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        section_label(ui, "SCALE & DENSITY", TEXT_FAINT);
        ui.add_space(10.0);
        let mut s = app.cfg.font_scale;
        ui.label(
            RichText::new("Interface scale")
                .font(font_med(12.5))
                .color(TEXT),
        );
        if ui
            .add(
                egui::Slider::new(&mut s, 0.85..=1.30)
                    .fixed_decimals(2)
                    .show_value(true),
            )
            .changed()
        {
            app.cfg.font_scale = s;
            *changed = true;
        }
        ui.add_space(8.0);
        let mut compact = app.cfg.compact;
        setting_row(
            ui,
            "Compact density",
            "Tighter rows for longer lists.",
            |ui| {
                if switch(ui, &mut compact, accent) {
                    app.cfg.compact = compact;
                    *changed = true;
                }
            },
        );
        ui.add_space(8.0);
        let mut w = app.cfg.chat_width;
        ui.label(
            RichText::new("Transcript width")
                .font(font_med(12.5))
                .color(TEXT),
        );
        if ui
            .add(
                egui::Slider::new(&mut w, 620.0..=1120.0)
                    .step_by(20.0)
                    .show_value(true),
            )
            .changed()
        {
            app.cfg.chat_width = w;
            *changed = true;
        }
    });
}

fn data(app: &mut App, ui: &mut Ui, changed: &mut bool) {
    card(ui, |ui| {
        section_label(ui, "STORAGE", TEXT_FAINT);
        ui.add_space(8.0);
        ui.label(
            RichText::new(store::dir().display().to_string())
                .font(mono(11.0))
                .color(TEXT_DIM),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Sessions, settings, CLI tools and logs. Nothing leaves this folder unless you send it to a model.",
            )
            .font(font(11.0))
            .color(TEXT_FAINT),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if small_button(ui, "Open folder", Some(Icon::Arrow), TEXT_DIM).clicked() {
                cli::reveal_dir(&store::dir());
            }
            if small_button(ui, "Save now", None, TEXT_DIM).clicked() {
                *changed = true;
            }
            if small_button(ui, "Export sessions", Some(Icon::Arrow), TEXT_DIM).clicked() {
                let path = store::dir().join("sessions-export.md");
                let mut out = String::from("# ROXY sessions\n\n");
                for s in &app.sessions {
                    out.push_str(&format!("## {}\n\n", s.title));
                    for m in &s.messages {
                        let who = match m.role {
                            Role::User => "You",
                            Role::Assistant => "Assistant",
                            Role::Handoff | Role::System => "—",
                        };
                        out.push_str(&format!("**{}**\n\n{}\n\n", who, m.text));
                    }
                }
                match std::fs::write(&path, out) {
                    Ok(_) => {
                        app.toast = Some((format!("Exported to {}", path.display()), 5.0));
                        cli::reveal_dir(&store::dir());
                    }
                    Err(e) => app.toast = Some((format!("Export failed: {e}"), 5.0)),
                }
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} sessions · {} messages",
                    app.sessions.len(),
                    app.sessions.iter().map(|s| s.messages.len()).sum::<usize>()
                ))
                .font(font(11.0))
                .color(TEXT_FAINT),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if small_button(ui, "Delete all sessions", Some(Icon::Trash), DANGER).clicked() {
                    app.sessions.clear();
                    let (pid, model) = ("", "");
                    let mut s = Session::new(pid, model);
                    s.title = "New session".into();
                    app.sessions.push(s);
                    app.active = 0;
                    app.toast = Some(("All sessions deleted".into(), 3.0));
                    *changed = true;
                }
            });
        });
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        section_label(ui, "PRIVACY", TEXT_FAINT);
        ui.add_space(8.0);
        for line in [
            "No telemetry, no analytics, no crash reporting.",
            "No account. Keys are stored only in this machine's config file.",
            "Requests go from this app straight to the provider you chose.",
            "Agent CLIs run as your own user, in the folder you set.",
        ] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter()
                    .circle_filled(r.center(), 2.0, OK.gamma_multiply(0.8));
                ui.label(RichText::new(line).font(font(11.5)).color(TEXT_DIM));
            });
            ui.add_space(3.0);
        }
    });

    ui.add_space(12.0);
    card(ui, |ui| {
        section_label(ui, "RESET", TEXT_FAINT);
        ui.add_space(8.0);
        ui.label(
            RichText::new("Clear every key, provider and preference. Sessions are kept.")
                .font(font(11.0))
                .color(TEXT_FAINT),
        );
        ui.add_space(8.0);
        if small_button(ui, "Reset settings", Some(Icon::Refresh), DANGER).clicked() {
            let sessions = app.sessions.clone();
            app.cfg = Config::default();
            app.sessions = sessions;
            app.heal_sessions();
            *changed = true;
            app.toast = Some(("Settings reset".into(), 3.0));
        }
    });
}

fn about(app: &mut App, ui: &mut Ui, accent: egui::Color32) {
    if let Some(rx) = &app.update_rx {
        if let Ok(result) = rx.try_recv() {
            app.update_rx = None;
            match result {
                Ok(release) => {
                    if release.version == env!("CARGO_PKG_VERSION") {
                        app.update_status = format!("You’re up to date ({})", release.version);
                    } else {
                        app.update_status = format!("Version {} is available", release.version);
                        app.pending_release = Some(release);
                    }
                }
                Err(error) => app.update_status = format!("Update check failed: {error}"),
            }
        }
    }
    card(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, CornerRadius::same(12), accent);
            let r = rect.shrink(11.0);
            p.line_segment(
                [
                    egui::pos2(r.left(), r.bottom()),
                    egui::pos2(r.right(), r.top()),
                ],
                Stroke::new(3.5_f32, BG_DEEP),
            );
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("ROXY {}", env!("CARGO_PKG_VERSION")))
                        .font(font_semi(17.0))
                        .color(TEXT),
                );
                ui.label(
                    RichText::new("One session. Every model.")
                        .font(font(12.0))
                        .color(TEXT_DIM),
                );
            });
        });
        ui.add_space(12.0);
        ui.label(
            RichText::new(
                "A single native window for every model you have access to — hosted APIs and local agent CLIs alike — with one continuous conversation that survives every switch. Built in Rust, no Electron, no telemetry, one binary.",
            )
            .font(font(12.5))
            .color(TEXT_DIM)
            .line_height(Some(19.0)),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if small_button(ui, "Check for updates", Some(Icon::Refresh), TEXT_DIM).clicked() {
                app.check_updates(ui.ctx());
            }
            if app.pending_release.is_some() {
                if small_button(ui, "Install update", Some(Icon::Arrow), accent).clicked() {
                    app.begin_update(ui.ctx());
                }
            }
            ui.label(
                RichText::new(&app.update_status)
                    .font(font(11.0))
                    .color(TEXT_FAINT),
            );
        });
        ui.add_space(10.0);
        hairline(ui, BORDER_SOFT);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            pill(ui, "rust", None, TEXT_FAINT, BG_DEEP);
            ui.add_space(4.0);
            pill(ui, "egui", None, TEXT_FAINT, BG_DEEP);
            ui.add_space(4.0);
            pill(ui, "no telemetry", None, TEXT_FAINT, BG_DEEP);
            ui.add_space(4.0);
            pill(ui, env!("CARGO_PKG_VERSION"), None, TEXT_FAINT, BG_DEEP);
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if small_button(
                ui,
                "Provider catalogue: models.dev",
                Some(Icon::Info),
                TEXT_DIM,
            )
            .clicked()
            {
                cli::open_url("https://models.dev");
            }
            if small_button(ui, "Data folder", Some(Icon::Arrow), TEXT_DIM).clicked() {
                cli::reveal_dir(&store::dir());
            }
        });
    });
}
