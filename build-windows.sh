#!/usr/bin/env bash
# Build ROXY for Windows from this Linux VM and publish it for download.
set -euo pipefail
cd "$(dirname "$0")"

# Private build settings (self-hosted manifest URL, signing hooks) live in
# local.env, which is gitignored — the published repository stays clean.
if [ -f local.env ]; then
  . ./local.env
  # local.env must reach cargo, which only reads exported variables.
  [ -n "${ROXY_MANIFEST:-}" ] && export ROXY_MANIFEST
fi

source "$HOME/.cargo/env"
rustup target add x86_64-pc-windows-gnu >/dev/null 2>&1 || true

cargo build --release
cargo build --release --target x86_64-pc-windows-gnu

EXE="target/x86_64-pc-windows-gnu/release/roxy.exe"
BIN="target/release/roxy"

mkdir -p ../dist/roxy
cp "$EXE" ../dist/roxy/roxy.exe
cp "$BIN" ../dist/roxy/roxy-linux
cp README.md ../dist/roxy/README.md
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
sed "s/__VERSION__/$VERSION/" download/index.html > ../dist/roxy/index.html

( cd ../dist/roxy && zip -q -FS roxy-windows-x64.zip roxy.exe README.md )

# Publish SHA-256 values with every release so Windows users can verify that
# their download matches the files served by this VM.
( cd ../dist/roxy && sha256sum roxy-windows-x64.zip roxy.exe roxy-linux > SHA256SUMS.txt )

# Optional: mirror the generated artifacts to a web root. Set
# ROXY_PUBLISH_DIR in local.env for self-hosted builds; nothing is committed.
if [ -n "${ROXY_PUBLISH_DIR:-}" ]; then
  install -d "$ROXY_PUBLISH_DIR"
  install -m 0644 ../dist/roxy/roxy-windows-x64.zip "$ROXY_PUBLISH_DIR/"
  install -m 0644 ../dist/roxy/roxy.exe "$ROXY_PUBLISH_DIR/"
  install -m 0644 ../dist/roxy/roxy-linux "$ROXY_PUBLISH_DIR/"
  install -m 0644 ../dist/roxy/README.md "$ROXY_PUBLISH_DIR/"
  install -m 0644 ../dist/roxy/SHA256SUMS.txt "$ROXY_PUBLISH_DIR/"
  install -m 0644 ../dist/roxy/index.html "$ROXY_PUBLISH_DIR/"
fi

echo "built:"
ls -lh "$EXE" "$BIN" ../dist/roxy/roxy-windows-x64.zip ../dist/roxy/SHA256SUMS.txt
