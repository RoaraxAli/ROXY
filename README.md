# ROXY

A native desktop AI client written in Rust — **one session, every model**.

ROXY keeps a single conversation per session and lets you change which model
answers *mid-thread*. Switch because a limit hit, because another brain fits
better, or just to compare. The full transcript is replayed to whichever
provider serves the next turn, so nothing is lost and nothing is summarised.

```
┌──────────┬────────────┬──────────────────────────────────────┐
│  rail    │  sessions  │            transcript                │
│          │            │                                      │
│  chat    │  new chat  │   you:  …                            │
│  models  │  search    │   AGY · gemini-3.8-flash   ── handoff ──
│          │  list      │   CDX · codex                        │
│  settings│  usage     │                                      │
│          │            │        ┌──────────────────────────┐   │
│          │            │        │ composer + model switch   │   │
└──────────┴────────────┴────────┴──────────────────────────┴───┘
```

## Why it is fast

* **Rust + egui/eframe (glow)** — immediate-mode UI on the GPU, no webview, no Node.
* **2 direct dependencies** (`serde`, `serde_json`); the UI toolkit is the only
  large tree. The rest is the standard library.
* **Hand-written HTTP/1.1 client** over `TcpStream`, with chunked + SSE decoding.
  No TLS stack linked in: `https://` endpoints are streamed through the system
  `curl` (present on Windows 10+, macOS and Linux), so the binary stays small.
* Release binary: **~8 MB (Linux) / 6.6 MB (Windows .exe)**, single file, no runtime.
* Idle CPU is 0: the UI repaints only on input or while tokens are streaming.

## Build

```bash
# Linux
cargo build --release
./target/release/roxy

# Windows cross-compile from Linux (already configured in .cargo/config.toml)
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
# → target/x86_64-pc-windows-gnu/release/roxy.exe
```

Requires `gcc-mingw-w64-x86-64` for the Windows target. `build-windows.sh`
builds both binaries, zips the Windows one, and writes `SHA256SUMS.txt`.

Builds self-hosted from a fork can point the updater at their own manifest at
compile time, without touching the source:

```bash
ROXY_MANIFEST="https://example.com/roxy/update.json" cargo build --release
```

Without that variable the binary reads the public manifest at
`update.json` on the `main` branch of this repository, whose download URLs point
at the newest GitHub release. `build-windows.sh` also reads an optional,
gitignored `local.env` for machine-specific settings.

The resulting `.exe` imports only system DLLs (kernel32, user32, gdi32,
opengl32, …) — copy it to Windows and double-click; no installer and no runtime
needed.

### Code signing (removes "unknown publisher")

Windows shows **“Windows protected your PC — unknown publisher”** for any binary
without an Authenticode signature. That message is about identity, not safety,
and it cannot be removed in code: it needs a certificate issued to a verified
publisher. Until a release is signed, either accept the prompt via **More info →
Run anyway**, or clear the download mark first:

```powershell
Unblock-File .\roxy.exe
```

If you own a code-signing certificate, `build-windows.sh` signs the executable
before zipping it. Set these in `local.env` (gitignored) or the environment:

```bash
ROXY_SIGN_CERT=/path/to/cert.pfx
ROXY_SIGN_PASS=…                                  # optional; prompted otherwise
ROXY_SIGN_URL=https://timestamp.digicert.com      # "none" to skip timestamping
```

It needs `osslsigncode` (`apt install osslsigncode`). The build timestamps the
signature, retries without a timestamp if the time-stamp authority is
unreachable, and falls back to an unsigned binary rather than failing.

Windows releases are currently unsigned. Windows therefore cannot show a
verified publisher, and SmartScreen may warn about an unrecognized app. The
release page identifies the ROXY project as publisher and provides SHA-256
hashes; compare a downloaded file with `SHA256SUMS.txt` using PowerShell:

```powershell
Get-FileHash .\roxy-windows-x64.zip -Algorithm SHA256
```

Hashes help detect a damaged or mismatched download. They are published in the
GitHub release next to the binaries, so they catch corruption and swapped files;
they are not a substitute for a signature. A verified Windows publisher requires
a code-signing certificate and signed builds.

## Providers and engine

Add an API provider in **Models → API models**, enter its key, and select one of
its discovered models. When OpenCode is installed from **Models → CLI agents**,
ROXY sends hosted API model turns through OpenCode's headless `run` command. It
passes the selected provider definition for that turn through OpenCode's inline
configuration and supplies the API key only to that child process. The normal
OpenCode config and auth files are left alone. If OpenCode is not installed,
API providers continue to use ROXY's OpenAI-compatible HTTP client.

Agent CLI cards remain individually installable. ROXY starts those CLIs using
their own headless interfaces and replays the same session transcript. OpenCode
does not provide built-in adapters for every third-party CLI, so those tools do
not become OpenCode providers simply because their executable is installed.

## 9Router and model priority

ROXY ships with a local [9Router](https://github.com/decolua/9router) route at
`http://localhost:20128/v1`. In **Models → API models** the 9Router card can
install and start the router, open its dashboard, and pull the model list it
exposes.

Models there can be ranked. The ranked order is what ROXY walks on every turn:
your selected model first, then the rest of the list. If a request fails or a
provider is rate-limited before any text has streamed, ROXY silently retries the
next model in the ranking, so a turn rarely dies on one provider.

## Data

| OS      | Folder                  |
|---------|-------------------------|
| Linux   | `~/.config/roxy/`       |
| Windows | `%APPDATA%\roxy\`       |

`config.json` (providers, accent, behaviour), `sessions.json` (every transcript),
`roxy.log` (one line per completed turn). Delete the folder for a clean slate.

## Shortcuts

| Key            | Action          |
|----------------|-----------------|
| `Ctrl/⌘ + N`   | new session     |
| `Ctrl/⌘ + ,`   | settings        |
| `Enter`        | send            |
| `Shift+Enter`  | newline         |
| `Esc`          | stop generation |

## Updates

ROXY checks a small release manifest when it starts and shows the result in
**Settings → About**. When a newer release exists, the in-app **Download &
install** flow shows a live progress bar, swaps the binary after the app closes
and restarts ROXY. Nothing opens a terminal window.
Keep the executable in a folder where your account can write files.

## Layout of the source

```
src/main.rs        entry point, window, generated icon
src/app.rs         state, streaming pump, routing/handoff logic
src/model.rs       Config / Provider / Session / Message
src/http.rs        minimal HTTP/1.1 + chunked + SSE reader
src/llm.rs         OpenAI requests, streaming, discovery, curl fallback
src/store.rs       JSON persistence per OS
src/theme.rs       palette, Inter + JetBrains Mono, egui styling
src/ui/            splash, rail, sessions, transcript, composer, pages
src/ui/markdown.rs small markdown renderer (bold, italic, code, lists…)
```
