//! Model gateway: OpenAI-compatible chat streaming, model discovery, and the
//! bridge to locally installed agent CLIs.
//!
//! Plain `http://` endpoints are spoken directly over TCP (no TLS dependency).
//! `https://` endpoints are streamed through the system `curl` binary, which
//! ships with Windows 10+, macOS and every Linux — so ROXY stays dependency-free
//! and still reaches hosted APIs like OpenRouter.

use crate::cli;
use crate::http;
use crate::model::{approx_tokens, Message, Provider, Role};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use crate::cli::Ev;

pub struct Disc {
    pub provider: String,
    pub models: Vec<String>,
    pub latency: u64,
    pub error: Option<String>,
}

/// Everything the router needs that is not part of the provider itself.
#[derive(Clone)]
pub struct Opts {
    pub system: String,
    pub stream: bool,
    pub temperature: f32,
    pub max_tokens: u32,
    /// 0 = replay the whole conversation.
    pub history_limit: usize,
    pub cli_path: Option<PathBuf>,
    pub cli_cwd: PathBuf,
    pub cli_full: bool,
    pub cli_timeout: u64,
    /// Installed OpenCode binary, used as the inference runtime for API models.
    pub opencode_path: Option<PathBuf>,
}

fn history(messages: &[Message], system: &str) -> Vec<Value> {
    let mut out = Vec::new();
    if !system.trim().is_empty() {
        out.push(json!({"role": "system", "content": system}));
    }
    for m in messages {
        match m.role {
            Role::User => out.push(json!({"role": "user", "content": m.text})),
            Role::Assistant => {
                if !m.text.trim().is_empty() {
                    out.push(json!({"role": "assistant", "content": m.text}));
                }
            }
            // Handoff markers are UI-only: real continuity comes from the
            // user/assistant turns above, which every provider receives in full.
            Role::Handoff | Role::System => {}
        }
    }
    out
}

pub fn payload(
    model: &str,
    messages: &[Message],
    system: &str,
    stream: bool,
    temperature: f32,
    max_tokens: u32,
) -> String {
    let mut body = json!({
        "model": model,
        "messages": history(messages, system),
        "stream": stream,
        "temperature": temperature,
    });
    if max_tokens > 0 {
        body["max_tokens"] = json!(max_tokens);
    }
    body.to_string()
}

fn extract_delta(v: &Value) -> Option<String> {
    let choices = v.get("choices")?.as_array()?;
    let first = choices.first()?;
    if let Some(d) = first.get("delta") {
        if let Some(c) = d.get("content").and_then(|c| c.as_str()) {
            return Some(c.to_string());
        }
    }
    if let Some(c) = first
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        return Some(c.to_string());
    }
    first
        .get("text")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}

fn usage_of(v: &Value) -> Option<(u64, u64)> {
    let u = v.get("usage")?;
    let i = u
        .get("prompt_tokens")
        .or_else(|| u.get("input_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let o = u
        .get("completion_tokens")
        .or_else(|| u.get("output_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    Some((i, o))
}

fn error_text(status: u16, body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(e) = v.get("error") {
            if let Some(m) = e.get("message").and_then(|m| m.as_str()) {
                return format!("{status} · {m}");
            }
            return format!("{status} · {e}");
        }
        if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
            return format!("{status} · {m}");
        }
    }
    let clean: String = body.chars().take(240).collect();
    format!("{status} · {}", clean.trim())
}

fn finish_hint(p: &Provider) -> String {
    if p.needs_key() {
        if p.key_env.is_empty() {
            "\nAdd an API key in Models → API models.".to_string()
        } else {
            format!("\nAdd a key in Models → API models, or set {}.", p.key_env)
        }
    } else {
        String::new()
    }
}

fn curl_bin() -> &'static str {
    "curl"
}

fn curl_headers(key: &str) -> Vec<String> {
    let mut v = vec![
        "-H".to_string(),
        "Content-Type: application/json".to_string(),
    ];
    if !key.trim().is_empty() {
        v.push("-H".to_string());
        v.push(format!("Authorization: Bearer {}", key.trim()));
    }
    v.push("-H".to_string());
    v.push("Accept: text/event-stream, application/json".to_string());
    v
}

fn curl_command() -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(curl_bin());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Stream response lines from either a direct TCP connection or `curl`.
fn run_stream(
    url: &str,
    key: &str,
    body: &str,
    cancel: &AtomicBool,
    on_line: &mut dyn FnMut(&str) -> bool,
) -> Result<(), String> {
    if url.starts_with("https://") {
        let mut args: Vec<String> = vec![
            "-sS".into(),
            "-N".into(),
            "--no-buffer".into(),
            "--max-time".into(),
            "900".into(),
            "-X".into(),
            "POST".into(),
            "--data-binary".into(),
            "@-".into(),
        ];
        args.extend(curl_headers(key));
        args.push(url.to_string());

        let mut child = curl_command()
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("curl not available: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(body.as_bytes());
        }

        let mut errs = String::new();
        let err_handle = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let mut buf = String::new();
                let r = BufReader::new(stderr);
                for line in r.lines().map_while(Result::ok) {
                    buf.push_str(&line);
                    buf.push('\n');
                    if buf.len() > 800 {
                        break;
                    }
                }
                buf
            })
        });

        let stdout = child.stdout.take().ok_or("curl: no stdout")?;
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                break;
            }
            match line {
                Ok(l) => {
                    if !on_line(&l) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        if let Some(h) = err_handle {
            errs = h.join().unwrap_or_default();
        }
        if !errs.trim().is_empty() {
            return Err(errs.trim().to_string());
        }
        return Ok(());
    }

    let (status, _headers, mut conn) = http::Conn::open(
        "POST",
        url,
        Some(key),
        Some(body),
        Duration::from_secs(6),
        Duration::from_secs(600),
    )?;

    if status >= 400 {
        let tail = conn.read_body_all().unwrap_or_default();
        return Err(error_text(status, &tail));
    }

    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        match conn.read_body_line() {
            Ok(Some(line)) => {
                if !on_line(&line) {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("stream error: {e}")),
        }
    }
    Ok(())
}

