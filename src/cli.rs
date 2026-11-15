//! Local agent CLIs: catalogue, detection, installation and headless runs.
//!
//! ROXY never ships credentials or a server address. Every CLI lives on the
//! user's own machine, is installed by ROXY on request, and is driven as a
//! child process: the whole transcript goes in on stdin, the reply is read
//! back from stdout.

use crate::model::{approx_tokens, Provider};
use crate::store;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Parse {
    Text,
    /// Codex `exec --json`: NDJSON `item.completed` / `turn.completed`.
    Codex,
    /// opencode `run --format json`: NDJSON `text` / `step_finish`.
    Opencode,
}

pub struct Spec {
    pub id: &'static str,
    pub name: &'static str,
    pub vendor: &'static str,
    pub short: &'static str,
    pub bin: &'static str,
    /// npm package, empty when the CLI is not installable through npm.
    pub pkg: &'static str,
    pub color: &'static str,
    pub blurb: &'static str,
    pub docs: &'static str,
    /// Command that starts the interactive sign-in flow.
    pub login: &'static str,
    pub models: &'static [&'static str],
    pub parse: Parse,
    /// A local server (not a chat CLI). Installed and started, but never
    /// offered to the picker as an agent on its own.
    pub server: bool,
}

pub const SPECS: &[Spec] = &[
    Spec {
        id: "codex",
        name: "Codex CLI",
        vendor: "OpenAI",
        short: "CDX",
        bin: "codex",
        pkg: "@openai/codex",
        color: "green",
        blurb: "OpenAI's terminal agent. Streams structured events, edits files in your working folder.",
        docs: "https://github.com/openai/codex",
        login: "codex login",
        models: &["gpt-5.1-codex", "gpt-5.1", "gpt-5.1-codex-mini"],
        parse: Parse::Codex,
        server: false,
    },
    Spec {
        id: "claude",
        name: "Claude Code",
        vendor: "Anthropic",
        short: "CLD",
        bin: "claude",
        pkg: "@anthropic-ai/claude-code",
        color: "orange",
        blurb: "Anthropic's coding agent. Strong at long files, refactors and careful edits.",
        docs: "https://docs.claude.com/en/docs/claude-code",
        login: "claude",
        models: &["sonnet", "opus", "haiku"],
        parse: Parse::Text,
        server: false,
    },
    Spec {
        id: "gemini",
        name: "Gemini CLI",
        vendor: "Google",
        short: "GEM",
        bin: "gemini",
        pkg: "@google/gemini-cli",
        color: "blue",
        blurb: "Google's agent with a generous free tier and a million-token window.",
        docs: "https://github.com/google-gemini/gemini-cli",
        login: "gemini",
        models: &["gemini-2.5-pro", "gemini-2.5-flash"],
        parse: Parse::Text,
        server: false,
    },
    Spec {
        id: "opencode",
        name: "OpenCode",
        vendor: "SST",
        short: "OC",
        bin: "opencode",
        pkg: "@opencode/cli",
        color: "violet",
        blurb: "Provider-agnostic TUI agent with fine-grained model routing.",
        docs: "https://opencode.ai",
        login: "opencode auth login",
        models: &[],
        parse: Parse::Opencode,
        server: false,
    },
    Spec {
        id: "qwen",
        name: "Qwen Code",
        vendor: "Alibaba",
        short: "QWN",
        bin: "qwen",
        pkg: "@qwen-code/qwen-code",
        color: "pink",
        blurb: "Qwen Coder fork of Gemini CLI. Long context, free daily quota.",
        docs: "https://github.com/QwenLM/qwen-code",
        login: "qwen",
        models: &["qwen3-coder-plus", "qwen3-coder-flash"],
        parse: Parse::Text,
        server: false,
    },
    Spec {
        id: "copilot",
        name: "Copilot CLI",
        vendor: "GitHub",
        short: "GH",
        bin: "copilot",
        pkg: "@github/copilot",
        color: "cyan",
        blurb: "GitHub's agent, wired to your existing Copilot subscription.",
        docs: "https://github.com/github/copilot-cli",
        login: "copilot",
        models: &["gpt-5", "claude-sonnet-4.5"],
        parse: Parse::Text,
        server: false,
    },
    Spec {
        id: "amp",
        name: "Amp",
        vendor: "Sourcegraph",
        short: "AMP",
        bin: "amp",
        pkg: "@sourcegraph/amp",
        color: "red",
        blurb: "Sourcegraph's high-throughput agent. Good for wide, parallel edits.",
        docs: "https://ampcode.com",
        login: "amp login",
        models: &[],
        parse: Parse::Text,
        server: false,
    },
    Spec {
        id: "crush",
        name: "Crush",
        vendor: "Charm",
        short: "CRS",
        bin: "crush",
        pkg: "@charmland/crush",
        color: "teal",
        blurb: "Charm's terminal agent. Bring any provider key you already have.",
        docs: "https://github.com/charmbracelet/crush",
        login: "crush",
        models: &[],
        parse: Parse::Text,
        server: false,
    },
    Spec {
        id: "grok",
        name: "Grok CLI",
        vendor: "xAI",
        short: "GRK",
        bin: "grok",
        pkg: "@vibe-kit/grok-cli",
        color: "amber",
        blurb: "A lean Grok agent. Needs a GROK_API_KEY or an interactive login.",
        docs: "https://github.com/superagent-ai/grok-cli",
        login: "grok",
        models: &["grok-code-fast-1", "grok-4"],
        parse: Parse::Text,
        server: false,
    },
    Spec {
        id: "9router",
        name: "9Router",
        vendor: "9Router",
        short: "9R",
        bin: "9router",
        pkg: "9router",
        color: "violet",
        blurb: "Local model router. Ranks models by your priority and falls through automatically when one is rate-limited or down.",
        docs: "https://github.com/decolua/9router",
        login: "9router",
        models: &[],
        parse: Parse::Text,
        server: true,
    },
];

