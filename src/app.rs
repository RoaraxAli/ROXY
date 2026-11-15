//! Application state and event pump.

use crate::cli::{self, Detect};
use crate::llm::{self, Disc, Ev, Opts};
use crate::model::*;
use crate::store;
use crate::theme;
use crate::ui;
use eframe::egui;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Chat,
    Models,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsTab {
    General,
    Models,
    Cli,
    Appearance,
    Data,
    About,
}

pub struct Call {
    pub rx: Receiver<Ev>,
    pub cancel: Arc<AtomicBool>,
    pub session_id: String,
    pub started: Instant,
    pub chars: usize,
}

pub enum Bg {
    Detected(String, Detect),
    OpenCodeModels(Vec<String>),
    NodeStatus(bool, String),
}

pub struct App {
    pub update_status: String,
    pub update_rx: Option<Receiver<Result<crate::updater::Release, String>>>,
    pub upd_rx: Option<Receiver<crate::updater::Upd>>,
    pub pending_release: Option<crate::updater::Release>,
    pub upd_progress: Option<(u64, Option<u64>)>,
    pub upd_stage: String,
    pub upd_error: Option<String>,
    pub cfg: Config,
    pub sessions: Vec<Session>,
    pub active: usize,
    pub view: View,
    pub draft: String,
    pub call: Option<Call>,
    pub search: String,
    pub picker: bool,
    pub picker_focus: bool,
    pub picker_query: String,
    pub picker_anchor: Option<egui::Rect>,
    pub boot_t: f32,
    pub booted: bool,
    pub boot_stage: usize,
    pub pending_discovery: usize,
    pub toast: Option<(String, f64)>,
    pub renaming: Option<(usize, String)>,
    pub deleting: Option<usize>,
    pub accent: egui::Color32,
    /// Live detection state per CLI spec id.
    pub clis: HashMap<String, Detect>,
    pub jobs: Vec<cli::Job>,
    pub settings_tab: SettingsTab,
    pub models_tab: u8,
    pub cat_search: String,
    pub cat_all: bool,
    pub cat_open: Option<String>,
    pub open_cli: Option<String>,
    pub node_ok: bool,
    pub node_version: String,
    pub adding: Option<Provider>,
    pub bg_rx: Option<Receiver<Bg>>,
    pub bg_tx: Option<std::sync::mpsc::Sender<Bg>>,
    pub disc_rx: Option<Receiver<Disc>>,
    pub disc_tx: Option<std::sync::mpsc::Sender<Disc>>,
    pub last_scale: f32,
    pub boot_skip: bool,
    pub opencode_prompt_dismissed: bool,
    /// Codex install chains into OpenCode's native OpenAI sign-in setup.
    pub codex_opencode_setup: bool,
}

