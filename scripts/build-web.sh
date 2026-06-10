#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

rustup target add wasm32-unknown-unknown >/dev/null
cargo build --release --target wasm32-unknown-unknown

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen CLI is required." >&2
  echo "Install the version matching Cargo.lock, for example:" >&2
  echo "  cargo install wasm-bindgen-cli --version 0.2.108" >&2
  exit 1
fi

rm -rf pkg
wasm-bindgen \
  --target web \
  --out-dir pkg \
  --out-name svo_engine \
  target/wasm32-unknown-unknown/release/svo_engine.wasm

cat <<'MSG'
Built pkg/svo_engine.js and pkg/svo_engine_bg.wasm.
Serve the repository root over HTTP, then open web/index.html in Firefox:
  python3 -m http.server 8000
  http://localhost:8000/web/
MSG