pub fn spec(id: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.id == id)
}

/// Address the local 9Router server listens on.
pub const ROUTER_ID: &str = "9router";
pub const ROUTER_URL: &str = "http://localhost:20128/v1";
pub const ROUTER_DASHBOARD: &str = "http://localhost:20128";

/// The 9Router route: an OpenAI-compatible endpoint served from this machine.
pub fn router_provider() -> Provider {
    let mut p = Provider::new(ROUTER_ID, "9Router", "9R", ROUTER_URL, "", "violet");
    p.catalog = ROUTER_ID.to_string();
    p.note = "Local router · priority fallback across providers".to_string();
    p.ready = true;
    p
}

/// Providers created for every known CLI on a fresh install. They stay inert
/// until the binary is detected, so the picker never offers a missing tool.
pub fn default_providers() -> Vec<Provider> {
    let mut out: Vec<Provider> = SPECS
        .iter()
        .filter(|s| !s.server)
        .map(|s| {
            let mut p = Provider::cli(
                s.id,
                s.name,
                s.short,
                s.id,
                s.models.first().copied().unwrap_or(""),
                s.color,
            );
            p.models = s.models.iter().map(|m| m.to_string()).collect();
            p.enabled = true;
            p
        })
        .collect();
    out.push(router_provider());
    out
}

// ---------------------------------------------------------------- detection

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Checking,
    Ready,
    Missing,
}

#[derive(Clone, Debug)]
pub struct Detect {
    pub state: State,
    pub path: Option<PathBuf>,
    pub version: String,
    pub managed: bool,
}

impl Detect {
    pub fn missing() -> Self {
        Self {
            state: State::Missing,
            path: None,
            version: String::new(),
            managed: false,
        }
    }
}

pub fn tools_dir() -> PathBuf {
    store::dir().join("tools")
}

fn exe_names(bin: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![
            format!("{bin}.exe"),
            format!("{bin}.cmd"),
            format!("{bin}.bat"),
            bin.to_string(),
        ]
    } else {
        vec![bin.to_string()]
    }
}

fn is_file(p: &Path) -> bool {
    p.is_file()
}

