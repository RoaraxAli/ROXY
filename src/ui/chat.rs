//! Conversation transcript.

use crate::app::{App, View};
use crate::model::{Message, Provider, Role};
use crate::theme::*;
use crate::ui::markdown;
use crate::ui::widgets::*;
use egui::{Align, Color32, CornerRadius, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, Vec2};

pub fn transcript(app: &mut App, ui: &mut Ui) {
    let accent = app.accent;
    let session_idx = app.active;
    let streaming = app
        .call
        .as_ref()
        .map(|c| {
            app.sessions
                .get(session_idx)
                .map(|s| s.id == c.session_id)
                .unwrap_or(false)
        })
        .unwrap_or(false);
    let elapsed = app
        .call
        .as_ref()
        .map(|c| c.started.elapsed().as_secs_f32())
        .unwrap_or(0.0);
    let mut suggestion: Option<String> = None;
    let mut go: Option<View> = None;

    let msg_count = app
        .sessions
        .get(session_idx)
        .map(|s| s.messages.len())
        .unwrap_or(0);

    egui::ScrollArea::vertical()
        .id_salt(("transcript", session_idx))
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            let full = ui.available_width();
            // Keep the transcript inside the central panel even when the
            // window is narrow (the session rail and list still take space).
            let w = (full - 32.0).clamp(1.0, 800.0);
            ui.horizontal(|ui| {
                ui.add_space(((full - w) * 0.5).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(w);
                    if msg_count == 0 {
                        empty_state(app, ui, &mut suggestion, &mut go);
                    } else {
                        let msgs: &[Message] = &app.sessions[session_idx].messages;
                        let providers: &[Provider] = &app.cfg.providers;
                        ui.add_space(20.0);
                        for (i, m) in msgs.iter().enumerate() {
                            let is_last = i + 1 == msgs.len();
                            draw_message(ui, m, providers, accent, streaming && is_last, elapsed);
                        }
                    }
                    ui.add_space(10.0);
                });
            });
        });

    if let Some(text) = suggestion {
        app.draft = text;
    }
    if let Some(v) = go {
        app.view = v;
    }
}

fn draw_message(
    ui: &mut Ui,
    m: &Message,
    providers: &[Provider],
    accent: Color32,
    streaming: bool,
    elapsed: f32,
) {
    match m.role {
        Role::User => {
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                egui::Frame::new()
                    .fill(SURFACE)
                    .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(540.0);
                        ui.label(
                            RichText::new(&m.text)
                                .font(font(14.5))
                                .color(TEXT)
                                .line_height(Some(21.0)),
                        );
                    });
            });
            ui.add_space(12.0);
        }
        Role::Assistant => {
            let provider = providers.iter().find(|p| p.id == m.provider);
            let color = provider.map(|p| p.accent()).unwrap_or(TEXT_FAINT);
            let short = provider
                .map(|p| {
                    if p.short.is_empty() {
                        p.name.to_uppercase()
                    } else {
                        p.short.to_uppercase()
                    }
                })
                .unwrap_or_else(|| "MODEL".into());

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, color);
                ui.label(
                    RichText::new(short)
                        .font(font_semi(9.5))
                        .color(TEXT_DIM)
                        .extra_letter_spacing(1.4),
                );
                ui.label(
                    RichText::new(if m.model.is_empty() { "—" } else { &m.model })
                        .font(mono(10.0))
                        .color(TEXT_FAINT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if m.tokens > 0 {
                        ui.label(
                            RichText::new(format!("{} tok", fmt_tokens(m.tokens)))
                                .font(mono(9.5))
                                .color(TEXT_FAINT),
                        );
                    }
                    if !streaming && !m.text.is_empty() {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                        let hov = resp.hovered();
                        draw_icon(
                            ui.painter(),
                            r.center(),
                            11.0,
                            Icon::Plus,
                            if hov { TEXT_DIM } else { Color32::TRANSPARENT },
                        );
                        if resp.clicked() {
                            ui.ctx().copy_text(m.text.clone());
                        }
                    }
                });
            });
            ui.add_space(5.0);

            if streaming && m.text.is_empty() {
                thinking(ui, color, elapsed);
            } else {
                let mut text = m.text.clone();
                if streaming {
                    text.push_str(" ▌");
                }
                let error = m.error;
                if error {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(26, 16, 18))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(70, 34, 38)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(13, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            markdown::render(ui, &text);
                        });
                } else {
                    markdown::render(ui, &text);
                }
            }
            let _ = accent;
            ui.add_space(16.0);
        }
        Role::Handoff | Role::System => {
            handoff(ui, &m.text);
            ui.add_space(12.0);
        }
    }
}

