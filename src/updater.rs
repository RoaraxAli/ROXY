//! Self-updater backed by the public ROXY release manifest.
//!
//! The download runs on a background thread and reports progress to the UI, so
//! the update is a visible in-app flow: check → download with a progress bar →
//! swap the binary → restart. No terminal window is opened on Windows.
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Release manifest. Builds that self-host can override at compile time with
/// `ROXY_MANIFEST=… cargo build --release`; the default points at the public
/// project manifest so the published source never hardcodes a private host.
const MANIFEST: &str = match option_env!("ROXY_MANIFEST") {
    Some(v) => v,
    None => "https://raw.githubusercontent.com/RoaraxAli/ROXY/main/update.json",
};

#[derive(Clone, Deserialize)]
pub struct Release {
    pub version: String,
    pub windows_x64: String,
    pub linux_aarch64: String,
}

/// Progress events for the in-app update screen.
#[derive(Clone, Debug)]
pub enum Upd {
    /// Bytes written so far, and the total when the server reports one.
    Downloading(u64, Option<u64>),
    /// The new binary is in place; ROXY is about to restart.
    Installing,
    Failed(String),
}

pub fn check() -> Result<Release, String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            MANIFEST,
        ])
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if !output.status.success() {
        return Err("release server is unavailable".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("invalid release manifest: {e}"))
}

fn total_size(url: &str) -> Option<u64> {
    let out = Command::new("curl")
        .args(["-fsIL", "--connect-timeout", "8", "--max-time", "20", url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            if k.trim().eq_ignore_ascii_case("content-length") {
                v.trim().parse::<u64>().ok()
            } else {
                None
            }
        })
        .next_back()
}

fn staged_path() -> PathBuf {
    std::env::temp_dir().join(if cfg!(windows) {
        "roxy-update.exe"
    } else {
        "roxy-update"
    })
}

/// Which download this platform should install, if any.
fn url_for(release: &Release) -> Option<String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Some(release.windows_x64.clone());
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Some(release.linux_aarch64.clone());
    }
    #[allow(unreachable_code)]
    {
        let _ = release;
        None
    }
}

/// Download the matching single-file binary, then replace this executable and
/// restart once the process exits. Progress is streamed back on `tx`.
pub fn install(release: Release, current: PathBuf, tx: Sender<Upd>) {
    std::thread::spawn(move || {
        let Some(url) = url_for(&release) else {
            let _ = tx.send(Upd::Failed(
                "No prebuilt binary for this platform — build from source.".into(),
            ));
            return;
        };
        let staged = staged_path();
        let total = total_size(&url);

        let mut child = match Command::new("curl")
            .args(["-fL", "--connect-timeout", "15", "--max-time", "600", "-o"])
            .arg(&staged)
            .arg(&url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Upd::Failed(format!("could not run curl: {e}")));
                return;
            }
        };

        // Poll the growing file so the UI has a real byte count even when the
        // server omits Content-Length.
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        let _ = tx.send(Upd::Failed(
                            "Download failed — check your connection and retry.".into(),
                        ));
                        return;
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = tx.send(Upd::Failed(format!("download error: {e}")));
                    return;
                }
            }
            if started.elapsed() > Duration::from_secs(600) {
                let _ = child.kill();
                let _ = tx.send(Upd::Failed("Download timed out.".into()));
                return;
            }
            let written = std::fs::metadata(&staged).map(|m| m.len()).unwrap_or(0);
            let _ = tx.send(Upd::Downloading(written, total));
            std::thread::sleep(Duration::from_millis(180));
        }

        let written = std::fs::metadata(&staged).map(|m| m.len()).unwrap_or(0);
        let _ = tx.send(Upd::Downloading(written, total));
        let _ = tx.send(Upd::Installing);

        let pid = std::process::id();
        #[cfg(target_os = "windows")]
        {
            let script = std::env::temp_dir().join("roxy-update.ps1");
            let body = format!(
                "$ErrorActionPreference='SilentlyContinue'\nWait-Process -Id {pid}\nMove-Item -Force '{}' '{}'\nStart-Process '{}'\n",
                staged.display(),
                current.display(),
                current.display()
            );
            if std::fs::write(&script, body).is_err() {
                let _ = tx.send(Upd::Failed("Could not stage the update script.".into()));
                return;
            }
            let launched = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                ])
                .arg(&script)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok();
            if !launched {
                let _ = tx.send(Upd::Failed("Could not start the installer.".into()));
                return;
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
            let script = std::env::temp_dir().join("roxy-update.sh");
            let body = format!(
                "#!/bin/sh\nwhile kill -0 {pid} 2>/dev/null; do sleep 1; done\nmv '{}' '{}'\nchmod +x '{}'\n'{}' >/dev/null 2>&1 &\n",
                staged.display(),
                current.display(),
                current.display(),
                current.display()
            );
            if std::fs::write(&script, body).is_err() {
                let _ = tx.send(Upd::Failed("Could not stage the update script.".into()));
                return;
            }
            if Command::new("sh").arg(&script).spawn().is_err() {
                let _ = tx.send(Upd::Failed("Could not start the installer.".into()));
                return;
            }
        }

        // Give the UI a beat to paint the final state, then hand over.
        std::thread::sleep(Duration::from_millis(700));
        std::process::exit(0);
    });
}