/// Directories worth searching in addition to PATH (GUI processes often start
/// with a thinner PATH than a terminal).
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            dirs.push(PathBuf::from(&appdata).join("npm"));
        }
        dirs.push(PathBuf::from(r"C:\Program Files\nodejs"));
        if let Ok(profile) = std::env::var("USERPROFILE") {
            dirs.push(PathBuf::from(&profile).join(".local").join("bin"));
        }
    } else {
        for p in [
            "/usr/local/bin",
            "/opt/homebrew/bin",
            "/usr/bin",
            "/snap/bin",
        ] {
            dirs.push(PathBuf::from(p));
        }
        if let Ok(home) = std::env::var("HOME") {
            let h = PathBuf::from(&home);
            dirs.push(h.join(".local").join("bin"));
            dirs.push(h.join(".npm-global").join("bin"));
            dirs.push(h.join(".bun").join("bin"));
            dirs.push(h.join(".cargo").join("bin"));
        }
    }
    dirs
}

/// Find an executable by name in ROXY's tool dir, then PATH.
pub fn find_bin(bin: &str) -> Option<PathBuf> {
    let names = exe_names(bin);
    let managed = tools_dir().join("node_modules").join(".bin");
    for n in &names {
        let p = managed.join(n);
        if is_file(&p) {
            return Some(p);
        }
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(path) = std::env::var("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.extend(search_dirs());
    for d in dirs {
        for n in &names {
            let p = d.join(n);
            if is_file(&p) {
                return Some(p);
            }
        }
    }
    None
}

fn run_version(path: &Path, timeout: Duration) -> String {
    let mut cmd = base_command(path);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else {
        return String::new();
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if started.elapsed() > timeout {
                    kill(&mut child);
                    return String::new();
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(_) => return String::new(),
        }
    }
    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let mut buf = [0u8; 256];
        if let Ok(n) = stdout.read(&mut buf) {
            out = String::from_utf8_lossy(&buf[..n]).to_string();
        }
    }
    let line = out.lines().next().unwrap_or("").trim().to_string();
    let line = line.trim_start_matches('v').to_string();
    // "codex-cli 0.167.1" → "0.167.1"
    let line = line
        .split_whitespace()
        .last()
        .unwrap_or(&line)
        .trim_start_matches('v')
        .to_string();
    line.chars().take(20).collect()
}

pub fn detect(spec: &Spec) -> Detect {
    match find_bin(spec.bin) {
        Some(path) => {
            let managed = path.starts_with(tools_dir());
            let version = run_version(&path, Duration::from_secs(6));
            Detect {
                state: State::Ready,
                path: Some(path),
                version,
                managed,
            }
        }
        None => Detect::missing(),
    }
}

/// Ask the installed OpenCode CLI for its current model catalogue. OpenCode
/// scopes this list to its saved credentials and configuration, so the picker
/// reflects the models this installation can actually use.
pub fn list_opencode_models(path: &Path) -> Vec<String> {
    let mut cmd = base_command(path);
    cmd.arg("models")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(output) = cmd.output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = strip_ansi(&String::from_utf8_lossy(&output.stdout));
    let mut models = Vec::new();
    for line in text.lines() {
        // `opencode models` prints selectable provider/model IDs. Ignore
        // headings and display-name columns, while allowing model IDs that
        // contain additional slashes (for example provider/model-family/id).
        for token in line.split_whitespace() {
            let token =
                token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && !"_./-:#".contains(c));
            if token.contains('/')
                && !token.contains("://")
                && token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_./-:#".contains(c))
                && !models.iter().any(|m| m == token)
            {
                models.push(token.to_string());
            }
        }
    }
    models
}

pub fn npm() -> Option<PathBuf> {
    find_bin("npm")
}

// ---------------------------------------------------------------- launching