fn thinking(ui: &mut Ui, color: Color32, elapsed: f32) {
    let t = ui.input(|i| i.time) as f32;
    ui.horizontal(|ui| {
        for i in 0..3 {
            let phase = (t * 2.2 - i as f32 * 0.22).sin() * 0.5 + 0.5;
            let (r, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
            ui.painter()
                .circle_filled(r.center(), 3.5, color.gamma_multiply(0.25 + phase * 0.75));
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("working · {:.1}s", elapsed))
                .font(mono(10.0))
                .color(TEXT_FAINT),
        );
    });
}

fn handoff(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    let p = ui.painter();
    let galley = p.layout_no_wrap(text.to_uppercase(), font_med(9.0), TEXT_FAINT);
    let w = galley.size().x;
    let y = rect.center().y;
    let mid = rect.center().x;
    let stroke = Stroke::new(1.0_f32, BORDER_SOFT);
    p.line_segment(
        [
            egui::pos2(rect.left() + 10.0, y),
            egui::pos2((mid - w * 0.5 - 12.0).max(rect.left() + 10.0), y),
        ],
        stroke,
    );
    p.line_segment(
        [
            egui::pos2((mid + w * 0.5 + 12.0).min(rect.right() - 10.0), y),
            egui::pos2(rect.right() - 10.0, y),
        ],
        stroke,
    );
    p.galley(
        egui::pos2(mid - w * 0.5, y - galley.size().y * 0.5),
        galley,
        TEXT_FAINT,
    );
}