/// Stream a chat completion (or drive a local CLI) on a background thread.
#[allow(clippy::too_many_arguments)]
pub fn spawn_chat(
    provider: Provider,
    model: String,
    messages: Vec<Message>,
    opts: Opts,
    tx: Sender<Ev>,
    cancel: Arc<AtomicBool>,
    ctx: egui::Context,
) -> std::thread::JoinHandle<()> {
    if provider.is_cli() {
        let mut trimmed = messages;
        if opts.history_limit > 0 && trimmed.len() > opts.history_limit {
            trimmed.drain(..trimmed.len() - opts.history_limit);
        }
        let pairs: Vec<(bool, String)> = trimmed
            .iter()
            .filter_map(|m| match m.role {
                Role::User => Some((true, m.text.clone())),
                Role::Assistant => Some((false, m.text.clone())),
                _ => None,
            })
            .collect();
        let prompt = cli::build_prompt(&pairs, &opts.system);
        let chars = prompt.chars().count();
        let spec_id = provider.cli.clone();
        let custom = provider.cmd.clone();
        let Some(path) = opts.cli_path.clone() else {
            let _ = tx.send(Ev::Failed(format!(
                "{} is not installed yet.\nOpen Models → CLI agents and press Install.",
                provider.name
            )));
            ctx.request_repaint();
            return std::thread::spawn(|| {});
        };
        return cli::spawn_chat(
            path,
            spec_id,
            custom,
            model,
            prompt,
            opts.cli_cwd,
            opts.cli_full,
            opts.cli_timeout,
            chars,
            tx,
            cancel,
            ctx,
            Vec::new(),
        );
    }

    if let Some(path) = opts.opencode_path.clone() {
        let mut trimmed = messages;
        if opts.history_limit > 0 && trimmed.len() > opts.history_limit {
            trimmed.drain(..trimmed.len() - opts.history_limit);
        }
        let pairs: Vec<(bool, String)> = trimmed
            .iter()
            .filter_map(|m| match m.role {
                Role::User => Some((true, m.text.clone())),
                Role::Assistant => Some((false, m.text.clone())),
                _ => None,
            })
            .collect();
        let prompt = cli::build_prompt(&pairs, &opts.system);
        let chars = prompt.chars().count();
        let provider_id = format!(
            "roxy_{}",
            provider
                .id
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                })
                .collect::<String>()
        );
        let model_id = model.clone();
        let config = json!({
            "$schema": "https://opencode.ai/config.json",
            "providers": {
                (provider_id.clone()): {
                    "name": provider.name,
                    "package": "@opencode/ai/providers/openai-compatible",
                    "env": ["ROXY_PROVIDER_KEY"],
                    "settings": { "baseURL": provider.base_url.trim_end_matches('/') },
                    "models": { (model_id): { "name": model } }
                }
            }
        })
        .to_string();
        return cli::spawn_opencode_chat(
            path,
            format!("{provider_id}/{model}"),
            prompt,
            opts.cli_cwd,
            opts.cli_full,
            opts.cli_timeout,
            chars,
            config,
            provider.effective_key(),
            tx,
            cancel,
            ctx,
        );
    }

    std::thread::spawn(move || {
        let mut messages = messages;
        if opts.history_limit > 0 && messages.len() > opts.history_limit {
            messages.drain(..messages.len() - opts.history_limit);
        }

        // A router walks its ranked list: the selected model first, then the
        // rest of the user's priority order, so a rate limit or an outage on
        // one provider never stops the turn.
        let mut attempts: Vec<String> = vec![model.clone()];
        if provider.is_router() {
            for m in provider.ordered_models() {
                if !attempts.iter().any(|x| x == &m) {
                    attempts.push(m);
                }
            }
        }

        let url = provider.endpoint();
        let key = provider.effective_key();
        let started = Instant::now();
        let mut last_error: Option<String> = None;

        for attempt in attempts.iter() {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let body = payload(
                attempt,
                &messages,
                &opts.system,
                opts.stream,
                opts.temperature,
                opts.max_tokens,
            );

            let mut acc = String::new();
            let mut cin = 0u64;
            let mut cout = 0u64;
            let mut saw_usage = false;
            let mut failure: Option<String> = None;

            let result = run_stream(&url, &key, &body, &cancel, &mut |line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return true;
                }
                let data = if let Some(d) = trimmed.strip_prefix("data:") {
                    d.trim()
                } else if trimmed.starts_with('{') || trimmed.starts_with('[') {
                    trimmed
                } else {
                    return true;
                };
                if data == "[DONE]" {
                    return false;
                }
                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    if let Some((i, o)) = usage_of(&v) {
                        cin = i;
                        cout = o;
                        saw_usage = true;
                    }
                    if v.get("error").is_some() {
                        failure = Some(error_text(200, data));
                        return false;
                    }
                    if let Some(delta) = extract_delta(&v) {
                        if !delta.is_empty() {
                            acc.push_str(&delta);
                            let _ = tx.send(Ev::Delta(delta));
                            ctx.request_repaint();
                        }
                    }
                }
                true
            });

            let err = failure.or_else(|| match result {
                Ok(()) => None,
                Err(e) => Some(if e.contains("curl") && !e.contains("HTTP") {
                    format!("Can't reach {} ({url})\n{e}", provider.name)
                } else {
                    format!("{} {e}", provider.name)
                }),
            });

            match err {
                // Nothing was streamed yet: safe to fall through to the next
                // model in the ranked list.
                Some(e) if acc.is_empty() => {
                    last_error = Some(e);
                    continue;
                }
                Some(e) => {
                    let _ = tx.send(Ev::Failed(format!("{}{}", e, finish_hint(&provider))));
                    ctx.request_repaint();
                    return;
                }
                None => {}
            }

            if !saw_usage {
                cin = approx_tokens(
                    &serde_json::to_string(&history(&messages, &opts.system)).unwrap_or_default(),
                );
                cout = approx_tokens(&acc);
            }

            crate::store::log(&format!(
                "{} {} · {} ms · {} chars",
                provider.id,
                attempt,
                started.elapsed().as_millis(),
                acc.chars().count()
            ));

            let _ = tx.send(Ev::Done {
                tokens_in: cin,
                tokens_out: cout,
                model: attempt.clone(),
            });
            ctx.request_repaint();
            return;
        }

        let base = last_error.unwrap_or_else(|| {
            format!(
                "{} returned no response. Is the router running on port 20128?",
                provider.name
            )
        });
        let _ = tx.send(Ev::Failed(format!("{}{}", base, finish_hint(&provider))));
        ctx.request_repaint();
    })
}