#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// The node entry point behind an npm shim, so Windows never needs `cmd.exe`
/// and multi-line prompts stay intact as real arguments.
#[cfg(windows)]
fn node_entry(shim: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(shim).ok()?;
    // Parse `"%~dp0\node_modules\pkg\bin\cli.js" %*` style shims.
    for token in text.split(|c: char| c == '"' || c == '\'' || c.is_whitespace()) {
        if token.ends_with(".js") {
            let name = token.rsplit(['\\', '/']).next()?.to_string();
            let dir = shim.parent()?;
            let candidate = dir.join(name);
            if is_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Best-effort: turn a resolved binary into a directly spawnable command.
fn base_command(path: &Path) -> Command {
    #[cfg(windows)]
    {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if ext == "js" {
            let mut c = Command::new("node");
            c.arg(path);
            hide_console(&mut c);
            return c;
        }
        if ext == "cmd" || ext == "bat" {
            // npm shims: run the JS entry with node when possible (no cmd.exe,
            // no console flash, newlines allowed in arguments).
            if let Some(js) = node_entry(path) {
                let mut c = Command::new("node");
                c.arg(js);
                hide_console(&mut c);
                return c;
            }
            let mut c = Command::new("cmd");
            c.arg("/C").arg(path);
            hide_console(&mut c);
            return c;
        }
        let mut c = Command::new(path);
        hide_console(&mut c);
        return c;
    }
    #[allow(unreachable_code)]
    {
        Command::new(path)
    }
}

fn kill(child: &mut Child) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// Build the headless invocation for one CLI. Prompt is piped on stdin unless
/// the CLI only accepts it as an argument.
fn invocation(
    spec: Option<&Spec>,
    custom: &str,
    model: &str,
    full: bool,
    prompt: &str,
) -> (Vec<String>, bool) {
    let m = model.trim();
    let use_model = !m.is_empty() && m != "auto";
    if let Some(s) = spec {
        let mut a: Vec<String> = Vec::new();
        match s.id {
            "codex" => {
                a.extend([
                    "exec".into(),
                    "--json".into(),
                    "--skip-git-repo-check".into(),
                ]);
                if full {
                    a.push("--dangerously-bypass-approvals-and-sandbox".into());
                } else {
                    a.extend(["--sandbox".into(), "read-only".into()]);
                }
                if use_model {
                    a.extend(["-m".into(), m.to_string()]);
                }
                a.push("-".into());
            }
            "claude" => {
                a.extend(["-p".into(), "--output-format".into(), "text".into()]);
                if use_model {
                    a.extend(["--model".into(), m.to_string()]);
                }
                if full {
                    a.push("--dangerously-skip-permissions".into());
                }
            }
            "gemini" => {
                if use_model {
                    a.extend(["-m".into(), m.to_string()]);
                }
                if full {
                    a.extend(["--approval-mode".into(), "yolo".into()]);
                }
            }
            "qwen" => {
                if use_model {
                    a.extend(["-m".into(), m.to_string()]);
                }
                if full {
                    a.push("--yolo".into());
                }
            }
            "opencode" => {
                a.extend(["run".into(), "--format".into(), "json".into()]);
                if use_model {
                    a.extend(["--model".into(), m.to_string()]);
                }
                if full {
                    a.push("--auto".into());
                }
            }
            "copilot" => {
                a.extend(["-s".into(), "--no-color".into()]);
                if use_model {
                    a.extend(["--model".into(), m.to_string()]);
                }
                if full {
                    a.push("--allow-all-tools".into());
                }
            }
            "amp" => {
                a.push("-x".into());
                if full {
                    a.push("--dangerously-allow-all".into());
                }
            }
            "crush" => {
                a.extend(["run".into(), "--quiet".into()]);
                if use_model {
                    a.extend(["-m".into(), m.to_string()]);
                }
            }
            "grok" => {
                // grok reads the prompt from an argument, not stdin.
                a.extend(["-p".into(), prompt.to_string()]);
                if use_model {
                    a.extend(["-m".into(), m.to_string()]);
                }
                return (a, false);
            }
            _ => {}
        }
        return (a, true);
    }

    // Custom command template, e.g. `agy -p {prompt} --model {model}`.
    let mut args: Vec<String> = Vec::new();
    let mut stdin = true;
    for tok in custom.split_whitespace() {
        if tok.contains("{prompt}") {
            args.push(tok.replace("{prompt}", prompt));
            stdin = false;
        } else if tok.contains("{model}") {
            args.push(tok.replace("{model}", m));
        } else {
            args.push(tok.to_string());
        }
    }
    if !custom.contains("{prompt}") {
        stdin = true;
    }
    (args, stdin)
}

// ---------------------------------------------------------------- rendering

/// Flatten the shared transcript into one self-contained prompt.
///
/// The final user message is repeated as an explicit request at the end: agent
/// CLIs are trained to act on instructions, so a bare "### Assistant" marker at
/// the bottom is easy for them to read as unfinished context instead.
pub fn build_prompt(messages: &[(bool, String)], system: &str) -> String {
    let mut out = String::new();
    if !system.trim().is_empty() {
        out.push_str(system.trim());
        out.push_str("\n\n");
    }
    if messages.len() <= 1 {
        if let Some((_, only)) = messages.first() {
            out.push_str(only);
        }
        return out;
    }

    out.push_str(
        "You are continuing a conversation that started in another app. The transcript below is \
         the shared history. Write the assistant's next message, in markdown, continuing naturally: \
         no preamble, no mention of being a CLI, and no repetition of earlier turns.\n\n",
    );

    let (last_user, earlier) = match messages.split_last() {
        Some(((true, text), rest)) => (Some(text.as_str()), rest),
        Some((_, rest)) => (None, rest),
        None => (None, messages),
    };

    for (is_user, text) in earlier {
        out.push_str(if *is_user {
            "### User\n"
        } else {
            "### Assistant\n"
        });
        out.push_str(text.trim());
        out.push_str("\n\n");
    }

    if let Some(text) = last_user {
        out.push_str("### User\n");
        out.push_str(text.trim());
        out.push_str("\n\nReply now as the assistant. Output only the message itself:\n");
    }
    out
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\u{7}' {
                        break;
                    }
                }
            }
            _ => {
                chars.next();
            }
        }
    }
    out
}