/// Bring on-disk state up to the current schema. Version 1 stored endpoints of
/// someone else's gateway — those are discarded instead of carried forward.
fn migrate(cfg: &mut Config) {
    if cfg.version < 2 {
        cfg.providers = cli::default_providers();
        cfg.version = 2;
    }
    // Keep at least one routing entry alive.
    if cfg.providers.is_empty() {
        cfg.providers = cli::default_providers();
    }
    for p in cfg.providers.iter_mut() {
        if p.is_cli() {
            p.ready = false;
            p.models.retain(|m| !m.eq_ignore_ascii_case("auto"));
            if p.model.eq_ignore_ascii_case("auto") {
                p.model.clear();
            }
            if p.cmd.is_empty() {
                if let Some(s) = cli::spec(&p.cli) {
                    p.name = s.name.to_string();
                    p.short = s.short.to_string();
                    if p.models.is_empty() {
                        p.models = s.models.iter().map(|m| m.to_string()).collect();
                    }
                    if p.color.is_empty() {
                        p.color = s.color.to_string();
                    }
                }
            }
        } else {
            p.ready = true;
        }
        if p.model.is_empty() {
            p.model = p.models.first().cloned().unwrap_or_default();
        }
    }
    // 9Router ships with every install: it is the priority router the picker
    // and automatic fallback read from.
    if !cfg.providers.iter().any(|p| p.is_router()) {
        cfg.providers.push(cli::router_provider());
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);

        let mut cfg = store::load_config();
        migrate(&mut cfg);
        let mut sessions = store::load_sessions();
        if sessions.is_empty() {
            sessions.push(Session::new("", ""));
        }
        sessions.sort_by(|a, b| b.updated.cmp(&a.updated));

        let accent = theme::accent_color(&cfg.accent);
        theme::apply(&cc.egui_ctx, cfg.font_scale, cfg.compact);

        let (bg_tx, bg_rx) = channel::<Bg>();
        let (disc_tx, disc_rx) = channel::<Disc>();
        let (update_tx, update_rx) = channel();
        let update_ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let _ = update_tx.send(crate::updater::check());
            update_ctx.request_repaint();
        });
        let ctx = cc.egui_ctx.clone();

        // Node detection can wait on PATH lookup or process startup. Keep it
        // off the window-creation path so the first frame appears immediately.
        let node_tx = bg_tx.clone();
        let node_ctx = ctx.clone();
        std::thread::spawn(move || {
            let (ok, version) = cli::node_status();
            let _ = node_tx.send(Bg::NodeStatus(ok, version));
            node_ctx.request_repaint();
        });

        // Detect every catalogued CLI in parallel; each reports back once.
        for spec in cli::SPECS {
            let tx = bg_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let d = cli::detect(spec);
                let _ = tx.send(Bg::Detected(spec.id.to_string(), d));
                ctx.request_repaint();
            });
        }

        // Discover models for API providers that can answer.
        let mut pending = 0;
        for p in cfg.providers.iter().filter(|p| !p.is_cli()) {
            if !p.enabled {
                continue;
            }
            if !p.needs_key() {
                llm::spawn_discovery(p.clone(), disc_tx.clone(), ctx.clone());
                pending += 1;
            }
        }

        let last_scale = cfg.font_scale;
        let mut app = Self {
            update_status: "Checking for updates…".into(),
            update_rx: Some(update_rx),
            upd_rx: None,
            pending_release: None,
            upd_progress: None,
            upd_stage: String::new(),
            upd_error: None,
            cfg,
            sessions,
            active: 0,
            view: View::Chat,
            draft: String::new(),
            call: None,
            search: String::new(),
            picker: false,
            picker_focus: false,
            picker_query: String::new(),
            picker_anchor: None,
            boot_t: 0.0,
            booted: false,
            boot_stage: 0,
            pending_discovery: pending,
            toast: None,
            renaming: None,
            deleting: None,
            accent,
            clis: HashMap::new(),
            jobs: Vec::new(),
            node_ok: false,
            node_version: String::new(),
            settings_tab: SettingsTab::General,
            models_tab: 0,
            cat_search: String::new(),
            cat_all: false,
            cat_open: None,
            open_cli: None,
            adding: None,
            bg_rx: Some(bg_rx),
            bg_tx: Some(bg_tx),
            disc_rx: Some(disc_rx),
            disc_tx: Some(disc_tx),
            last_scale,
            boot_skip: false,
            opencode_prompt_dismissed: false,
            codex_opencode_setup: false,
        };
        app.heal_sessions();
        app
    }

    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.cfg.providers.iter().find(|p| p.id == id)
    }

    pub fn provider_of_active(&self) -> Option<&Provider> {
        let s = self.sessions.get(self.active)?;
        self.provider(&s.provider)
            .filter(|p| p.usable())
            .or_else(|| self.cfg.providers.iter().find(|p| p.usable()))
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.sessions.get(self.active)
    }

    pub fn active_name(&self) -> String {
        self.provider_of_active()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "No model".into())
    }

    pub fn active_model(&self) -> String {
        let Some(s) = self.sessions.get(self.active) else {
            return String::new();
        };
        if let Some(p) = self.provider(&s.provider) {
            if s.model.is_empty() || s.model.eq_ignore_ascii_case("auto") {
                return p.model.clone();
            }
        }
        s.model.clone()
    }

    pub fn streaming(&self) -> bool {
        self.call.is_some()
    }

    /// Any route that could serve a turn right now?
    pub fn usable_count(&self) -> usize {
        self.cfg.providers.iter().filter(|p| p.usable()).count()
    }

    pub fn cli_path(&self, p: &Provider) -> Option<std::path::PathBuf> {
        self.clis.get(&p.cli).and_then(|d| d.path.clone())
    }

    pub fn boot_progress(&self) -> f32 {
        let base = (self.boot_t / 0.60).clamp(0.0, 1.0);
        if self.pending_discovery > 0 {
            (base * 0.85).clamp(0.0, 0.98)
        } else {
            base
        }
    }

    pub fn heal_sessions(&mut self) {
        let fallback = self
            .cfg
            .providers
            .iter()
            .find(|p| !p.is_cli() && p.enabled)
            .or_else(|| self.cfg.providers.iter().find(|p| p.usable()))
            .map(|p| (p.id.clone(), p.model.clone()));
        for s in self.sessions.iter_mut() {
            let known = self.cfg.providers.iter().any(|p| p.id == s.provider);
            if s.provider.is_empty() || !known {
                if let Some((id, model)) = &fallback {
                    s.provider = id.clone();
                    if s.model.is_empty() {
                        s.model = model.clone();
                    }
                }
            }
        }
    }

    /// Re-point sessions that were left on a missing CLI once it is removed.
    pub fn route_after_removal(&mut self, gone: &str) {
        let fallback = self
            .cfg
            .providers
            .iter()
            .find(|p| p.usable())
            .map(|p| (p.id.clone(), p.model.clone()));
        for s in self.sessions.iter_mut() {
            if s.provider == gone {
                if let Some((id, model)) = &fallback {
                    s.provider = id.clone();
                    s.model = model.clone();
                } else {
                    s.provider.clear();
                }
            }
        }
    }

    fn default_route(&self) -> (String, String) {
        if !self.cfg.default_provider.is_empty() {
            if let Some(p) = self.provider(&self.cfg.default_provider) {
                let m = if self.cfg.default_model.is_empty() {
                    p.model.clone()
                } else {
                    self.cfg.default_model.clone()
                };
                return (p.id.clone(), m);
            }
        }
        self.cfg
            .providers
            .iter()
            .find(|p| p.usable())
            .map(|p| (p.id.clone(), p.model.clone()))
            .unwrap_or_default()
    }

    pub fn new_chat(&mut self) {
        let (pid, model) = self.default_route();
        self.sessions.insert(0, Session::new(&pid, &model));
        self.active = 0;
        self.view = View::Chat;
        self.draft.clear();
        self.persist();
    }

    pub fn delete_session(&mut self, idx: usize) {
        if idx >= self.sessions.len() {
            return;
        }
        if let Some(s) = self.sessions.get(idx) {
            if self.call.as_ref().map(|c| &c.session_id) == Some(&s.id) {
                self.stop();
            }
        }
        self.sessions.remove(idx);
        if self.sessions.is_empty() {
            let (pid, model) = self.default_route();
            self.sessions.push(Session::new(&pid, &model));
            self.active = 0;
        }
        if self.active >= self.sessions.len() {
            self.active = self.sessions.len().saturating_sub(1);
        }
        self.persist();
    }

    /// Point the current session at another provider/model, keeping full context.
    pub fn set_route(&mut self, provider_id: &str, model: &str) {
        let Some((new_short, default_model)) = self.provider(provider_id).map(|p| {
            (
                if p.short.is_empty() {
                    p.name.clone()
                } else {
                    p.short.clone()
                },
                p.model.clone(),
            )
        }) else {
            return;
        };
        let Some(s) = self.sessions.get_mut(self.active) else {
            return;
        };
        if s.provider == provider_id && s.model == model {
            self.picker = false;
            return;
        }
        let changed_provider = s.provider != provider_id;
        let old_label = self
            .cfg
            .providers
            .iter()
            .find(|x| x.id == s.provider)
            .map(|x| {
                if x.short.is_empty() {
                    x.name.clone()
                } else {
                    x.short.clone()
                }
            })
            .unwrap_or_default();
        let old_model = s.model.clone();
        let has_context = s.messages.iter().any(|m| m.role != Role::Handoff);

        s.provider = provider_id.to_string();
        s.model = if model.is_empty() {
            default_model
        } else {
            model.to_string()
        };
        s.updated = now();

        if has_context && (changed_provider || old_model != model) {
            let old = if old_model.is_empty() {
                old_label
            } else {
                format!("{old_label} · {old_model}")
            };
            let marker = format!(
                "{} → {} · {} · full context carried over",
                old, new_short, model
            );
            // Switching twice without a turn should move the marker, not stack them.
            match s.messages.last_mut() {
                Some(m) if m.role == Role::Handoff => m.text = marker,
                _ => s.messages.push(Message::handoff(&marker)),
            }
        }
        self.picker = false;
        self.persist();
    }

    pub fn persist(&self) {
        store::save_config(&self.cfg);
        store::save_sessions(&self.sessions);
    }

    pub fn send(&mut self, ctx: &egui::Context) {
        if self.streaming() {
            return;
        }
        let text = self.draft.trim().to_string();
        if text.is_empty() {
            return;
        }
        let Some(provider) = self.provider_of_active().cloned() else {
            self.toast = Some(("Connect a model first — open Models".into(), 4.5));
            self.view = View::Models;
            return;
        };
        self.draft.clear();

        let pid = provider.id.clone();
        let model = self.active_model();

        let session_idx = self.active;
        {
            let s = &mut self.sessions[session_idx];
            if s.provider != pid {
                s.provider = pid.clone();
            }
            if s.model.is_empty() {
                s.model = model.clone();
            }
            s.messages.push(Message::user(&text));
            s.messages.push(Message::assistant(&pid, &model));
            s.updated = now();
            if self.cfg.auto_title && (s.title == "New session" || s.title.is_empty()) {
                s.derive_title();
            }
        }

        let history: Vec<Message> = {
            let s = &self.sessions[session_idx];
            s.messages[..s.messages.len() - 1].to_vec()
        };

        let opts = Opts {
            system: self.cfg.system_prompt.clone(),
            stream: self.cfg.streaming,
            temperature: self.cfg.temperature,
            max_tokens: self.cfg.max_tokens,
            history_limit: self.cfg.history_limit,
            cli_path: self.cli_path(&provider),
            cli_cwd: cli::working_dir(&self.cfg.cli_dir),
            cli_full: self.cfg.cli_permissions == "full",
            cli_timeout: self.cfg.cli_timeout.max(30),
            opencode_path: self.clis.get("opencode").and_then(|d| d.path.clone()),
        };

        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        llm::spawn_chat(
            provider,
            model.clone(),
            history,
            opts,
            tx,
            cancel.clone(),
            ctx.clone(),
        );

        self.call = Some(Call {
            rx,
            cancel,
            session_id: self.sessions[session_idx].id.clone(),
            started: Instant::now(),
            chars: 0,
        });
        self.persist();
    }

    pub fn stop(&mut self) {
        if let Some(c) = &self.call {
            c.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn start_install(
        &mut self,
        spec: &'static cli::Spec,
        kind: cli::JobKind,
        ctx: &egui::Context,
    ) {
        if self.jobs.iter().any(|j| j.cli == spec.id && !j.done) {
            return;
        }
        self.jobs.retain(|j| j.cli != spec.id);
        let job = cli::spawn_install(spec, kind, ctx.clone());
        let label = match job.kind {
            cli::JobKind::Install => format!("Installing {}", spec.name),
            cli::JobKind::Update => format!("Updating {}", spec.name),
            cli::JobKind::Uninstall => format!("Removing {}", spec.name),
        };
        self.toast = Some((label, 3.0));
        self.jobs.push(job);
    }

    fn finish_codex_opencode_setup(&mut self, ctx: &egui::Context) {
        if let Some(spec) = cli::spec("opencode") {
            self.clis.insert("opencode".into(), cli::detect(spec));
        }
        if let Some(path) = self.clis.get("opencode").and_then(|d| d.path.clone()) {
            cli::open_opencode_codex_login(&path);
            self.codex_opencode_setup = false;
            self.toast = Some((
                "OpenCode is ready for Codex. Finish ChatGPT sign-in in the opened terminal, then refresh models.".into(),
                7.0,
            ));
        } else if let Some(spec) = cli::spec("opencode") {
            self.toast = Some(("Installing OpenCode to finish Codex setup…".into(), 5.0));
            self.start_install(spec, cli::JobKind::Install, ctx);
        }
    }

    pub fn redetect(&mut self, cli_id: &str, ctx: &egui::Context) {
        let Some(spec) = cli::spec(cli_id) else {
            return;
        };
        let Some(tx) = self.bg_tx.clone() else { return };
        let id = cli_id.to_string();
        self.clis.remove(cli_id);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let d = cli::detect(spec);
            let _ = tx.send(Bg::Detected(id, d));
            ctx.request_repaint();
        });
    }

    /// Refresh only the 9Router model list (and its reachability).
    pub fn refresh_router(&mut self, ctx: &egui::Context) {
        let Some(p) = self.cfg.providers.iter().find(|p| p.is_router()).cloned() else {
            return;
        };
        if let Some(tx) = self.disc_tx.clone() {
            llm::spawn_discovery(p, tx, ctx.clone());
            self.pending_discovery += 1;
        }
    }

    /// Refresh every provider's model list and every CLI detection.
    pub fn refresh_all(&mut self, ctx: &egui::Context) {
        for spec in cli::SPECS {
            self.redetect(spec.id, ctx);
        }
        let mut started = 0;
        for p in self.cfg.providers.iter().filter(|p| !p.is_cli()) {
            if let Some(tx) = self.disc_tx.clone() {
                llm::spawn_discovery(p.clone(), tx, ctx.clone());
                started += 1;
            }
        }
        self.pending_discovery += started;
        self.toast = Some(("Refreshing connections…".into(), 2.5));
    }

    fn pump_bg(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.bg_rx else { return };
        let mut got = Vec::new();
        while let Ok(d) = rx.try_recv() {
            got.push(d);
        }
        if got.is_empty() {
            return;
        }
        let mut dirty = false;
        for msg in got {
            match msg {
                Bg::Detected(id, d) => {
                    let ready = d.state == cli::State::Ready;
                    let version = d.version.clone();
                    if id == "opencode" && ready {
                        if let (Some(path), Some(tx)) = (d.path.clone(), self.bg_tx.clone()) {
                            let ctx = ctx.clone();
                            std::thread::spawn(move || {
                                let models = cli::list_opencode_models(&path);
                                let _ = tx.send(Bg::OpenCodeModels(models));
                                ctx.request_repaint();
                            });
                        }
                    }
                    if let Some(p) = self.cfg.providers.iter_mut().find(|p| p.cli == id) {
                        p.ready = ready;
                        p.version = version;
                        p.status = if ready { Some(true) } else { None };
                        if !ready && p.enabled {
                            // keep enabled; the UI shows "not installed"
                        }
                    }
                    if d.state != cli::State::Checking {
                        self.clis.insert(id, d);
                    }
                    dirty = true;
                }
                Bg::NodeStatus(ok, version) => {
                    self.node_ok = ok;
                    self.node_version = version;
                    dirty = true;
                }
                Bg::OpenCodeModels(models) => {
                    if let Some(p) = self.cfg.providers.iter_mut().find(|p| p.cli == "opencode") {
                        if !models.is_empty() {
                            let keep = p.model.clone();
                            p.models = models;
                            if !p.models.iter().any(|m| m == &keep) {
                                p.model = p.models[0].clone();
                            }
                        }
                    }
                    dirty = true;
                }
            }
        }
        if dirty {
            self.heal_sessions();
            store::save_config(&self.cfg);
            ctx.request_repaint();
        }
    }

    fn pump_discovery(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.disc_rx else { return };
        let mut got = Vec::new();
        while let Ok(d) = rx.try_recv() {
            got.push(d);
        }
        if got.is_empty() {
            return;
        }
        for d in got {
            self.pending_discovery = self.pending_discovery.saturating_sub(1);
            if let Some(p) = self.cfg.providers.iter_mut().find(|p| p.id == d.provider) {
                p.latency = Some(d.latency);
                p.status = Some(d.error.is_none());
                if let Some(err) = &d.error {
                    store::log(&format!("discovery {} failed: {err}", d.provider));
                }
                if !d.models.is_empty() {
                    let keep = p.model.clone();
                    p.models = d.models;
                    if !p.models.iter().any(|m| m == &keep) {
                        if let Some(first) = p.models.first().cloned() {
                            p.model = first;
                        }
                    }
                }
            }
        }
        store::save_config(&self.cfg);
        ctx.request_repaint();
    }

    fn pump_jobs(&mut self, ctx: &egui::Context) {
        let mut finished: Vec<(String, bool)> = Vec::new();
        for job in self.jobs.iter_mut() {
            let mut done: Option<bool> = None;
            if let Some(rx) = &job.rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        cli::JobMsg::Line(l) => {
                            if job.lines.len() >= 300 {
                                job.lines.pop_front();
                            }
                            if !l.trim().is_empty() {
                                job.lines.push_back(l);
                            }
                        }
                        cli::JobMsg::Done(ok) => done = Some(ok),
                    }
                }
            }
            if let Some(ok) = done {
                job.done = true;
                job.ok = ok;
                job.rx = None;
                finished.push((job.cli.clone(), ok));
            }
        }
        if !finished.is_empty() {
            for (id, ok) in &finished {
                self.redetect(id, ctx);
                if id == "codex" && *ok {
                    self.codex_opencode_setup = true;
                    self.finish_codex_opencode_setup(ctx);
                } else if id == "opencode" && *ok && self.codex_opencode_setup {
                    self.finish_codex_opencode_setup(ctx);
                }
                let name = cli::spec(id)
                    .map(|s| s.name.to_string())
                    .unwrap_or_else(|| id.clone());
                self.toast = Some((
                    if *ok {
                        format!("{name} is ready")
                    } else {
                        format!("{name}: install failed — see the log")
                    },
                    4.5,
                ));
            }
            self.persist();
        }
        for j in self.jobs.iter().filter(|j| !j.done) {
            let _ = j;
            ctx.request_repaint_after(std::time::Duration::from_millis(180));
        }
    }

    fn pump(&mut self, ctx: &egui::Context) {
        let Some(call) = &self.call else { return };
        let sid = call.session_id.clone();
        let Some(idx) = self.sessions.iter().position(|s| s.id == sid) else {
            self.call = None;
            return;
        };
        let mut events = Vec::new();
        while let Ok(ev) = call.rx.try_recv() {
            events.push(ev);
        }
        if events.is_empty() {
            return;
        }

        let mut finished = false;
        for ev in events {
            match ev {
                Ev::Delta(d) => {
                    if let Some(c) = &mut self.call {
                        c.chars += d.chars().count();
                    }
                    if let Some(s) = self.sessions.get_mut(idx) {
                        if let Some(m) = s.messages.last_mut() {
                            m.text.push_str(&d);
                        }
                    }
                }
                Ev::Done {
                    tokens_in,
                    tokens_out,
                    model,
                } => {
                    if let Some(s) = self.sessions.get_mut(idx) {
                        if let Some(m) = s.messages.last_mut() {
                            if m.text.trim().is_empty() {
                                m.text = "_No content returned by this model._".to_string();
                            }
                            m.tokens = tokens_out;
                            if !model.is_empty() {
                                m.model = model;
                            }
                            m.ts = now();
                        }
                        s.tokens_in += tokens_in;
                        s.tokens_out += tokens_out;
                        s.updated = now();
                    }
                    finished = true;
                }
                Ev::Failed(err) => {
                    if let Some(s) = self.sessions.get_mut(idx) {
                        if let Some(m) = s.messages.last_mut() {
                            if m.text.is_empty() {
                                m.text = format!("**Request failed**\n\n```\n{err}\n```");
                            } else {
                                m.text.push_str(&format!("\n\n```\n{err}\n```"));
                            }
                            m.error = true;
                        }
                    }
                    self.toast = Some((err.lines().next().unwrap_or("error").to_string(), 6.0));
                    finished = true;
                }
            }
        }

        if finished {
            self.call = None;
            self.persist();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let (new_chat, settings, escape, slash, find, chat, models, gw) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::N),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Slash),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::K),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num1),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num2),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num3),
            )
        });
        if chat {
            self.view = View::Chat;
        }
        if new_chat {
            self.new_chat();
        }
        if settings {
            self.view = View::Settings;
        }
        if find {
            self.picker = !self.picker;
            self.picker_focus = false;
            self.picker_query.clear();
            let screen = ctx.screen_rect();
            self.picker_anchor = Some(egui::Rect::from_min_size(
                egui::pos2(screen.center().x - 60.0, screen.bottom() - 96.0),
                egui::vec2(120.0, 2.0),
            ));
        }
        if models {
            self.view = View::Models;
        }
        if gw {
            self.view = View::Models;
            self.models_tab = 1;
        }
        if escape {
            if self.streaming() {
                self.stop();
            } else if self.picker {
                self.picker = false;
            } else if self.renaming.is_some() {
                self.renaming = None;
            } else if self.deleting.is_some() {
                self.deleting = None;
            }
        }
        if slash && !self.booted {
            self.boot_skip = true;
        }
    }

    fn tick_boot(&mut self, ctx: &egui::Context) {
        let dt = ctx.input(|i| i.stable_dt).min(0.08);
        self.boot_t += if self.boot_skip { dt * 6.0 } else { dt };
        self.boot_stage = if self.boot_t < 0.15 {
            0
        } else if self.boot_t < 0.30 {
            1
        } else if self.boot_t < 0.45 {
            2
        } else {
            3
        };
        if self.boot_t > 0.60 {
            self.booted = true;
            self.toast = None;
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump(ctx);
        self.pump_bg(ctx);
        self.pump_discovery(ctx);
        self.pump_jobs(ctx);
        self.pump_updates(ctx);
        self.handle_keys(ctx);

        self.accent = theme::accent_color(&self.cfg.accent);
        theme::set_accent(self.accent);
        if (self.cfg.font_scale - self.last_scale).abs() > 0.001 {
            theme::apply(ctx, self.cfg.font_scale, self.cfg.compact);
            self.last_scale = self.cfg.font_scale;
        }

        if let Some((_, ttl)) = self.toast.clone() {
            if ttl <= 0.0 {
                self.toast = None;
            }
        }
        if let Some((_, ttl)) = &mut self.toast {
            *ttl -= ctx.input(|i| i.stable_dt) as f64;
        }

        if !self.booted {
            self.tick_boot(ctx);
            ui::splash::show(self, ctx);
            return;
        }

        ui::layout(self, ctx);
        self.opencode_download_prompt(ctx);
        self.update_modal(ctx);

        if let Some((msg, _)) = self.toast.clone() {
            let screen = ctx.screen_rect();
            egui::Area::new(egui::Id::new("toast"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(screen.right() - 24.0, screen.bottom() - 132.0))
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(theme::SURFACE_HI)
                        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(egui::Margin::symmetric(12, 7))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 8],
                            blur: 24,
                            spread: 0,
                            color: egui::Color32::from_black_alpha(140),
                        })
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(msg)
                                    .font(theme::font(12.5))
                                    .color(theme::TEXT),
                            );
                        });
                });
        }
    }
}

