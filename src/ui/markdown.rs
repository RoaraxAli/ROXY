//! Tiny markdown renderer: paragraphs, headings, lists, quotes, code fences,
//! inline bold / italic / code.

use crate::theme::*;
use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat, Ui};

fn base_format(size: f32, color: Color32) -> TextFormat {
    TextFormat {
        font_id: font(size),
        color,
        ..Default::default()
    }
}

/// Split inline markdown into formatted runs.
fn inline_job(text: &str, size: f32, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width.max(40.0);
    // URLs, file paths, and generated identifiers can be longer than the
    // viewport and otherwise force the transcript wider than the window.
    job.wrap.break_anywhere = true;

    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut plain = String::new();

    let flush = |job: &mut LayoutJob, plain: &mut String, fmt: TextFormat| {
        if !plain.is_empty() {
            job.append(plain, 0.0, fmt.clone());
            plain.clear();
        }
    };

    while i < bytes.len() {
        let c = bytes[i];

        // Inline code
        if c == '`' {
            if let Some(end) = (i + 1..bytes.len()).find(|&j| bytes[j] == '`') {
                flush(&mut job, &mut plain, base_format(size, color));
                let code: String = bytes[i + 1..end].iter().collect();
                job.append(
                    &code,
                    0.0,
                    TextFormat {
                        font_id: mono(size - 1.5),
                        color: Color32::from_rgb(232, 232, 238),
                        background: SURFACE_HI,
                        ..Default::default()
                    },
                );
                i = end + 1;
                continue;
            }
        }

        // Bold
        if c == '*' && i + 1 < bytes.len() && bytes[i + 1] == '*' {
            if let Some(end) = find_double(&bytes, i + 2, '*') {
                flush(&mut job, &mut plain, base_format(size, color));
                let inner: String = bytes[i + 2..end].iter().collect();
                job.append(
                    &inner,
                    0.0,
                    TextFormat {
                        font_id: font_semi(size),
                        color,
                        ..Default::default()
                    },
                );
                i = end + 2;
                continue;
            }
        }

        // Italic
        if c == '*' && (i + 1 >= bytes.len() || bytes[i + 1] != '*') {
            if let Some(end) = find_single(&bytes, i + 1, '*') {
                flush(&mut job, &mut plain, base_format(size, color));
                let inner: String = bytes[i + 1..end].iter().collect();
                job.append(
                    &inner,
                    0.0,
                    TextFormat {
                        font_id: FontId::new(size, egui::FontFamily::Name("med".into())),
                        color: color.gamma_multiply(0.92),
                        italics: true,
                        ..Default::default()
                    },
                );
                i = end + 1;
                continue;
            }
        }

        // [label](url) → label in accent
        if c == '[' {
            if let Some(close) = (i + 1..bytes.len()).find(|&j| bytes[j] == ']') {
                if close + 1 < bytes.len() && bytes[close + 1] == '(' {
                    if let Some(paren) = (close + 2..bytes.len()).find(|&j| bytes[j] == ')') {
                        flush(&mut job, &mut plain, base_format(size, color));
                        let label: String = bytes[i + 1..close].iter().collect();
                        job.append(
                            &label,
                            0.0,
                            TextFormat {
                                font_id: font_med(size),
                                color: crate::theme::current_accent().gamma_multiply(0.95),
                                underline: egui::Stroke::new(
                                    1.0_f32,
                                    crate::theme::current_accent().gamma_multiply(0.5),
                                ),
                                ..Default::default()
                            },
                        );
                        i = paren + 1;
                        continue;
                    }
                }
            }
        }

        plain.push(c);
        i += 1;
    }

    flush(&mut job, &mut plain, base_format(size, color));
    job
}

fn find_double(chars: &[char], from: usize, c: char) -> Option<usize> {
    let mut i = from;
    while i + 1 < chars.len() {
        if chars[i] == c && chars[i + 1] == c {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_single(chars: &[char], from: usize, c: char) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == c {
            let prev = i > 0 && chars[i - 1] == c;
            let next = i + 1 < chars.len() && chars[i + 1] == c;
            if !prev && !next {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn paragraph(ui: &mut Ui, text: &str, size: f32, color: Color32) {
    let width = ui.available_width();
    let job = inline_job(text, size, color, width);
    ui.add(egui::Label::new(job).selectable(true));
}

fn bullet(ui: &mut Ui, marker: &str, text: &str, size: f32, accent: Color32) {
    ui.horizontal_top(|ui| {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(marker)
                .font(font_med(size))
                .color(accent),
        );
        ui.add_space(2.0);
        let w = ui.available_width() - 6.0;
        let job = inline_job(text, size, TEXT, w);
        ui.add(egui::Label::new(job).selectable(true));
    });
}

fn code_block(ui: &mut Ui, code: &str, lang: &str) {
    let full = ui.available_width();
    egui::Frame::new()
        .fill(Color32::from_rgb(16, 16, 20))
        .stroke(egui::Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(full - 26.0);
            if !lang.trim().is_empty() {
                ui.label(
                    egui::RichText::new(lang.trim().to_uppercase())
                        .font(font_semi(9.5))
                        .color(TEXT_FAINT),
                );
                ui.add_space(4.0);
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(code.trim_end())
                        .font(mono(12.5))
                        .color(Color32::from_rgb(214, 220, 230)),
                )
                .wrap(),
            );
        });
}

/// Render markdown text into the current layout.
pub fn render(ui: &mut Ui, text: &str) {
    let accent = crate::theme::current_accent();
    let mut lines = text.lines().peekable();
    let mut first = true;

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim().to_string();
            let mut code = String::new();
            while let Some(l) = lines.next() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(l);
                code.push('\n');
            }
            code_block(ui, &code, &lang);
            ui.add_space(10.0);
            first = true;
            continue;
        }

        if trimmed.is_empty() {
            if !first {
                ui.add_space(7.0);
            }
            continue;
        }

        if trimmed == "---" || trimmed == "***" {
            ui.add_space(6.0);
            let w = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::ZERO, BORDER_SOFT);
            ui.add_space(6.0);
            first = true;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("### ") {
            ui.add_space(4.0);
            paragraph(ui, rest, 15.5, TEXT);
            ui.add_space(4.0);
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            ui.add_space(6.0);
            paragraph(ui, rest, 17.0, TEXT);
            ui.add_space(4.0);
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            ui.add_space(8.0);
            paragraph(ui, rest, 19.5, TEXT);
            ui.add_space(5.0);
        } else if let Some(rest) = trimmed.strip_prefix("> ") {
            bullet(ui, "│", rest, 14.5, BORDER.gamma_multiply(1.4));
        } else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("• "))
        {
            bullet(ui, "•", rest, 15.0, accent.gamma_multiply(0.85));
        } else if let Some((num, rest)) = numbered(trimmed) {
            bullet(
                ui,
                &format!("{num}."),
                rest,
                15.0,
                accent.gamma_multiply(0.85),
            );
        } else {
            paragraph(ui, line, 15.0, TEXT);
        }

        if !first {
            ui.add_space(2.0);
        }
        first = false;
    }
}

fn numbered(s: &str) -> Option<(String, &str)> {
    let idx = s.find(". ")?;
    if idx == 0 || idx > 3 {
        return None;
    }
    let head = &s[..idx];
    if !head.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((head.to_string(), &s[idx + 2..]))
}