// ---------------------------------------------------------------- chat run

pub enum Ev {
    Delta(String),
    Done {
        tokens_in: u64,
        tokens_out: u64,
        model: String,
    },
    Failed(String),
}

/// Spawn one headless CLI turn. `path` is the resolved binary.
#[allow(clippy::too_many_arguments)]
pub fn spawn_chat(
    path: PathBuf,
    spec_id: String,
    custom: String,
    model: String,
    prompt: String,
    cwd: PathBuf,
    full: bool,
    timeout: u64,
    history_chars: usize,
    tx: Sender<Ev>,
    cancel: Arc<AtomicBool>,
    ctx: egui::Context,
    extra_env: Vec<(String, String)>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let spec = spec(
            custom
                .is_empty()
                .then_some(spec_id.as_str())
                .unwrap_or(&spec_id),
        );
        let (args, use_stdin) = invocation(spec, &custom, &model, full, &prompt);
        let parse = spec.map(|s| s.parse).unwrap_or(Parse::Text);

        let mut cmd = base_command(&path);
        cmd.args(&args)
            .current_dir(&cwd)
            .stdin(if use_stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NO_COLOR", "1")
            .env("TERM", "dumb")
            .env("CLICOLOR", "0");
        for (key, value) in extra_env {
            cmd.env(key, value);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Ev::Failed(format!(
                    "Could not start {}: {e}\n{}",
                    path.display(),
                    sign_in_hint(spec)
                )));
                ctx.request_repaint();
                return;
            }
        };

        if use_stdin {
            if let Some(mut stdin) = child.stdin.take() {
                let body = prompt.clone();
                std::thread::spawn(move || {
                    let _ = stdin.write_all(body.as_bytes());
                });
            }
        }

        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let errs = stderr_buf.clone();
        if let Some(err) = child.stderr.take() {
            std::thread::spawn(move || {
                let mut buf = String::new();
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    if buf.len() < 3000 {
                        buf.push_str(&line);
                        buf.push('\n');
                    }
                }
                if let Ok(mut guard) = errs.lock() {
                    *guard = buf;
                }
            });
        }

        // Cancellation + timeout watchdog.
        let shared = Arc::new(Mutex::new(child));
        {
            let shared = shared.clone();
            let cancel = cancel.clone();
            let started = Instant::now();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(120));
                let mut guard = match shared.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                match guard.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) => {
                        if cancel.load(Ordering::Relaxed) || started.elapsed().as_secs() > timeout {
                            kill(&mut guard);
                            return;
                        }
                    }
                    Err(_) => return,
                }
            });
        }

        let stdout = shared.lock().ok().and_then(|mut c| c.stdout.take());
        let Some(stdout) = stdout else {
            let _ = tx.send(Ev::Failed("No output stream from the CLI".into()));
            ctx.request_repaint();
            return;
        };

        let mut acc = String::new();
        let mut tokens_out = 0u64;
        let mut pending = String::new();
        let mut reader = BufReader::new(stdout);
        let mut buf = [0u8; 8192];

        let emit = |text: String, acc: &mut String| {
            if text.is_empty() {
                return;
            }
            acc.push_str(&text);
            let _ = tx.send(Ev::Delta(text));
            ctx.request_repaint();
        };

        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                    match parse {
                        Parse::Text => {
                            let clean = strip_ansi(&chunk);
                            emit(clean, &mut acc);
                        }
                        Parse::Codex | Parse::Opencode => {
                            pending.push_str(&chunk);
                            while let Some(pos) = pending.find('\n') {
                                let line = pending[..pos].trim().to_string();
                                pending.drain(..=pos);
                                let (text, usage) = parse_line(parse, &line);
                                if let Some(t) = text {
                                    emit(t, &mut acc);
                                }
                                if let Some(o) = usage {
                                    tokens_out = o;
                                }
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // Flush a trailing partial line.
        if matches!(parse, Parse::Codex | Parse::Opencode) && !pending.trim().is_empty() {
            let (text, _) = parse_line(parse, pending.trim());
            if let Some(t) = text {
                emit(t, &mut acc);
            }
        }

        let status = shared
            .lock()
            .ok()
            .and_then(|mut c| c.wait().ok())
            .map(|s| s.code().unwrap_or(-1))
            .unwrap_or(-1);

        let stderr = stderr_buf.lock().map(|g| g.clone()).unwrap_or_default();
        let stopped = cancel.load(Ordering::Relaxed);

        if acc.trim().is_empty() && !stopped && status != 0 {
            let clean = strip_ansi(stderr.trim());
            let detail: String = clean.chars().take(600).collect();
            let reason = if detail.is_empty() {
                format!("exited with code {status}")
            } else {
                detail
            };
            let _ = tx.send(Ev::Failed(format!("{reason}\n{}", sign_in_hint(spec))));
            ctx.request_repaint();
            return;
        }
        if acc.trim().is_empty() && !stopped {
            let clean = strip_ansi(stderr.trim());
            let hint = if clean.is_empty() {
                "The CLI returned no output. It may need a sign-in.".to_string()
            } else {
                clean.chars().take(600).collect::<String>()
            };
            let _ = tx.send(Ev::Failed(hint));
            ctx.request_repaint();
            return;
        }

        let tokens_in = approx_tokens(&prompt) + (history_chars as u64 / 4);
        let out_tokens = if tokens_out > 0 {
            tokens_out
        } else {
            approx_tokens(&acc)
        };
        if stopped {
            acc.push_str("\n\n_stopped_");
        }
        crate::store::log(&format!(
            "cli {} {} · {} chars{}",
            spec_id,
            model,
            acc.chars().count(),
            if stopped { " · stopped" } else { "" }
        ));
        let _ = tx.send(Ev::Done {
            tokens_in,
            tokens_out: out_tokens,
            model,
        });
        ctx.request_repaint();
    })
}

