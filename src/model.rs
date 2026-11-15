//! Core data model: routing targets, sessions, messages, settings.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn short_id() -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", n % 0xFFFF_FFFF_FFFF)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    /// Context handoff notice — carried into the transcript, never sent to a model.
    Handoff,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub text: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub ts: u64,
    #[serde(default)]
    pub error: bool,
}

impl Message {
    pub fn user(text: &str) -> Self {
        Self {
            role: Role::User,
            text: text.to_string(),
            provider: String::new(),
            model: String::new(),
            tokens: 0,
            ts: now(),
            error: false,
        }
    }

    pub fn assistant(provider: &str, model: &str) -> Self {
        Self {
            role: Role::Assistant,
            text: String::new(),
            provider: provider.to_string(),
            model: model.to_string(),
            tokens: 0,
            ts: now(),
            error: false,
        }
    }

    pub fn handoff(text: &str) -> Self {
        Self {
            role: Role::Handoff,
            text: text.to_string(),
            provider: String::new(),
            model: String::new(),
            tokens: 0,
            ts: now(),
            error: false,
        }
    }
}

/// How ROXY talks to a model source.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// OpenAI-compatible HTTP endpoint reached with a key.
    #[default]
    Api,
    /// A locally installed agent CLI driven as a subprocess.
    Cli,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    /// Short label shown next to messages, e.g. "AGY".
    #[serde(default)]
    pub short: String,
    #[serde(default)]
    pub kind: Kind,
    /// OpenAI-compatible base URL, e.g. https://api.openai.com/v1
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    /// Environment variable to fall back to when `api_key` is empty.
    #[serde(default)]
    pub key_env: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub note: String,
    /// Comma-separated prefixes used to show only this provider's models.
    #[serde(default)]
    pub filter: String,
    /// Catalogue entry this was created from (models.dev id or CLI spec id).
    #[serde(default)]
    pub catalog: String,
    /// CLI spec id, when `kind` is Cli.
    #[serde(default)]
    pub cli: String,
    /// Custom command template for hand-wired CLIs. `{prompt}` / `{model}`.
    #[serde(default)]
    pub cmd: String,
    /// Ordered router priority. When set, failed turns fall through to the next
    /// model in this list instead of stopping.
    #[serde(default)]
    pub priority: Vec<String>,
    #[serde(default)]
    pub added: u64,
    #[serde(skip)]
    pub status: Option<bool>,
    #[serde(skip)]
    pub latency: Option<u64>,
    /// Runtime readiness: key present for APIs, binary found for CLIs.
    #[serde(skip)]
    pub ready: bool,
    #[serde(skip)]
    pub version: String,
}

impl Provider {
    pub fn new(
        id: &str,
        name: &str,
        short: &str,
        base_url: &str,
        model: &str,
        color: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            short: short.to_string(),
            kind: Kind::Api,
            base_url: base_url.to_string(),
            api_key: String::new(),
            key_env: String::new(),
            models: vec![model.to_string()],
            model: model.to_string(),
            enabled: true,
            color: color.to_string(),
            note: String::new(),
            filter: String::new(),
            catalog: String::new(),
            cli: String::new(),
            cmd: String::new(),
            priority: Vec::new(),
            added: now(),
            status: None,
            latency: None,
            ready: false,
            version: String::new(),
        }
    }

    pub fn cli(id: &str, name: &str, short: &str, spec: &str, model: &str, color: &str) -> Self {
        let mut p = Self::new(id, name, short, "", model, color);
        p.kind = Kind::Cli;
        p.cli = spec.to_string();
        p.catalog = spec.to_string();
        p
    }

    pub fn accent(&self) -> egui::Color32 {
        match self.color.as_str() {
            "orange" => egui::Color32::from_rgb(255, 138, 61),
            "blue" => egui::Color32::from_rgb(108, 140, 255),
            "green" => egui::Color32::from_rgb(16, 163, 127),
            "violet" => egui::Color32::from_rgb(167, 139, 250),
            "cyan" => egui::Color32::from_rgb(77, 197, 225),
            "pink" => egui::Color32::from_rgb(236, 120, 180),
            "amber" => egui::Color32::from_rgb(255, 184, 77),
            "red" => egui::Color32::from_rgb(236, 106, 96),
            "teal" => egui::Color32::from_rgb(45, 212, 191),
            _ => egui::Color32::from_rgb(154, 154, 166),
        }
    }

    pub fn endpoint(&self) -> String {
        let base = self.base_url.trim_end_matches('/').to_string();
        if base.ends_with("/chat/completions") {
            base
        } else {
            format!("{base}/chat/completions")
        }
    }

    /// Key from config, or from the provider's conventional environment variable.
    pub fn effective_key(&self) -> String {
        if !self.api_key.trim().is_empty() {
            return self.api_key.trim().to_string();
        }
        if self.key_env.trim().is_empty() {
            return String::new();
        }
        std::env::var(&self.key_env).unwrap_or_default()
    }

    pub fn key_present(&self) -> bool {
        !self.effective_key().is_empty()
    }

    /// A server running on this machine — no key required.
    pub fn local(&self) -> bool {
        let u = self.base_url.to_lowercase();
        u.contains("127.0.0.1")
            || u.contains("localhost")
            || u.contains("0.0.0.0")
            || u.contains("[::1]")
    }

    /// Does this route still need the user to paste a key?
    pub fn needs_key(&self) -> bool {
        !self.is_cli() && !self.key_present() && !self.local()
    }

    pub fn is_cli(&self) -> bool {
        self.kind == Kind::Cli
    }

    /// Is this provider usable right now?
    pub fn usable(&self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.is_cli() {
            self.ready
        } else {
            !self.base_url.trim().is_empty()
        }
    }

    /// Models this provider should advertise, honouring `filter`.
    pub fn visible_models(&self) -> Vec<String> {
        let mut all = if self.models.is_empty() {
            vec![self.model.clone()]
        } else {
            self.models.clone()
        };
        if !self.filter.trim().is_empty() && !self.is_cli() {
            let pref: Vec<String> = self
                .filter
                .split(',')
                .map(|x| x.trim().to_lowercase())
                .filter(|x| !x.is_empty())
                .collect();
            all = all
                .into_iter()
                .filter(|m| pref.iter().any(|p| m.to_lowercase().starts_with(p)))
                .collect();
        }
        all.retain(|m| !m.trim().is_empty());
        all.dedup();
        all
    }

    /// Is this provider a 9Router-style priority router?
    pub fn is_router(&self) -> bool {
        self.catalog == "9router" || self.id == "9router"
    }

    /// Every model in the user's priority order, followed by the rest.
    /// Used both by the picker and by automatic fallback.
    pub fn ordered_models(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for m in &self.priority {
            if !m.trim().is_empty() && !out.iter().any(|x| x == m) {
                out.push(m.clone());
            }
        }
        for m in self.visible_models() {
            if !out.iter().any(|x| x == &m) {
                out.push(m);
            }
        }
        out
    }

    pub fn models_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/').to_string();
        let base = base.replace("/chat/completions", "");
        format!("{base}/models")
    }

    /// Host shown under the name in the UI.
    pub fn host(&self) -> String {
        let url = self.base_url.trim();
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);
        rest.split('/').next().unwrap_or(rest).to_string()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created: u64,
    pub updated: u64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tokens_in: u64,
    #[serde(default)]
    pub tokens_out: u64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
}

