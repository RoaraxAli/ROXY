//! Layout: rail, session list, header, composer and the model switcher.

pub mod chat;
pub mod markdown;
pub mod panels;
pub mod splash;
pub mod widgets;

use crate::app::{App, View};
use crate::model::*;
use crate::theme::*;
use crate::ui::widgets::*;
use eframe::egui;
use egui::{
    Align, Align2, Color32, CornerRadius, Layout, Margin, RichText, Sense, Stroke, Ui, Vec2,
};

pub fn layout(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::left("rail")
        .exact_width(64.0)
        .resizable(false)
        .frame(egui::Frame::new().fill(BG_DEEP).inner_margin(Margin::ZERO))
        .show(ctx, |ui| rail(app, ui));

    if app.view == View::Chat {
        egui::SidePanel::left("sessions")
            .exact_width(274.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL))
            .show(ctx, |ui| sessions_panel(app, ui));

        egui::TopBottomPanel::top("header")
            .exact_height(62.0)
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin {
                left: 26,
                right: 22,
                top: 0,
                bottom: 0,
            }))
            .show(ctx, |ui| header(app, ui));

        egui::TopBottomPanel::bottom("composer")
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin {
                left: 26,
                right: 26,
                top: 6,
                bottom: 16,
            }))
            .show(ctx, |ui| composer(app, ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| chat::transcript(app, ui));

        if app.picker {
            picker(app, ctx);
        }
        rename_popup(app, ctx);
    } else {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| panels::view(app, ui));
        rename_popup(app, ctx);
    }
}

fn rename_popup(app: &mut App, ctx: &egui::Context) {
    let Some((idx, original)) = app.renaming.clone() else {
        return;
    };
    let mut buf = original;
    let mut save = false;
    let mut cancel = false;
    let accent = app.accent;

    egui::Area::new(egui::Id::new("rename_popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(ctx.screen_rect().center())
        .pivot(Align2::CENTER_CENTER)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_rgb(16, 16, 19))
                .stroke(Stroke::new(1.0_f32, BORDER))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::symmetric(16, 14))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 16],
                    blur: 40,
                    spread: 0,
                    color: Color32::from_black_alpha(180),
                })
                .show(ui, |ui| {
                    ui.set_width(340.0);
                    section_label(ui, "RENAME SESSION", TEXT_FAINT);
                    ui.add_space(9.0);
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .desired_width(f32::INFINITY)
                            .font(font(14.0))
                            .text_color(TEXT)
                            .margin(Margin::symmetric(8, 6)),
                    );
                    if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        save = true;
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if action_button(ui, "Save", accent, true, Some(84.0)).clicked() {
                            save = true;
                        }
                        if action_button(ui, "Cancel", accent, false, Some(84.0)).clicked() {
                            cancel = true;
                        }
                    });
                });
        });

    if save {
        let t = buf.trim().to_string();
        if let Some(s) = app.sessions.get_mut(idx) {
            if !t.is_empty() {
                s.title = t;
            }
        }
        app.persist();
    }
    if save || cancel {
        app.renaming = None;
    } else {
        app.renaming = Some((idx, buf));
    }
}

fn logo(ui: &mut Ui, size: f32, accent: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    draw_brand_mark(ui.painter(), rect, accent, BG_DEEP);
}