pub fn spawn_opencode_chat(
    path: PathBuf,
    model: String,
    prompt: String,
    cwd: PathBuf,
    full: bool,
    timeout: u64,
    history_chars: usize,
    config: String,
    api_key: String,
    tx: Sender<Ev>,
    cancel: Arc<AtomicBool>,
    ctx: egui::Context,
) -> std::thread::JoinHandle<()> {
    spawn_chat(
        path,
        "opencode".into(),
        String::new(),
        model,
        prompt,
        cwd,
        full,
        timeout,
        history_chars,
        tx,
        cancel,
        ctx,
        vec![
            ("OPENCODE_CONFIG_CONTENT".into(), config),
            ("ROXY_PROVIDER_KEY".into(), api_key),
        ],
    )
}

fn sign_in_hint(spec: Option<&Spec>) -> String {
    match spec {
        Some(s) => format!(
            "Sign in first: run `{}` in a terminal (Models → {} → Sign in).",
            s.login, s.name
        ),
        None => {
            "The command failed. Check Models for the command and its sign-in step.".to_string()
        }
    }
}

/// Pull one published chunk out of a provider's NDJSON stream.
fn parse_line(parse: Parse, line: &str) -> (Option<String>, Option<u64>) {
    if line.is_empty() {
        return (None, None);
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
        // Some CLIs mix log lines into stdout.
        if line.starts_with('{') {
            return (None, None);
        }
        return (Some(format!("{line}\n")), None);
    };
    match parse {
        Parse::Codex => {
            let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if ty == "item.completed" {
                let item = v.get("item");
                let kind = item
                    .and_then(|i| i.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if kind == "agent_message" {
                    let text = item
                        .and_then(|i| i.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    return (Some(text.to_string()), None);
                }
                if kind == "error" {
                    let msg = item
                        .and_then(|i| i.get("message"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("Codex reported an error");
                    return (Some(format!("\n\n```\n{msg}\n```\n")), None);
                }
            }
            if ty == "turn.completed" {
                let out = v
                    .get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|t| t.as_u64());
                return (None, out);
            }
            (None, None)
        }
        Parse::Opencode => {
            let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if ty == "text" {
                let text = v
                    .get("part")
                    .and_then(|p| p.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                return (Some(text.to_string()), None);
            }
            if ty == "step_finish" {
                let out = v
                    .get("part")
                    .and_then(|p| p.get("tokens"))
                    .and_then(|t| t.get("output"))
                    .and_then(|t| t.as_u64());
                return (None, out);
            }
            if ty == "error" {
                let msg = v
                    .get("error")
                    .and_then(|e| e.get("data"))
                    .and_then(|d| d.get("message"))
                    .or_else(|| v.get("error").and_then(|e| e.get("name")))
                    .and_then(|t| t.as_str())
                    .unwrap_or("opencode reported an error");
                return (Some(format!("\n\n```\n{msg}\n```\n")), None);
            }
            (None, None)
        }
        Parse::Text => (Some(line.to_string()), None),
    }
}

// ---------------------------------------------------------------- lifecycle

#[derive(Clone, Debug)]
pub enum JobKind {
    Install,
    Update,
    Uninstall,
}

pub struct Job {
    pub cli: String,
    pub kind: JobKind,
    pub lines: VecDeque<String>,
    pub done: bool,
    pub ok: bool,
    pub rx: Option<std::sync::mpsc::Receiver<JobMsg>>,
}

pub enum JobMsg {
    Line(String),
    Done(bool),
}

/// Install or update a CLI into ROXY's own tools directory — no admin rights,
/// no PATH changes, nothing global on the user's machine.
pub fn spawn_install(spec: &'static Spec, kind: JobKind, ctx: egui::Context) -> Job {
    let (tx, rx) = std::sync::mpsc::channel::<JobMsg>();
    let dir = tools_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pkg = spec.pkg.to_string();
    let k = kind.clone();
    std::thread::spawn(move || {
        let Some(npm) = npm() else {
            let _ = tx.send(JobMsg::Line(
                "Node.js was not found. Install it from nodejs.org, then retry.".into(),
            ));
            let _ = tx.send(JobMsg::Done(false));
            ctx.request_repaint();
            return;
        };
        let mut cmd = base_command(&npm);
        let action = match k {
            JobKind::Uninstall => "uninstall",
            _ => "install",
        };
        cmd.arg(action)
            .arg("--prefix")
            .arg(&dir)
            .arg("--no-audit")
            .arg("--no-fund")
            .arg("--loglevel")
            .arg("error");
        if !matches!(k, JobKind::Uninstall) {
            cmd.arg(format!("{pkg}@latest"));
        } else {
            cmd.arg(&pkg);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("npm_config_update_notifier", "false")
            .env("NO_COLOR", "1");
        let _ = tx.send(JobMsg::Line(format!("$ npm {action} {pkg}")));
        ctx.request_repaint();

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(JobMsg::Line(format!("Could not run npm: {e}")));
                let _ = tx.send(JobMsg::Done(false));
                ctx.request_repaint();
                return;
            }
        };
        let out = child.stdout.take();
        let err = child.stderr.take();
        let tx2 = tx.clone();
        let ctx2 = ctx.clone();
        let handle = std::thread::spawn(move || {
            if let Some(err) = err {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    let _ = tx2.send(JobMsg::Line(strip_ansi(&line).trim_end().to_string()));
                    ctx2.request_repaint();
                }
            }
        });
        if let Some(out) = out {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send(JobMsg::Line(strip_ansi(&line).trim_end().to_string()));
                ctx.request_repaint();
            }
        }
        let _ = handle.join();
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        let _ = tx.send(JobMsg::Done(ok));
        ctx.request_repaint();
    });
    Job {
        cli: spec.id.to_string(),
        kind,
        lines: VecDeque::new(),
        done: false,
        ok: false,
        rx: Some(rx),
    }
}

// ---------------------------------------------------------------- system

pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url]);
        hide_console(&mut c);
        let _ = c.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

