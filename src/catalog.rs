//! Embedded catalogue of OpenAI-compatible providers (sourced from models.dev
//! plus a few local runtimes). No network needed, no account baked in: the
//! user brings their own key and ROXY fills in the endpoint.

use crate::model::Provider;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize, Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub api: String,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Deserialize)]
struct File {
    providers: Vec<Entry>,
}

static DATA: &str = include_str!("../assets/catalog/providers.json");
static PARSED: OnceLock<Vec<Entry>> = OnceLock::new();

pub fn all() -> &'static [Entry] {
    PARSED.get_or_init(|| {
        serde_json::from_str::<File>(DATA)
            .map(|f| f.providers)
            .unwrap_or_default()
    })
}

const COLORS: &[&str] = &[
    "violet", "green", "blue", "orange", "cyan", "pink", "amber", "teal", "red",
];

fn color_for(id: &str) -> &'static str {
    let sum: u32 = id.bytes().map(|b| b as u32).sum();
    COLORS[(sum as usize) % COLORS.len()]
}

fn short_for(name: &str) -> String {
    let mut out = String::new();
    for word in name.split_whitespace().take(3) {
        if let Some(c) = word.chars().next() {
            if c.is_alphanumeric() {
                out.extend(c.to_uppercase());
            }
        }
    }
    if out.is_empty() {
        name.chars().take(3).collect::<String>().to_uppercase()
    } else {
        out
    }
}

/// Build a routable provider from a catalogue entry.
pub fn provider(entry: &Entry) -> Provider {
    let id = format!("api-{}", entry.id);
    let default_model = entry.models.first().cloned().unwrap_or_default();
    let mut p = Provider::new(
        &id,
        &entry.name,
        &short_for(&entry.name),
        &entry.api,
        &default_model,
        color_for(&entry.id),
    );
    p.models = entry.models.clone();
    p.key_env = entry.env.clone();
    p.catalog = entry.id.clone();
    p.note = entry.doc.clone();
    p.ready = true;
    p
}