pub(crate) fn draw_brand_mark(p: &egui::Painter, rect: egui::Rect, _tile: Color32, _ink: Color32) {
    let ctx = p.ctx();
    let id = egui::Id::new("roxy-brand-mark");
    let texture = ctx
        .data_mut(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let (width, height, rgba) =
                crate::image::decode_png_rgba(include_bytes!("../../assets/roxy-brand.png"))
                    .expect("bundled ROXY mark is a valid PNG");
            let image =
                egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
            let texture = ctx.load_texture("roxy-brand-mark", image, egui::TextureOptions::LINEAR);
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    p.image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn rail(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    ui.set_width(64.0);
    ui.add_space(18.0);
    ui.vertical_centered(|ui| {
        logo(ui, 30.0, accent);
    });
    ui.add_space(22.0);

    let items = [
        (View::Chat, Icon::Chat, "Chat"),
        (View::Models, Icon::Models, "Models"),
        (View::Settings, Icon::Settings, "Settings"),
    ];

    for (view, icon, tip) in items {
        let active = app.view == view;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(64.0, 52.0), Sense::click());
        let p = ui.painter();
        let inner = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.center().y - 1.0),
            Vec2::new(42.0, 46.0),
        );
        if active {
            p.rect_filled(inner, CornerRadius::same(11), SURFACE);
            p.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 0.0, rect.center().y - 10.0),
                    Vec2::new(2.5, 20.0),
                ),
                CornerRadius::same(2),
                accent,
            );
        } else if resp.hovered() {
            p.rect_filled(inner, CornerRadius::same(11), Color32::from_white_alpha(8));
        }
        draw_icon(
            p,
            egui::pos2(rect.center().x, rect.center().y - 6.0),
            18.0,
            icon,
            if active { accent } else { TEXT_FAINT },
        );
        p.text(
            egui::pos2(rect.center().x, rect.center().y + 15.0),
            Align2::CENTER_CENTER,
            tip,
            mono(8.0),
            if active {
                TEXT_DIM
            } else {
                TEXT_FAINT.gamma_multiply(0.75)
            },
        );
        let resp = resp.on_hover_text(format!("{tip}  ({})", shortcut_for(view)));
        if resp.clicked() {
            app.view = view;
        }
    }

    ui.add_space(8.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(64.0, 46.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            egui::Rect::from_center_size(rect.center(), Vec2::splat(42.0)),
            CornerRadius::same(11),
            Color32::from_white_alpha(8),
        );
    }
    draw_icon(
        ui.painter(),
        egui::pos2(rect.center().x, rect.center().y - 4.0),
        18.0,
        Icon::Plus,
        TEXT_FAINT,
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.center().y + 15.0),
        Align2::CENTER_CENTER,
        "New",
        mono(8.0),
        TEXT_FAINT.gamma_multiply(0.75),
    );
    if resp.on_hover_text("New session  (Ctrl+N)").clicked() {
        app.new_chat();
    }

    // bottom status
    let h = ui.available_height();
    ui.add_space((h - 84.0).max(0.0));
    let ready = app.usable_count();
    let col = if ready > 0 { OK } else { WARN };
    ui.vertical_centered(|ui| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
        ui.painter().circle_filled(rect.center(), 3.5, col);
        let resp = resp.on_hover_text(format!("{ready} ready model sources"));
        if resp.clicked() {
            app.view = View::Models;
        }
    });
    ui.add_space(6.0);
}

