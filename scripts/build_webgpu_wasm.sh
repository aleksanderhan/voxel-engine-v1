#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_NAME="$(cargo metadata --format-version=1 --no-deps | sed -n 's/.*"name":"\([^"]*\)".*/\1/p' | head -n1)"
TARGET="wasm32-unknown-unknown"
PROFILE="release"
OUT_DIR="$ROOT_DIR/web/dist"
WEB_DIR="$ROOT_DIR/web"
WASM_PATH="$ROOT_DIR/target/$TARGET/$PROFILE/${CRATE_NAME}.wasm"
CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
WASM_BINDGEN_BIN=""

if ! command -v rustup >/dev/null 2>&1; then
  echo "error: rustup is required but was not found in PATH" >&2
  exit 1
fi

if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "Installing Rust target: $TARGET"
  rustup target add "$TARGET"
fi

if command -v wasm-bindgen >/dev/null 2>&1; then
  WASM_BINDGEN_BIN="$(command -v wasm-bindgen)"
elif [ -x "$CARGO_BIN_DIR/wasm-bindgen" ]; then
  WASM_BINDGEN_BIN="$CARGO_BIN_DIR/wasm-bindgen"
else
  echo "Installing wasm-bindgen-cli"
  cargo install wasm-bindgen-cli

  if [ -x "$CARGO_BIN_DIR/wasm-bindgen" ]; then
    WASM_BINDGEN_BIN="$CARGO_BIN_DIR/wasm-bindgen"
  elif command -v wasm-bindgen >/dev/null 2>&1; then
    WASM_BINDGEN_BIN="$(command -v wasm-bindgen)"
  else
    echo "error: wasm-bindgen was installed but is not available in PATH or $CARGO_BIN_DIR" >&2
    exit 1
  fi
fi

mkdir -p "$OUT_DIR"
mkdir -p "$WEB_DIR/.well-known/appspecific"

echo "Building crate '$CRATE_NAME' for $TARGET ($PROFILE)..."
cargo build --$PROFILE --target "$TARGET"

echo "Generating browser bindings with wasm-bindgen ($WASM_BINDGEN_BIN)..."
"$WASM_BINDGEN_BIN" \
  --target web \
  --out-dir "$OUT_DIR" \
  "$WASM_PATH"

cat > "$WEB_DIR/index.html" <<HTML
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <link rel="icon" type="image/svg+xml" href="./favicon.svg" />
    <title>SVO Engine (WebGPU + WASM)</title>
    <style>
      html, body, canvas { margin: 0; width: 100%; height: 100%; background: #000; }
      canvas { display: block; }
    </style>
  </head>
  <body>
    <script type="module">
      import init from './dist/${CRATE_NAME}.js';

      async function bootstrap() {
        const wasmSupported =
          typeof WebAssembly === 'object' &&
          typeof WebAssembly.instantiate === 'function';
        const webGpuApiPresent = typeof navigator !== 'undefined' && !!navigator.gpu;

        console.warn('[support] wasmSupported =', wasmSupported);
        console.warn('[support] webGpuApiPresent =', webGpuApiPresent);

        if (!wasmSupported || !webGpuApiPresent) {
          console.error('[support] Cannot start SVO Engine: missing required browser capabilities.', {
            wasmSupported,
            webGpuApiPresent,
          });
          return;
        }

        try {
          await init();
        } catch (error) {
          console.error('[support] SVO Engine init failed. WebGPU may be unavailable (no adapter/device).', error);
        }
      }

      bootstrap();
    </script>
  </body>
</html>
HTML

cat > "$WEB_DIR/.well-known/appspecific/com.chrome.devtools.json" <<JSON
{}
JSON

cat > "$WEB_DIR/favicon.svg" <<SVG
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <rect width="64" height="64" fill="#000" />
  <path d="M12 16h40v8H12zm0 16h40v8H12zm0 16h28v8H12z" fill="#3ddc97" />
</svg>
SVG

echo
cat <<MSG
Web build complete.

Artifacts:
  - $OUT_DIR/${CRATE_NAME}.js
  - $OUT_DIR/${CRATE_NAME}_bg.wasm
  - $WEB_DIR/index.html

Run locally:
  python3 -m http.server 8080 -d "$WEB_DIR"
Then open:
  http://localhost:8080
MSG
