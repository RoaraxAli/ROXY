//! ROXY — theme, palette and typography.

use egui::{Color32, FontFamily, FontId, TextStyle, Visuals};
use std::sync::Arc;

pub const BG_DEEP: Color32 = Color32::from_rgb(7, 7, 9);
pub const BG: Color32 = Color32::from_rgb(11, 11, 13);
pub const PANEL: Color32 = Color32::from_rgb(14, 14, 17);
pub const SURFACE: Color32 = Color32::from_rgb(20, 20, 24);
pub const SURFACE_HI: Color32 = Color32::from_rgb(27, 27, 32);
pub const BORDER: Color32 = Color32::from_rgb(38, 38, 45);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(25, 25, 30);
pub const TEXT: Color32 = Color32::from_rgb(236, 236, 240);
pub const TEXT_DIM: Color32 = Color32::from_rgb(150, 150, 162);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(104, 104, 117);
pub const DANGER: Color32 = Color32::from_rgb(255, 92, 92);
pub const WARN: Color32 = Color32::from_rgb(255, 184, 77);
pub const OK: Color32 = Color32::from_rgb(110, 214, 122);

const INTER_REG: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_MED: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
const INTER_SEMI: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
const JBM_REG: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
const JBM_MED: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf");

static CURRENT_ACCENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_accent(c: Color32) {
    CURRENT_ACCENT.store(
        ((c.r() as u32) << 24) | ((c.g() as u32) << 16) | ((c.b() as u32) << 8) | 255,
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub fn current_accent() -> Color32 {
    let v = CURRENT_ACCENT.load(std::sync::atomic::Ordering::Relaxed);
    if v == 0 {
        return accent_color("lime");
    }
    Color32::from_rgba_premultiplied(
        (v >> 24) as u8,
        ((v >> 16) & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        255,
    )
}

pub fn accent_color(name: &str) -> Color32 {
    match name {
        "violet" => Color32::from_rgb(139, 108, 255),
        "amber" => Color32::from_rgb(255, 184, 77),
        "cyan" => Color32::from_rgb(77, 225, 255),
        "coral" => Color32::from_rgb(255, 106, 92),
        _ => Color32::from_rgb(200, 247, 81),
    }
}

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn font_semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semi".into()))
}

pub fn font_med(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("med".into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Apply user-controllable scale and density without a restart.
pub fn apply(ctx: &egui::Context, scale: f32, compact: bool) {
    let s = scale.clamp(0.8, 1.35);
    ctx.set_zoom_factor(s);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, if compact { 7.0 } else { 11.0 });
    style.visuals.selection.bg_fill = current_accent().gamma_multiply(0.28);
    ctx.set_style(style);
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(egui::FontData::from_static(INTER_REG)),
    );
    fonts.font_data.insert(
        "inter-med".to_owned(),
        Arc::new(egui::FontData::from_static(INTER_MED)),
    );
    fonts.font_data.insert(
        "inter-semi".to_owned(),
        Arc::new(egui::FontData::from_static(INTER_SEMI)),
    );
    fonts.font_data.insert(
        "jbm".to_owned(),
        Arc::new(egui::FontData::from_static(JBM_REG)),
    );
    fonts.font_data.insert(
        "jbm-med".to_owned(),
        Arc::new(egui::FontData::from_static(JBM_MED)),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());

    fonts.families.insert(
        FontFamily::Name("med".into()),
        vec!["inter-med".to_owned(), "inter".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("semi".into()),
        vec!["inter-semi".to_owned(), "inter-med".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Monospace,
        vec!["jbm".to_owned(), "jbm-med".to_owned(), "inter".to_owned()],
    );

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BG_DEEP;
    visuals.faint_bg_color = SURFACE;
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER_SOFT);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);

    let w = &mut visuals.widgets;
    w.inactive.bg_fill = SURFACE;
    w.inactive.weak_bg_fill = SURFACE;
    w.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    w.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_DIM);
    w.inactive.corner_radius = egui::CornerRadius::same(8);

    w.hovered.bg_fill = SURFACE_HI;
    w.hovered.weak_bg_fill = SURFACE_HI;
    w.hovered.bg_stroke = egui::Stroke::new(1.0_f32, BORDER.gamma_multiply(1.4));
    w.hovered.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    w.hovered.corner_radius = egui::CornerRadius::same(8);

    w.active.bg_fill = SURFACE_HI;
    w.active.weak_bg_fill = SURFACE_HI;
    w.active.bg_stroke = egui::Stroke::new(1.0_f32, BORDER.gamma_multiply(1.6));
    w.active.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    w.active.corner_radius = egui::CornerRadius::same(8);

    w.open.bg_fill = SURFACE;
    w.open.weak_bg_fill = SURFACE;
    w.open.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    w.open.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    w.open.corner_radius = egui::CornerRadius::same(8);

    visuals.selection.bg_fill = accent_color("lime").gamma_multiply(0.25);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, TEXT);
    visuals.striped = false;
    style.visuals = visuals;

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = true;

    style.text_styles = [
        (TextStyle::Heading, font_semi(21.0)),
        (TextStyle::Body, font(15.0)),
        (TextStyle::Button, font_med(14.0)),
        (TextStyle::Small, font(12.5)),
        (TextStyle::Monospace, mono(13.0)),
    ]
    .into();

    ctx.set_style(style);
}

/// Draw a tiny uppercase label with tracking (the "micro type" used all over the UI).
pub fn micro_label(text: &str, color: Color32, size: f32, tracking: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: font_semi(size),
            color,
            extra_letter_spacing: tracking,
            ..Default::default()
        },
    );
    job
}
