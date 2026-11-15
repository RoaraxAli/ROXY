//! Image based icons and small UI primitives.

use crate::theme::*;
use egui::{
    Align2, Color32, CornerRadius, FontId, Margin, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Icon {
    Chat,
    Models,
    Settings,
    Plus,
    Send,
    Stop,
    Search,
    Trash,
    Close,
    Refresh,
    Info,
    Arrow,
}

fn icon_asset(icon: Icon) -> (&'static str, &'static [u8]) {
    match icon {
        Icon::Chat => (
            "chat",
            include_bytes!("../../assets/icons/message-circle.png"),
        ),
        Icon::Models => (
            "models",
            include_bytes!("../../assets/icons/layout-grid.png"),
        ),
        Icon::Settings => (
            "settings",
            include_bytes!("../../assets/icons/settings.png"),
        ),
        Icon::Plus => ("plus", include_bytes!("../../assets/icons/plus.png")),
        Icon::Send => ("send", include_bytes!("../../assets/icons/send.png")),
        Icon::Stop => ("stop", include_bytes!("../../assets/icons/square.png")),
        Icon::Search => ("search", include_bytes!("../../assets/icons/search.png")),
        Icon::Trash => ("trash", include_bytes!("../../assets/icons/trash.png")),
        Icon::Close => ("close", include_bytes!("../../assets/icons/x.png")),
        Icon::Refresh => (
            "refresh",
            include_bytes!("../../assets/icons/refresh-cw.png"),
        ),
        Icon::Info => ("info", include_bytes!("../../assets/icons/info.png")),
        Icon::Arrow => (
            "arrow",
            include_bytes!("../../assets/icons/arrow-right.png"),
        ),
    }
}

pub fn draw_icon(p: &egui::Painter, c: egui::Pos2, s: f32, icon: Icon, color: Color32) {
    let (key, bytes) = icon_asset(icon);
    let ctx = p.ctx();
    let id = egui::Id::new(("roxy-icon", key));
    let texture = ctx.data_mut(|data| data.get_temp::<egui::TextureHandle>(id));
    let texture = texture.unwrap_or_else(|| {
        let (width, height, rgba) =
            crate::image::decode_png_rgba(bytes).expect("bundled icon PNG is valid");
        let image =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
        let texture = ctx.load_texture(
            format!("roxy-icon-{key}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
        texture
    });
    let rect = Rect::from_center_size(c, Vec2::splat(s));
    p.image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        color,
    );
}

/// A square, borderless icon button used by the rail and headers.
pub fn icon_btn(ui: &mut Ui, icon: Icon, size: f32, tint: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    let hovering = resp.hovered();
    if hovering {
        p.rect_filled(rect, CornerRadius::same(10), Color32::from_white_alpha(10));
    }
    let c = if hovering { TEXT } else { tint };
    draw_icon(p, rect.center(), size * 0.46, icon, c);
    resp
}

pub fn hairline(ui: &mut Ui, color: Color32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// Micro label used for section headers.
pub fn section_label(ui: &mut Ui, text: &str, color: Color32) {
    label_micro(ui, text, color);
}

/// Compact rounded tag with an optional status dot on the left.
pub fn chip_dot(
    ui: &mut Ui,
    text: &str,
    fg: Color32,
    bg: Color32,
    border: Color32,
    dot: Option<Color32>,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), theme_chip_font(), fg);
    let lead = if dot.is_some() { 13.0 } else { 0.0 };
    let pad = Vec2::new(8.0, 4.0);
    let size = galley.size() + pad * 2.0 + Vec2::new(lead, 0.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let hover = resp.hovered();
    p.rect(
        rect,
        CornerRadius::same(7),
        bg,
        Stroke::new(
            1.0_f32,
            if hover {
                border.gamma_multiply(1.6)
            } else {
                border
            },
        ),
        StrokeKind::Inside,
    );
    if let Some(c) = dot {
        p.circle_filled(
            egui::pos2(rect.left() + pad.x + 2.5, rect.center().y),
            3.0,
            c,
        );
    }
    p.galley(rect.min + pad + Vec2::new(lead, 0.0), galley, fg);
    resp
}

/// Compact rounded tag.
pub fn chip(ui: &mut Ui, text: &str, fg: Color32, bg: Color32, border: Color32) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), theme_chip_font(), fg);
    let pad = Vec2::new(8.0, 4.0);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(7),
        bg,
        Stroke::new(
            1.0_f32,
            if resp.hovered() {
                border.gamma_multiply(1.5)
            } else {
                border
            },
        ),
        StrokeKind::Inside,
    );
    p.galley(rect.min + pad, galley, fg);
    resp
}