impl App {
    /// Check the public release manifest in the background.
    pub fn check_updates(&mut self, ctx: &egui::Context) {
        if self.update_rx.is_some() || self.upd_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_rx = Some(rx);
        self.update_status = "Checking for updates…".into();
        self.upd_error = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::updater::check());
            ctx.request_repaint();
        });
    }

    /// Start the download → install → restart flow for a pending release.
    pub fn begin_update(&mut self, ctx: &egui::Context) {
        let Some(release) = self.pending_release.clone() else {
            return;
        };
        let current = std::env::current_exe().unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel();
        self.upd_rx = Some(rx);
        self.upd_progress = Some((0, None));
        self.upd_error = None;
        self.upd_stage = format!("Downloading ROXY {}…", release.version);
        crate::updater::install(release, current, tx);
        ctx.request_repaint();
    }

    fn pump_updates(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.update_rx {
            if let Ok(res) = rx.try_recv() {
                self.update_rx = None;
                match res {
                    Ok(release) => {
                        if release.version == env!("CARGO_PKG_VERSION") {
                            self.update_status = format!("You’re up to date ({})", release.version);
                        } else {
                            self.update_status =
                                format!("Version {} is available", release.version);
                            self.pending_release = Some(release);
                        }
                    }
                    Err(e) => self.update_status = format!("Update check failed: {e}"),
                }
            }
        }

        if let Some(rx) = &self.upd_rx {
            let mut finished = false;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    crate::updater::Upd::Downloading(bytes, total) => {
                        self.upd_progress = Some((bytes, total));
                    }
                    crate::updater::Upd::Installing => {
                        self.upd_stage = "Installing — ROXY will restart…".into();
                    }
                    crate::updater::Upd::Failed(e) => {
                        self.upd_error = Some(e);
                        finished = true;
                    }
                }
            }
            if finished {
                self.upd_rx = None;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn update_modal(&mut self, ctx: &egui::Context) {
        let offering =
            self.pending_release.is_some() && self.upd_rx.is_none() && self.upd_error.is_none();
        let running = self.upd_rx.is_some() || self.upd_error.is_some();
        if !offering && !running {
            return;
        }

        let accent = self.accent;
        let version = self
            .pending_release
            .as_ref()
            .map(|r| r.version.clone())
            .unwrap_or_default();
        let mut download = false;
        let mut later = false;

        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("update_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::SURFACE_HI)
                    .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::symmetric(26, 22))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 14],
                        blur: 36,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(170),
                    })
                    .show(ui, |ui| {
                        ui.set_width(420.0);
                        ui.label(
                            egui::RichText::new(if offering {
                                format!("ROXY {version} is ready")
                            } else {
                                "Updating ROXY".to_string()
                            })
                            .font(theme::font_semi(19.0))
                            .color(theme::TEXT),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(if offering {
                                "Download and install the new build in place. Your sessions and settings stay where they are."
                            } else {
                                self.upd_stage.as_str()
                            })
                            .font(theme::font(13.0))
                            .color(theme::TEXT_DIM),
                        );
                        ui.add_space(16.0);

                        if offering {
                            ui.horizontal(|ui| {
                                if ui
                                    .button(
                                        egui::RichText::new("Download & install")
                                            .color(egui::Color32::BLACK)
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    download = true;
                                }
                                if ui.button("Later").clicked() {
                                    later = true;
                                }
                            });
                        } else {
                            // Progress bar
                            let (bytes, total) = self.upd_progress.unwrap_or((0, None));
                            let frac = total
                                .filter(|t| *t > 0)
                                .map(|t| (bytes as f32 / t as f32).clamp(0.0, 1.0))
                                .unwrap_or(0.0);
                            let w = ui.available_width();
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(w, 6.0), egui::Sense::hover());
                            let p = ui.painter();
                            p.rect_filled(rect, egui::CornerRadius::same(3), theme::BORDER_SOFT);
                            let fill = egui::Rect::from_min_size(
                                rect.min,
                                egui::vec2((w * frac).max(3.0), 6.0),
                            );
                            p.rect_filled(fill, egui::CornerRadius::same(3), accent);
                            ui.add_space(8.0);
                            let detail = if let Some(e) = &self.upd_error {
                                e.clone()
                            } else if let Some(t) = total.filter(|t| *t > 0) {
                                format!("{:.1} / {:.1} MiB", bytes as f32 / 1048576.0, t as f32 / 1048576.0)
                            } else {
                                format!("{:.1} MiB", bytes as f32 / 1048576.0)
                            };
                            ui.label(
                                egui::RichText::new(detail)
                                    .font(theme::mono(11.0))
                                    .color(if self.upd_error.is_some() {
                                        theme::DANGER
                                    } else {
                                        theme::TEXT_FAINT
                                    }),
                            );
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                if self.upd_error.is_some() {
                                    if ui.button("Retry").clicked() {
                                        download = true;
                                    }
                                    if ui.button("Close").clicked() {
                                        later = true;
                                    }
                                } else {
                                    ui.label(
                                        egui::RichText::new("Keep ROXY open until the restart.")
                                            .font(theme::font(11.0))
                                            .color(theme::TEXT_FAINT),
                                    );
                                }
                            });
                        }
                    });
            });

        if download {
            self.upd_error = None;
            self.begin_update(ctx);
        }
        if later {
            self.pending_release = None;
            self.upd_error = None;
            self.upd_rx = None;
        }
    }

    fn opencode_download_prompt(&mut self, ctx: &egui::Context) {
        if self.opencode_prompt_dismissed
            || self
                .clis
                .get("opencode")
                .map(|d| d.state != cli::State::Missing)
                .unwrap_or(true)
        {
            return;
        }

        let mut install = false;
        let mut later = false;
        let mut get_node = false;
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("opencode_download_prompt"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::SURFACE_HI)
                    .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::symmetric(24, 20))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 12], blur: 32, spread: 0,
                        color: egui::Color32::from_black_alpha(160),
                    })
                    .show(ui, |ui| {
                        ui.set_width(390.0);
                        ui.label(egui::RichText::new("Download OpenCode?")
                            .font(theme::font(19.0)).color(theme::TEXT));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(
                            "OpenCode is not installed yet. ROXY can download and install it for you."
                        ).font(theme::font(13.0)).color(theme::TEXT_DIM));
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if self.node_ok {
                                install = ui.button(egui::RichText::new("Download OpenCode")
                                    .color(egui::Color32::WHITE)
                                    .strong()).clicked();
                            } else {
                                get_node = ui.button(egui::RichText::new("Get Node.js")
                                    .color(egui::Color32::WHITE)
                                    .strong()).clicked();
                            }
                            if ui.button("Not now").clicked() { later = true; }
                        });
                        if !self.node_ok {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("OpenCode installation needs Node.js. ROXY will open the Node.js download page, then you can install OpenCode from Models → CLI agents.")
                                .font(theme::font(11.5)).color(theme::TEXT_FAINT));
                        }
                    });
            });

        if install {
            if let Some(spec) = cli::spec("opencode") {
                self.start_install(spec, cli::JobKind::Install, ctx);
            }
            self.opencode_prompt_dismissed = true;
        }
        if get_node {
            cli::open_url("https://nodejs.org/en/download");
            self.view = View::Models;
            self.models_tab = 0;
            self.open_cli = Some("opencode".into());
            self.opencode_prompt_dismissed = true;
        }
        if later {
            self.opencode_prompt_dismissed = true;
        }
    }
}