pub fn reveal_dir(path: &Path) {
    let p = path.display().to_string();
    #[cfg(windows)]
    {
        let mut c = Command::new("explorer");
        c.arg(&p);
        hide_console(&mut c);
        let _ = c.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(&p).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(&p).spawn();
    }
}

/// Open a real terminal running the CLI's sign-in flow.
pub fn open_login(spec: &Spec, path: Option<&Path>) {
    let bin = path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| spec.bin.to_string());
    #[cfg(not(windows))]
    let line = if spec.login.contains(' ') {
        let mut parts = spec.login.splitn(2, ' ');
        let first = parts.next().unwrap_or(spec.bin);
        let rest = parts.next().unwrap_or("");
        if first == spec.bin {
            format!("\"{bin}\" {rest}")
        } else {
            spec.login.to_string()
        }
    } else {
        format!("\"{bin}\"")
    };

    #[cfg(windows)]
    {
        // Keep the path as a PowerShell single-quoted literal. cmd.exe's
        // nested quoting breaks npm .cmd paths under user profiles containing
        // spaces, leaving the quote characters in the executable name.
        let exe = bin.replace('\'', "''");
        let login_args = if spec.login.contains(' ') {
            spec.login
                .split_once(' ')
                .map(|(_, args)| args)
                .unwrap_or("")
        } else {
            ""
        };
        let script = if login_args.is_empty() {
            format!("& '{exe}'")
        } else {
            format!("& '{exe}' {login_args}")
        };
        let mut c = Command::new("powershell.exe");
        c.args(["-NoExit", "-Command", &script]);
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        c.creation_flags(CREATE_NEW_CONSOLE);
        let _ = c.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            line.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let _ = Command::new("osascript").args(["-e", &script]).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let inner = format!("{line}; echo; echo 'You can close this window.'; exec sh");
        for (term, args) in [
            ("x-terminal-emulator", vec!["-e", "sh", "-c"]),
            ("gnome-terminal", vec!["--", "sh", "-c"]),
            ("konsole", vec!["-e", "sh", "-c"]),
            ("xfce4-terminal", vec!["-x", "sh", "-c"]),
            ("kitty", vec!["sh", "-c"]),
            ("alacritty", vec!["-e", "sh", "-c"]),
            ("xterm", vec!["-e", "sh", "-c"]),
        ] {
            if which_plain(term) {
                let mut c = Command::new(term);
                c.args(args).arg(&inner);
                if c.spawn().is_ok() {
                    return;
                }
            }
        }
    }
}