fn empty_state(app: &App, ui: &mut Ui, pick: &mut Option<String>, go: &mut Option<View>) {
    let accent = app.accent;
    let fresh = app.usable_count() == 0;
    ui.add_space(if fresh { 44.0 } else { 64.0 });

    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
        ui.painter()
            .circle_filled(r.center(), 3.5, if fresh { WARN } else { accent });
        ui.label(
            RichText::new(if fresh {
                "NOTHING CONNECTED YET"
            } else {
                "SESSION READY"
            })
            .font(font_semi(9.5))
            .color(TEXT_DIM)
            .extra_letter_spacing(1.8),
        );
    });

    ui.add_space(16.0);
    ui.label(
        RichText::new("One session.\nEvery model.")
            .font(font_semi(32.0))
            .color(TEXT)
            .line_height(Some(38.0)),
    );
    ui.add_space(12.0);
    ui.label(
        RichText::new(if fresh {
            "ROXY runs entirely on this machine. Install an agent CLI or bring an API key\nof your own — then every one of them shares this conversation."
        } else {
            "Start with any model. When a limit hits — or another brain fits better —\nswitch mid-conversation and keep every token of context."
        })
        .font(font(14.0))
        .color(TEXT_DIM)
        .line_height(Some(22.0)),
    );

    ui.add_space(if fresh { 22.0 } else { 30.0 });

    if fresh {
        onboarding(ui, accent, go);
        ui.add_space(22.0);
    }

    let cards = [
        (
            "COMPARE",
            "Ask once, answer twice",
            "Put the same prompt to Gemini and Codex, then keep the stronger answer.",
        ),
        (
            "HANDOFF",
            "Survive any limit",
            "Start on Claude, hit the cap, continue on OpenCode with nothing lost.",
        ),
        (
            "BUILD",
            "Long-running work",
            "Plan on one model, implement on another — the thread stays intact.",
        ),
    ];

    let full = ui.available_width();
    let cw = ((full - 24.0) / 3.0).max(150.0);
    ui.horizontal(|ui| {
        for (tag, title, body) in cards {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(cw - 8.0, 104.0), Sense::click());
            let p = ui.painter();
            p.rect(
                rect,
                CornerRadius::same(12),
                if resp.hovered() { SURFACE } else { PANEL },
                Stroke::new(1.0_f32, if resp.hovered() { BORDER } else { BORDER_SOFT }),
                egui::StrokeKind::Inside,
            );
            p.text(
                egui::pos2(rect.left() + 13.0, rect.top() + 12.0),
                egui::Align2::LEFT_TOP,
                tag,
                font_semi(8.5),
                accent.gamma_multiply(0.8),
            );
            p.text(
                egui::pos2(rect.left() + 13.0, rect.top() + 29.0),
                egui::Align2::LEFT_TOP,
                title,
                font_med(12.5),
                TEXT,
            );
            let g = p.layout(body.to_string(), font(11.0), TEXT_FAINT, cw - 34.0);
            p.galley(
                egui::pos2(rect.left() + 13.0, rect.top() + 50.0),
                g,
                TEXT_FAINT,
            );
            if resp.clicked() {
                *pick = Some(format!("{title}: {body}"));
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }
    });

    ui.add_space(30.0);

    // route strip
    ui.horizontal(|ui| {
        for (i, p) in app.cfg.providers.iter().filter(|p| p.usable()).enumerate() {
            if i > 0 {
                ui.add_space(14.0);
            }
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
            let col = match p.status {
                Some(true) => OK,
                Some(false) => DANGER,
                None => WARN,
            };
            ui.painter().circle_filled(r.center(), 3.0, col);
            ui.label(RichText::new(&p.short).font(mono(9.5)).color(TEXT_FAINT));
            let detail = if p.is_cli() {
                format!("{} · local CLI", p.name)
            } else {
                format!("{} · {}", p.name, p.host())
            };
            resp.on_hover_text(detail);
        }
    });
    let _ = Rect::ZERO;
}
/// First-run panel: two concrete ways to get a model, no marketing.
fn onboarding(ui: &mut Ui, accent: Color32, go: &mut Option<View>) {
    let full = ui.available_width();
    let cw = ((full - 12.0) / 2.0).max(180.0);
    ui.horizontal(|ui| {
        for (tag, title, body, label, target) in [
            (
                "01",
                "Install an agent CLI",
                "Codex, Claude Code, Gemini, OpenCode and five more. ROXY installs them into its own folder — no admin rights.",
                "Browse CLI agents",
                View::Models,
            ),
            (
                "02",
                "Add an API key",
                "OpenAI, Anthropic, Google, OpenRouter, Groq, DeepSeek, xAI, a local server — 200+ endpoints, your keys.",
                "Open models",
                View::Models,
            ),
        ] {
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(cw - 6.0, 148.0), Sense::click());
            let p = ui.painter();
            let hov = resp.hovered();
            p.rect(
                rect,
                CornerRadius::same(13),
                if hov { SURFACE } else { PANEL },
                Stroke::new(1.0_f32, if hov { BORDER } else { BORDER_SOFT }),
                egui::StrokeKind::Inside,
            );
            p.text(
                egui::pos2(rect.left() + 15.0, rect.top() + 14.0),
                egui::Align2::LEFT_TOP,
                tag,
                font_semi(9.0),
                accent.gamma_multiply(0.8),
            );
            p.text(
                egui::pos2(rect.left() + 15.0, rect.top() + 31.0),
                egui::Align2::LEFT_TOP,
                title,
                font_med(14.0),
                TEXT,
            );
            let g = p.layout(body.to_string(), font(11.5), TEXT_FAINT, cw - 40.0);
            p.galley(egui::pos2(rect.left() + 15.0, rect.top() + 54.0), g, TEXT_FAINT);
            p.text(
                egui::pos2(rect.left() + 15.0, rect.bottom() - 22.0),
                egui::Align2::LEFT_TOP,
                label,
                font_med(11.5),
                accent,
            );
            if hov {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                *go = Some(target);
            }
        }
    });
}
