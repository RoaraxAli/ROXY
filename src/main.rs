#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! ROXY — one session, every model.
//!
//! A native, GPU-accelerated desktop client that keeps one shared
//! conversation across hosted APIs and locally installed agent CLIs.

mod app;
mod catalog;
mod cli;
mod http;
mod image;
mod llm;
mod model;
mod store;
mod theme;
mod ui;
mod updater;

use eframe::egui;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 850.0])
            .with_min_inner_size([1000.0, 640.0])
            .with_title("ROXY")
            .with_app_id("roxy")
            .with_icon(Arc::new(icon())),
        vsync: true,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "ROXY",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

/// Load the bundled ROXY brand image for the native window icon.
fn icon() -> egui::IconData {
    let (width, height, rgba) =
        crate::image::decode_png_rgba(include_bytes!("../assets/roxy-brand.png"))
            .expect("bundled ROXY icon is a valid PNG");
    egui::IconData {
        rgba,
        width,
        height,
    }
}
