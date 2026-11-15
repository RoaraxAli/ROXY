//! Session + config persistence in a per-user directory.

use crate::model::{Config, Session};
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join("roxy")
}

fn ensure_dir() -> PathBuf {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn config_path() -> PathBuf {
    dir().join("config.json")
}

pub fn sessions_path() -> PathBuf {
    dir().join("sessions.json")
}

pub fn load_config() -> Config {
    match std::fs::read_to_string(config_path()) {
        Ok(s) => serde_json::from_str::<Config>(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save_config(cfg: &Config) {
    let d = ensure_dir();
    if let Ok(s) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(d.join("config.json"), s);
    }
}

pub fn load_sessions() -> Vec<Session> {
    match std::fs::read_to_string(sessions_path()) {
        Ok(s) => serde_json::from_str::<Vec<Session>>(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_sessions(sessions: &[Session]) {
    let d = ensure_dir();
    if let Ok(s) = serde_json::to_string_pretty(sessions) {
        let _ = std::fs::write(d.join("sessions.json"), s);
    }
}

pub fn log(line: &str) {
    use std::io::Write;
    let d = ensure_dir();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(d.join("roxy.log"))
    {
        let _ = writeln!(f, "{line}");
    }
}