fn sessions_panel(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        let job = micro_label("ROXY", TEXT, 15.0, 3.2);
        let galley = ui.fonts(|f| f.layout_job(job));
        let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
        ui.painter().galley(rect.min, galley, TEXT);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(14.0);
            ui.label(RichText::new("v0.1").font(mono(10.0)).color(TEXT_FAINT));
        });
    });
    ui.add_space(14.0);

    // New session
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        let w = ui.available_width() - 14.0;
        let btn = action_button(ui, "New session", accent, true, Some(w));
        if btn.clicked() {
            app.new_chat();
        }
    });

    ui.add_space(14.0);

    // Search
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        let w = ui.available_width() - 14.0;
        egui::Frame::new()
            .fill(BG_DEEP)
            .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
            .corner_radius(CornerRadius::same(9))
            .inner_margin(Margin::symmetric(9, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
                    draw_icon(ui.painter(), r.center(), 13.0, Icon::Search, TEXT_FAINT);
                    ui.add(
                        egui::TextEdit::singleline(&mut app.search)
                            .hint_text(
                                RichText::new("Search sessions")
                                    .font(font(13.0))
                                    .color(TEXT_FAINT),
                            )
                            .frame(false)
                            .desired_width(w - 52.0)
                            .font(font(13.0))
                            .text_color(TEXT),
                    );
                });
            });
    });

    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        section_label(ui, "SESSIONS", TEXT_FAINT);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("{}", app.sessions.len()))
                    .font(mono(10.0))
                    .color(TEXT_FAINT),
            );
        });
    });
    ui.add_space(6.0);

    let mut select: Option<usize> = None;
    let mut delete: Option<usize> = None;
    let mut rename: Option<usize> = None;
    let q = app.search.to_lowercase();

    let list_h = (ui.available_height() - 86.0).max(90.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(list_h)
        .id_salt("session_list")
        .show(ui, |ui| {
            let mut shown = 0usize;
            for i in 0..app.sessions.len() {
                let s = &app.sessions[i];
                if !q.is_empty()
                    && !s.title.to_lowercase().contains(&q)
                    && !s.preview().to_lowercase().contains(&q)
                {
                    continue;
                }
                shown += 1;
                let is_active = i == app.active;
                let provider = app
                    .cfg
                    .providers
                    .iter()
                    .find(|p| p.id == s.provider)
                    .cloned();
                let title = s.title.clone();
                let when = time_ago(s.updated);
                let msgs = s.messages.iter().filter(|m| m.role == Role::User).count();
                let short = provider
                    .as_ref()
                    .map(|p| {
                        if p.short.is_empty() {
                            p.name.clone()
                        } else {
                            p.short.clone()
                        }
                    })
                    .unwrap_or_else(|| "—".into());

                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let w = ui.available_width() - 8.0;
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 58.0), Sense::click());
                    let p = ui.painter();
                    if is_active {
                        p.rect_filled(rect, CornerRadius::same(9), SURFACE);
                        p.rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(rect.left(), rect.center().y - 11.0),
                                Vec2::new(2.0, 22.0),
                            ),
                            CornerRadius::same(2),
                            accent,
                        );
                    } else if resp.hovered() {
                        p.rect_filled(rect, CornerRadius::same(9), Color32::from_white_alpha(5));
                    }

                    let title_col = if is_active { TEXT } else { TEXT_DIM };
                    let shown: String = if title.chars().count() > 30 {
                        format!("{}…", title.chars().take(29).collect::<String>())
                    } else {
                        title.clone()
                    };
                    let galley = p.layout(shown, font_med(13.5), title_col, w - 22.0);
                    p.galley(
                        egui::pos2(rect.left() + 12.0, rect.top() + 10.0),
                        galley,
                        title_col,
                    );

                    let sub = format!(
                        "{short} · {when} · {msgs} {}",
                        if msgs == 1 { "msg" } else { "msgs" }
                    );
                    p.text(
                        egui::pos2(rect.left() + 12.0, rect.bottom() - 17.0),
                        Align2::LEFT_TOP,
                        sub,
                        mono(10.0),
                        TEXT_FAINT,
                    );

                    if resp.hovered() && !is_active {
                        let dr = egui::Rect::from_center_size(
                            egui::pos2(rect.right() - 16.0, rect.center().y),
                            Vec2::splat(22.0),
                        );
                        let dresp = ui.interact(dr, egui::Id::new(("del", i)), Sense::click());
                        draw_icon(
                            ui.painter(),
                            dr.center(),
                            13.0,
                            Icon::Trash,
                            if dresp.hovered() { DANGER } else { TEXT_FAINT },
                        );
                        if dresp.clicked() {
                            delete = Some(i);
                        }
                    }

                    if resp.clicked() {
                        select = Some(i);
                    }
                    if resp.double_clicked() {
                        rename = Some(i);
                    }
                });
            }

            if shown == 0 {
                ui.add_space(18.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("No sessions match")
                            .font(font(12.5))
                            .color(TEXT_FAINT),
                    );
                });
            }
        });

    if let Some(i) = select {
        app.active = i;
        app.view = View::Chat;
    }
    if let Some(i) = delete {
        app.delete_session(i);
    }
    if let Some(i) = rename {
        app.renaming = Some((i, app.sessions[i].title.clone()));
    }

    // footer stats
    let s = app.active_session().cloned();
    let h = ui.available_height();
    if h > 70.0 {
        ui.add_space(h - 76.0);
    }
    hairline(ui, BORDER_SOFT);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.add_space(10.0);
            section_label(ui, "THIS SESSION", TEXT_FAINT);
            ui.add_space(3.0);
            let toks = s.as_ref().map(|x| x.total_tokens()).unwrap_or(0);
            ui.label(RichText::new(fmt_tokens(toks)).font(mono(15.0)).color(TEXT));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(16.0);
            let ready = app.usable_count();
            ui.label(
                RichText::new(format!("{ready} ready"))
                    .font(mono(10.0))
                    .color(if ready > 0 { OK } else { TEXT_FAINT }),
            );
        });
    });
    ui.add_space(12.0);
}