fn theme_chip_font() -> FontId {
    font_med(11.5)
}

pub fn keycap(ui: &mut Ui, label: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), mono(10.0), TEXT_FAINT);
    let pad = Vec2::new(5.0, 2.0);
    let size = galley.size() + pad * 2.0;
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(4),
        Color32::TRANSPARENT,
        Stroke::new(1.0_f32, BORDER),
        StrokeKind::Inside,
    );
    p.galley(rect.min + pad, galley, TEXT_FAINT);
}

/// Simple filled primary button (accent) or ghost button.
pub fn action_button(
    ui: &mut Ui,
    text: &str,
    accent: Color32,
    filled: bool,
    width: Option<f32>,
) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        font_med(13.5),
        if filled { BG_DEEP } else { TEXT },
    );
    let h = 34.0;
    let w = width.unwrap_or(galley.size().x + 28.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let p = ui.painter();
    let hover = resp.hovered();
    let fill = if filled {
        if hover {
            accent.gamma_multiply(1.08)
        } else {
            accent
        }
    } else if hover {
        SURFACE_HI
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if filled {
        Stroke::NONE
    } else {
        Stroke::new(1.0_f32, if hover { BORDER } else { BORDER_SOFT })
    };
    p.rect(
        rect,
        CornerRadius::same(9),
        fill,
        stroke,
        StrokeKind::Inside,
    );
    let fg = if filled { BG_DEEP } else { TEXT };
    p.galley(
        rect.center() - galley.size() * 0.5,
        ui.painter()
            .layout_no_wrap(text.to_string(), font_med(13.5), fg),
        fg,
    );
    resp
}

pub fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn time_ago(ts: u64) -> String {
    let now = crate::model::now();
    if ts == 0 || now <= ts {
        return "now".into();
    }
    let d = now - ts;
    if d < 60 {
        format!("{d}s")
    } else if d < 3600 {
        format!("{}m", d / 60)
    } else if d < 86_400 {
        format!("{}h", d / 3600)
    } else {
        format!("{}d", d / 86_400)
    }
}

pub fn label_micro(ui: &mut Ui, text: &str, color: Color32) {
    let job = micro_label(text, color, 10.0, 1.5);
    let galley = ui.fonts(|f| f.layout_job(job));
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}
// ------------------------------------------------------------ modern controls

/// iOS-style switch. Returns true when the value changed.
pub fn switch(ui: &mut Ui, on: &mut bool, accent: Color32) -> bool {
    let size = Vec2::new(38.0, 21.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let t = if resp.hovered() { 1.0 } else { 0.0 };
    let track = if *on {
        if resp.hovered() {
            accent.gamma_multiply(1.05)
        } else {
            accent
        }
    } else if resp.hovered() {
        SURFACE_HI
    } else {
        BORDER
    };
    p.rect_filled(rect, CornerRadius::same(11), track);
    if !*on {
        p.rect_stroke(
            rect,
            CornerRadius::same(11),
            Stroke::new(1.0_f32, BORDER_SOFT),
            StrokeKind::Inside,
        );
    }
    let r = 8.0;
    let cx = if *on {
        rect.right() - r - 2.5
    } else {
        rect.left() + r + 2.5
    };
    p.circle_filled(
        egui::pos2(cx, rect.center().y),
        r,
        if *on { BG_DEEP } else { TEXT_FAINT },
    );
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        let _ = t;
        false
    }
}

/// Setting row: label + optional hint on the left, control on the right.
pub fn setting_row(ui: &mut Ui, label: &str, hint: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).font(font_med(13.0)).color(TEXT));
            if !hint.is_empty() {
                ui.add_space(1.0);
                ui.label(RichText::new(hint).font(font(11.5)).color(TEXT_FAINT));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(2.0);
            control(ui);
        });
    });
}