fn parse_models(body: &str) -> Vec<String> {
    let mut models = Vec::new();
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
            for m in arr {
                if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                    models.push(id.to_string());
                }
            }
        } else if let Some(arr) = v.as_array() {
            for m in arr {
                if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                    models.push(id.to_string());
                }
            }
        }
    }
    models.sort();
    models
}

fn fetch_models(url: &str, key: &str) -> Result<String, String> {
    if url.starts_with("https://") {
        let mut args: Vec<String> = vec![
            "-sS".into(),
            "--max-time".into(),
            "15".into(),
            "-X".into(),
            "GET".into(),
        ];
        args.extend(curl_headers(key));
        args.push(url.to_string());
        let out = curl_command()
            .args(&args)
            .output()
            .map_err(|e| format!("curl not available: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        return Ok(String::from_utf8_lossy(&out.stdout).to_string());
    }
    let (status, body) = http::json("GET", url, Some(key), None, Duration::from_secs(8))?;
    if status >= 400 {
        return Err(error_text(status, &body));
    }
    Ok(body)
}

/// Ask an API provider for its model list. CLIs keep their catalogue list.
pub fn spawn_discovery(provider: Provider, tx: Sender<Disc>, ctx: egui::Context) {
    if provider.is_cli() {
        return;
    }
    std::thread::spawn(move || {
        let started = Instant::now();
        let url = provider.models_url();
        let res = fetch_models(&url, &provider.effective_key());
        let latency = started.elapsed().as_millis() as u64;
        match res {
            Ok(body) => {
                let models = parse_models(&body);
                let looks_html = body.trim_start().starts_with('<');
                let error = if models.is_empty() && looks_html {
                    Some("endpoint returned a web page — not an OpenAI-compatible API".to_string())
                } else {
                    None
                };
                let _ = tx.send(Disc {
                    provider: provider.id.clone(),
                    models,
                    latency,
                    error,
                });
            }
            Err(e) => {
                let _ = tx.send(Disc {
                    provider: provider.id.clone(),
                    models: Vec::new(),
                    latency,
                    error: Some(e),
                });
            }
        }
        ctx.request_repaint();
    });
}