/// Open OpenCode's built-in ChatGPT/Codex account flow. This registers the
/// OpenAI account with OpenCode, whose models can then be selected through
/// ROXY's OpenCode route while keeping the current conversation context.
pub fn open_opencode_codex_login(path: &Path) {
    let spec = Spec {
        id: "opencode-openai-login",
        name: "OpenCode OpenAI sign-in",
        vendor: "OpenAI",
        short: "OA",
        bin: "opencode",
        pkg: "",
        color: "",
        blurb: "",
        docs: "",
        login: "opencode auth login openai",
        models: &[],
        parse: Parse::Opencode,
        server: false,
    };
    open_login(&spec, Some(path));
}

fn which_plain(name: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for d in std::env::split_paths(&path) {
            if d.join(name).is_file() {
                return true;
            }
        }
    }
    false
}

/// Pipe-free working directory for agent CLIs.
pub fn working_dir(configured: &str) -> PathBuf {
    if !configured.trim().is_empty() {
        let p = PathBuf::from(configured.trim());
        if p.is_dir() {
            return p;
        }
    }
    let p = store::dir().join("workspace");
    let _ = std::fs::create_dir_all(&p);
    p
}
/// Node.js is the prerequisite for every npm-installable agent.
pub fn node_status() -> (bool, String) {
    match find_bin("node") {
        Some(p) => {
            let v = run_version(&p, Duration::from_secs(4));
            (true, v)
        }
        None => (false, String::new()),
    }
}