/// Segmented control. Returns the clicked index.
pub fn segmented(ui: &mut Ui, labels: &[&str], active: usize, accent: Color32) -> Option<usize> {
    let pad = Vec2::new(12.0, 5.0);
    let mut widths = Vec::new();
    for l in labels {
        let g = ui
            .painter()
            .layout_no_wrap((*l).to_string(), font_med(12.5), TEXT);
        widths.push(g.size().x + pad.x * 2.0);
    }
    let h = 30.0;
    let total: f32 = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, h), Sense::hover());
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(9),
        BG_DEEP,
        Stroke::new(1.0_f32, BORDER_SOFT),
        StrokeKind::Inside,
    );
    let mut x = rect.left() + 2.0;
    let mut clicked = None;
    for (i, l) in labels.iter().enumerate() {
        let w = widths[i];
        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 2.0), Vec2::new(w, h - 4.0));
        let resp = ui.interact(r, ui.id().with(("seg", i, l)), Sense::click());
        if i == active {
            p.rect_filled(r, CornerRadius::same(7), SURFACE_HI);
            p.rect_stroke(
                r,
                CornerRadius::same(7),
                Stroke::new(1.0_f32, BORDER),
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(7), Color32::from_white_alpha(5));
        }
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            *l,
            font_med(12.5),
            if i == active { TEXT } else { TEXT_DIM },
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && i != active {
            clicked = Some(i);
        }
        x += w;
    }
    let _ = accent;
    clicked
}

/// Rounded square avatar with initials.
pub fn avatar(ui: &mut Ui, label: &str, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(
        rect,
        CornerRadius::same((size * 0.29) as u8),
        color.gamma_multiply(0.16),
    );
    p.rect_stroke(
        rect,
        CornerRadius::same((size * 0.29) as u8),
        Stroke::new(1.0_f32, color.gamma_multiply(0.35)),
        StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label.chars().take(3).collect::<String>(),
        font_semi(size * 0.36),
        color,
    );
}

/// Small status pill: dot + text.
pub fn pill(ui: &mut Ui, text: &str, dot: Option<Color32>, fg: Color32, bg: Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), mono(9.5), fg);
    let lead = if dot.is_some() { 10.0 } else { 0.0 };
    let pad = Vec2::new(7.0, 3.0);
    let (rect, _) = ui.allocate_exact_size(
        galley.size() + pad * 2.0 + Vec2::new(lead, 0.0),
        Sense::hover(),
    );
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(5), bg);
    if let Some(c) = dot {
        p.circle_filled(
            egui::pos2(rect.left() + pad.x + 2.5, rect.center().y),
            2.5,
            c,
        );
    }
    p.galley(rect.min + pad + Vec2::new(lead, 0.0), galley, fg);
}

/// Ghost button with an icon and label.
pub fn small_button(ui: &mut Ui, label: &str, icon: Option<Icon>, tint: Color32) -> Response {
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_med(11.5), tint);
    let lead = if icon.is_some() { 15.0 } else { 0.0 };
    let pad = Vec2::new(9.0, 4.0);
    let (rect, resp) =
        ui.allocate_exact_size(g.size() + pad * 2.0 + Vec2::new(lead, 0.0), Sense::click());
    let p = ui.painter();
    let hov = resp.hovered();
    p.rect(
        rect,
        CornerRadius::same(7),
        if hov {
            SURFACE_HI
        } else {
            Color32::TRANSPARENT
        },
        Stroke::new(1.0_f32, if hov { BORDER } else { BORDER_SOFT }),
        StrokeKind::Inside,
    );
    if let Some(i) = icon {
        draw_icon(
            p,
            egui::pos2(rect.left() + pad.x + 5.0, rect.center().y),
            11.0,
            i,
            tint,
        );
    }
    p.galley(
        rect.min + pad + Vec2::new(lead, 0.0),
        g,
        if hov { TEXT } else { tint },
    );
    if hov {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Monospace block used for install logs and commands.
pub fn code_block(ui: &mut Ui, text: &str, height: f32) {
    egui::Frame::new()
        .fill(BG_DEEP)
        .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .max_height(height)
                .auto_shrink([false, true])
                .id_salt(("code", text.len(), height as i32))
                .show(ui, |ui| {
                    ui.label(RichText::new(text).font(mono(10.5)).color(TEXT_DIM));
                });
        });
}