fn header(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    ui.horizontal_centered(|ui| {
        ui.vertical(|ui| {
            ui.add_space(1.0);
            let title = app
                .active_session()
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "New session".into());
            let title = if title.chars().count() > 64 {
                format!("{}…", title.chars().take(63).collect::<String>())
            } else {
                title
            };
            ui.label(RichText::new(title).font(font_semi(16.5)).color(TEXT));

            let s = app.active_session();
            let ctx_label = match (s, app.view) {
                (_, View::Chat) => {
                    let msgs = s
                        .map(|x| x.messages.iter().filter(|m| m.role == Role::User).count())
                        .unwrap_or(0);
                    format!(
                        "{msgs} {} · context preserved across models",
                        if msgs == 1 { "turn" } else { "turns" }
                    )
                }
                (_, View::Models) => "CLI agents and API models".to_string(),
                (_, View::Settings) => "Every option, in one place".to_string(),
            };
            ui.add_space(1.0);
            let job = micro_label(&ctx_label.to_uppercase(), TEXT_FAINT, 9.5, 1.2);
            ui.label(job);
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if app.view == View::Chat {
                let toks = app.active_session().map(|x| x.total_tokens()).unwrap_or(0);
                egui::Frame::new()
                    .fill(SURFACE)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(9, 5))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} tok", fmt_tokens(toks)))
                                .font(mono(10.5))
                                .color(TEXT_DIM),
                        );
                    });
                ui.add_space(4.0);
                if let Some(p) = app.provider_of_active() {
                    let dot_color = match p.status {
                        Some(true) => OK,
                        Some(false) => DANGER,
                        None => WARN,
                    };
                    let name = p.name.to_uppercase();
                    let resp = chip_dot(ui, &name, TEXT, SURFACE, BORDER, Some(dot_color))
                        .on_hover_text("Switch model");
                    if resp.clicked() {
                        app.picker = !app.picker;
                        app.picker_query.clear();
                        app.picker_anchor = Some(resp.rect);
                    }
                } else if chip_dot(ui, "CONNECT A MODEL", WARN, SURFACE, BORDER, Some(WARN))
                    .clicked()
                {
                    app.view = View::Models;
                }
            } else {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(1.0), Sense::hover());
                let _ = r;
            }
            let _ = accent;
        });
    });
}

fn composer(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    let streaming = app.streaming();
    let full = ui.available_width();
    let w = full.min(app.cfg.chat_width.clamp(560.0, 1120.0));
    let mut send_now = false;

    ui.horizontal(|ui| {
        ui.add_space(((full - w) * 0.5).max(0.0));
        ui.vertical(|ui| {
            ui.set_width(w);
            let focused = app.draft.is_empty() && ctx_focused(ui);
            let border = if focused { BORDER } else { BORDER_SOFT };
            egui::Frame::new()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0_f32, border))
                .corner_radius(CornerRadius::same(15))
                .inner_margin(Margin {
                    left: 14,
                    right: 12,
                    top: 12,
                    bottom: 10,
                })
                .show(ui, |ui| {
                    ui.set_width(w - 26.0);
                    let te = egui::TextEdit::multiline(&mut app.draft)
                        .id(egui::Id::new("composer_input"))
                        .frame(false)
                        .desired_rows(1)
                        .desired_width(f32::INFINITY)
                        .font(font(15.0))
                        .text_color(TEXT)
                        .hint_text(
                            RichText::new("Ask anything — every model shares this session")
                                .font(font(15.0))
                                .color(TEXT_FAINT),
                        )
                        .margin(Margin::ZERO);
                    ui.add(te);

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        // model chip
                        let has = app.provider_of_active().is_some();
                        let name = if has {
                            format!("{} · {}", app.active_name(), app.active_model())
                        } else {
                            "Connect a model".to_string()
                        };
                        let dotc = app
                            .provider_of_active()
                            .and_then(|p| p.status)
                            .map(|ok| if ok { OK } else { DANGER })
                            .unwrap_or(if has { WARN } else { DANGER });
                        let resp = chip_dot(
                            ui,
                            &name,
                            if has { TEXT } else { WARN },
                            BG_DEEP,
                            if has {
                                BORDER
                            } else {
                                WARN.gamma_multiply(0.4)
                            },
                            Some(dotc),
                        );
                        if resp.clicked() {
                            app.picker = !app.picker;
                            app.picker_query.clear();
                            app.picker_anchor = Some(resp.rect);
                        }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let size = 30.0;
                            let (rect, bresp) =
                                ui.allocate_exact_size(Vec2::splat(size), Sense::click());
                            let can = !app.draft.trim().is_empty();
                            let fill = if streaming {
                                Color32::from_rgb(58, 30, 30)
                            } else if can {
                                accent
                            } else {
                                SURFACE_HI
                            };
                            ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
                            let icon_col = if streaming {
                                DANGER
                            } else if can {
                                BG_DEEP
                            } else {
                                TEXT_FAINT
                            };
                            draw_icon(
                                ui.painter(),
                                rect.center(),
                                14.0,
                                if streaming { Icon::Stop } else { Icon::Send },
                                icon_col,
                            );
                            if bresp.clicked() {
                                if streaming {
                                    app.stop();
                                } else {
                                    send_now = true;
                                }
                            }
                            if bresp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(if streaming {
                                    "esc to stop"
                                } else {
                                    "Enter sends · Shift+Enter newline"
                                })
                                .font(mono(9.5))
                                .color(TEXT_FAINT),
                            );
                        });
                    });
                });
        });
    });

    // Enter to send
    if app.cfg.send_on_enter {
        let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        if enter {
            if let Some(focused) = ui.memory(|m| m.focused()) {
                if focused == egui::Id::new("composer_input") {
                    send_now = true;
                }
            }
        }
    }

    if send_now && !streaming {
        app.send(ui.ctx());
    }
}

