//! Startup loader.

use crate::app::App;
use crate::theme::*;
use egui::{Align, Align2, Color32, CornerRadius, Layout, Margin, Sense, Ui, Vec2};

const STAGES: [(&str, &str); 4] = [
    ("INITIALISING RUNTIME", "warm gpu context · fonts · theme"),
    ("RESTORING SESSIONS", "shared context across every model"),
    ("PROBING GATEWAYS", "opencode · 9router · codex"),
    ("READY", "press any key"),
];

pub fn show(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(BG_DEEP))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let fade = (app.boot_t / 0.35).clamp(0.0, 1.0);
            let accent = accent_color(&app.cfg.accent);
            let p = ui.painter();

            let center = egui::pos2(rect.center().x, rect.center().y - 26.0);

            // Brand mark shared with the app rail.
            let mark = egui::Rect::from_center_size(
                egui::pos2(center.x, center.y - 96.0),
                Vec2::splat(58.0),
            );
            super::draw_brand_mark(p, mark, with_alpha(accent, fade), with_alpha(BG_DEEP, fade));

            // wordmark
            let job = {
                let mut j = egui::text::LayoutJob::default();
                j.append(
                    "ROXY",
                    0.0,
                    egui::TextFormat {
                        font_id: font_semi(40.0),
                        color: with_alpha(TEXT, fade),
                        extra_letter_spacing: 13.0,
                        ..Default::default()
                    },
                );
                j
            };
            let galley = ui.fonts(|f| f.layout_job(job));
            let pos = egui::pos2(center.x - galley.size().x * 0.5 + 6.0, center.y - 52.0);
            ui.painter().galley(pos, galley, with_alpha(TEXT, fade));

            // tagline
            let mut tag = egui::text::LayoutJob::default();
            tag.append(
                "ONE SESSION  ·  EVERY MODEL",
                0.0,
                egui::TextFormat {
                    font_id: font_med(10.0),
                    color: with_alpha(TEXT_FAINT, fade),
                    extra_letter_spacing: 3.0,
                    ..Default::default()
                },
            );
            let tg = ui.fonts(|f| f.layout_job(tag));
            ui.painter().galley(
                egui::pos2(center.x - tg.size().x * 0.5 + 1.5, center.y + 2.0),
                tg,
                with_alpha(TEXT_FAINT, fade),
            );

            // built-on line
            let mut built = egui::text::LayoutJob::default();
            built.append(
                "BUILT ON OPENCODE & 9ROUTER",
                0.0,
                egui::TextFormat {
                    font_id: mono(9.0),
                    color: with_alpha(accent, fade * 0.75),
                    extra_letter_spacing: 1.2,
                    ..Default::default()
                },
            );
            let bg2 = ui.fonts(|f| f.layout_job(built));
            ui.painter().galley(
                egui::pos2(center.x - bg2.size().x * 0.5, center.y + 22.0),
                bg2,
                with_alpha(accent, fade * 0.75),
            );

            // progress
            let bar_w = 250.0;
            let bar = egui::Rect::from_center_size(
                egui::pos2(center.x, center.y + 64.0),
                Vec2::new(bar_w, 2.0),
            );
            p.rect_filled(bar, CornerRadius::same(1), BORDER_SOFT);
            let prog = app.boot_progress();
            let filled =
                egui::Rect::from_min_size(bar.min, Vec2::new((bar_w * prog).max(2.0), 2.0));
            p.rect_filled(filled, CornerRadius::same(1), with_alpha(accent, fade));

            // stage text
            let (stage_name, stage_sub) = STAGES[app.boot_stage.min(STAGES.len() - 1)];
            let mut s = egui::text::LayoutJob::default();
            s.append(
                stage_name,
                0.0,
                egui::TextFormat {
                    font_id: font_semi(9.5),
                    color: with_alpha(TEXT_DIM, fade),
                    extra_letter_spacing: 1.8,
                    ..Default::default()
                },
            );
            let sg = ui.fonts(|f| f.layout_job(s));
            ui.painter().galley(
                egui::pos2(center.x - sg.size().x * 0.5, center.y + 86.0),
                sg,
                with_alpha(TEXT_DIM, fade),
            );

            let mut sub = egui::text::LayoutJob::default();
            sub.append(
                stage_sub,
                0.0,
                egui::TextFormat {
                    font_id: mono(9.5),
                    color: with_alpha(TEXT_FAINT, fade),
                    ..Default::default()
                },
            );
            let subg = ui.fonts(|f| f.layout_job(sub));
            ui.painter().galley(
                egui::pos2(center.x - subg.size().x * 0.5, center.y + 104.0),
                subg,
                with_alpha(TEXT_FAINT, fade),
            );

            // corner meta
            let l = format!(
                "BUILD {} · RUST · {} SOURCES",
                env!("CARGO_PKG_VERSION"),
                app.cfg.providers.iter().filter(|p| p.usable()).count()
            );
            let mut j = egui::text::LayoutJob::default();
            j.append(
                &l,
                0.0,
                egui::TextFormat {
                    font_id: mono(9.0),
                    color: with_alpha(TEXT_FAINT, fade * 0.9),
                    ..Default::default()
                },
            );
            let g = ui.fonts(|f| f.layout_job(j));
            ui.painter().galley(
                egui::pos2(rect.left() + 22.0, rect.bottom() - 26.0),
                g,
                with_alpha(TEXT_FAINT, fade),
            );

            let mut j2 = egui::text::LayoutJob::default();
            j2.append(
                "LOCAL FIRST · NO CLOUD RELAY",
                0.0,
                egui::TextFormat {
                    font_id: mono(9.0),
                    color: with_alpha(TEXT_FAINT, fade * 0.9),
                    ..Default::default()
                },
            );
            let g2 = ui.fonts(|f| f.layout_job(j2));
            ui.painter().galley(
                egui::pos2(rect.right() - 22.0 - g2.size().x, rect.bottom() - 26.0),
                g2,
                with_alpha(TEXT_FAINT, fade),
            );

            let _ = (
                Layout::top_down(Align::Center),
                Margin::ZERO,
                Sense::hover(),
                Align2::CENTER_CENTER,
            );
        });
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (255.0 * a.clamp(0.0, 1.0)) as u8)
}

#[allow(dead_code)]
fn unused(_: &mut Ui) {}