impl Session {
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            id: short_id(),
            title: "New session".to_string(),
            created: now(),
            updated: now(),
            provider: provider.to_string(),
            model: model.to_string(),
            messages: Vec::new(),
            tokens_in: 0,
            tokens_out: 0,
            pinned: false,
            archived: false,
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.tokens_in + self.tokens_out
    }

    pub fn preview(&self) -> String {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| {
                m.text
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(90)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn derive_title(&mut self) {
        if let Some(m) = self.messages.iter().find(|m| m.role == Role::User) {
            let t: String = m.text.trim().chars().take(46).collect();
            let t = t.replace('\n', " ");
            if !t.trim().is_empty() {
                self.title = if m.text.trim().chars().count() > 46 {
                    format!("{}…", t.trim())
                } else {
                    t.trim().to_string()
                };
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub providers: Vec<Provider>,
    pub accent: String,
    pub streaming: bool,
    pub temperature: f32,
    pub system_prompt: String,
    #[serde(default = "default_true")]
    pub send_on_enter: bool,
    #[serde(default = "default_true")]
    pub auto_title: bool,
    #[serde(default = "default_true")]
    pub show_tokens: bool,
    #[serde(default)]
    pub max_tokens: u32,
    /// How many past turns to replay. 0 = the whole conversation.
    #[serde(default)]
    pub history_limit: usize,
    #[serde(default = "default_scale")]
    pub font_scale: f32,
    #[serde(default = "default_true")]
    pub compact: bool,
    #[serde(default = "default_width")]
    pub chat_width: f32,
    #[serde(default = "default_true")]
    pub motion: bool,
    /// Working directory handed to agent CLIs.
    #[serde(default)]
    pub cli_dir: String,
    /// "safe" keeps the CLI's own approval prompts; "full" skips them.
    #[serde(default = "default_safe")]
    pub cli_permissions: String,
    #[serde(default = "default_timeout")]
    pub cli_timeout: u64,
    #[serde(default = "default_true")]
    pub confirm_delete: bool,
    #[serde(default)]
    pub default_provider: String,
    #[serde(default)]
    pub default_model: String,
}

fn default_true() -> bool {
    true
}
fn default_scale() -> f32 {
    1.0
}
fn default_width() -> f32 {
    820.0
}
fn default_safe() -> String {
    "safe".to_string()
}
fn default_timeout() -> u64 {
    900
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 2,
            providers: crate::cli::default_providers(),
            accent: "lime".to_string(),
            streaming: true,
            temperature: 0.7,
            system_prompt:
                "You are ROXY, a precise and fast assistant. Answer directly, use markdown when it helps."
                    .to_string(),
            send_on_enter: true,
            auto_title: true,
            show_tokens: true,
            max_tokens: 0,
            history_limit: 0,
            font_scale: 1.0,
            compact: true,
            chat_width: 820.0,
            motion: true,
            cli_dir: String::new(),
            cli_permissions: "safe".to_string(),
            cli_timeout: 900,
            confirm_delete: true,
            default_provider: String::new(),
            default_model: String::new(),
        }
    }
}

pub fn approx_tokens(text: &str) -> u64 {
    ((text.chars().count() as f64) / 4.0).ceil() as u64
}