fn model_row(ui: &mut Ui, label: &str, selected: bool, accent: Color32) -> egui::Response {
    let w = (ui.available_width() - 8.0).max(60.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 27.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(7), SURFACE);
        p.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 1.0, rect.center().y - 7.0),
                Vec2::new(2.0, 14.0),
            ),
            CornerRadius::same(1),
            accent,
        );
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), Color32::from_white_alpha(6));
    }
    p.text(
        egui::pos2(rect.left() + 11.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        mono(11.5),
        if selected { TEXT } else { TEXT_DIM },
    );
    resp
}

fn shortcut_for(view: View) -> &'static str {
    match view {
        View::Chat => "Ctrl+1",
        View::Models => "Ctrl+2",
        View::Settings => "Ctrl+,",
    }
}

fn ctx_focused(ui: &Ui) -> bool {
    ui.memory(|m| m.focused()) == Some(egui::Id::new("composer_input"))
}

fn picker(app: &mut App, ctx: &egui::Context) {
    let anchor = app
        .picker_anchor
        .unwrap_or_else(|| egui::Rect::from_center_size(ctx.screen_rect().center(), Vec2::ZERO));
    let accent = app.accent;
    let mut close = false;
    let mut choose: Option<(String, String)> = None;
    let mut query = app.picker_query.clone();

    // Filter first so the popup's geometry comes from the row count instead of
    // its own measured height. A measured height inside a bottom-pivoted Area
    // feeds back into the next frame's position: the list drifts upward and
    // clicks land on the wrong row.
    let q = query.trim().to_lowercase();
    let providers: Vec<Provider> = app
        .cfg
        .providers
        .iter()
        .filter(|p| p.usable())
        .filter(|p| {
            q.is_empty()
                || p.name.to_lowercase().contains(&q)
                || p.visible_models()
                    .iter()
                    .any(|m| m.to_lowercase().contains(&q))
        })
        .cloned()
        .collect();
    let session = app.active_session().cloned();

    let rows_for = |p: &Provider| -> Vec<String> {
        let source = if p.is_router() {
            p.ordered_models()
        } else {
            p.visible_models()
        };
        source
            .into_iter()
            .filter(|m| {
                !m.trim().is_empty()
                    && !m.eq_ignore_ascii_case("auto")
                    && (q.is_empty()
                        || m.to_lowercase().contains(&q)
                        || p.name.to_lowercase().contains(&q))
            })
            .collect()
    };

    let mut rows_h = 0.0f32;
    for p in &providers {
        rows_h += 22.0 + 7.0;
        if !p.note.is_empty() {
            rows_h += 13.0 + 7.0;
        }
        rows_h += rows_for(p).len() as f32 * 34.0 + 3.0;
    }
    if providers.is_empty() {
        rows_h = 40.0;
    }

    let screen = ctx.screen_rect();
    let chrome = 116.0;
    let max_list = (screen.height() * 0.58).max(160.0);
    let total_h = (rows_h + 6.0).min(max_list) + chrome;
    let list_h = total_h - chrome;
    let top = (anchor.top() - 10.0 - total_h).max(screen.top() + 10.0);

    egui::Area::new(egui::Id::new("model_picker"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(anchor.left(), top))
        .pivot(Align2::LEFT_TOP)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_rgb(16, 16, 19))
                .stroke(Stroke::new(1.0_f32, BORDER))
                .corner_radius(CornerRadius::same(13))
                .inner_margin(Margin::symmetric(8, 8))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 12],
                    blur: 30,
                    spread: 0,
                    color: Color32::from_black_alpha(160),
                })
                .show(ui, |ui| {
                    ui.set_width(380.0);
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        section_label(ui, "SWITCH MODEL", TEXT_FAINT);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_btn(ui, Icon::Close, 22.0, TEXT_FAINT).clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        egui::Frame::new()
                            .fill(BG_DEEP)
                            .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(8, 5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(Vec2::splat(11.0), Sense::hover());
                                    draw_icon(
                                        ui.painter(),
                                        r.center(),
                                        11.0,
                                        Icon::Search,
                                        TEXT_FAINT,
                                    );
                                    ui.add(
                                        egui::TextEdit::singleline(&mut query)
                                            .id(egui::Id::new("picker_search"))
                                            .hint_text(
                                                RichText::new("Search models and agents")
                                                    .font(font(12.0))
                                                    .color(TEXT_FAINT),
                                            )
                                            .frame(false)
                                            .desired_width(300.0)
                                            .font(font(12.0))
                                            .text_color(TEXT),
                                    );
                                });
                            });
                    });
                    ui.add_space(7.0);

                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), list_h.max(40.0)),
                        Layout::top_down(Align::Min),
                        |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if providers.is_empty() {
                                        ui.add_space(10.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(10.0);
                                            ui.label(
                                                RichText::new(
                                                    "Nothing to route to yet. Install a CLI or add a key.",
                                                )
                                                .font(font(11.5))
                                                .color(TEXT_FAINT),
                                            );
                                        });
                                        return;
                                    }
                                    for p in &providers {
                                        let p_accent = p.accent();
                                        ui.horizontal(|ui| {
                                            ui.add_space(8.0);
                                            let (r, _) = ui.allocate_exact_size(
                                                Vec2::splat(7.0),
                                                Sense::hover(),
                                            );
                                            ui.painter().circle_filled(r.center(), 3.5, p_accent);
                                            ui.label(
                                                RichText::new(p.name.to_uppercase())
                                                    .font(font_semi(10.0))
                                                    .color(TEXT_DIM),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.add_space(8.0);
                                                    let st = match p.status {
                                                        Some(true) => {
                                                            format!("{} ms", p.latency.unwrap_or(0))
                                                        }
                                                        Some(false) => "offline".to_string(),
                                                        None => "…".to_string(),
                                                    };
                                                    ui.label(
                                                        RichText::new(st)
                                                            .font(mono(9.5))
                                                            .color(if p.status == Some(false) {
                                                                DANGER
                                                            } else {
                                                                TEXT_FAINT
                                                            }),
                                                    );
                                                },
                                            );
                                        });
                                        if !p.note.is_empty() {
                                            ui.horizontal(|ui| {
                                                ui.add_space(17.0);
                                                let n: String = p.note.chars().take(64).collect();
                                                ui.label(
                                                    RichText::new(n)
                                                        .font(mono(9.0))
                                                        .color(WARN.gamma_multiply(0.8)),
                                                );
                                            });
                                        }
                                        for m in rows_for(p) {
                                            let selected = session
                                                .as_ref()
                                                .map(|s| s.provider == p.id && s.model == m)
                                                .unwrap_or(false);
                                            ui.horizontal(|ui| {
                                                ui.add_space(8.0);
                                                let resp = model_row(ui, &m, selected, accent);
                                                if resp.clicked() {
                                                    choose = Some((p.id.clone(), m.clone()));
                                                }
                                            });
                                        }
                                        ui.add_space(3.0);
                                    }
                                });
                        },
                    );

                    ui.add_space(4.0);
                    hairline(ui, BORDER_SOFT);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Full history is replayed to whichever model you pick.")
                                .font(font(11.0))
                                .color(TEXT_FAINT),
                        );
                    });
                });
        });

    if !app.picker_focus {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("picker_search")));
        app.picker_focus = true;
    }
    app.picker_query = query.clone();
    if let Some((pid, model)) = choose {
        app.set_route(&pid, &model);
    }
    if close {
        app.picker = false;
        app.picker_focus = false;
    }
    if ctx.input(|i| i.pointer.primary_clicked()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            let inside = ctx
                .memory(|m| m.area_rect(egui::Id::new("model_picker")))
                .map(|r| r.contains(pos))
                .unwrap_or(false);
            let on_anchor = app
                .picker_anchor
                .map(|r| r.expand(6.0).contains(pos))
                .unwrap_or(false);
            if !inside && !on_anchor {
                app.picker = false;
                app.picker_focus = false;
            }
        }
    }
}
