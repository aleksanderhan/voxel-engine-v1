# Build for WebGPU + WebAssembly

This engine already type-checks for `wasm32-unknown-unknown`, so you can compile it for the web with the steps below.

## 1) Install required toolchains

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

## 2) Compile to `.wasm`

```bash
cargo build --release --target wasm32-unknown-unknown
```

This produces:

- `target/wasm32-unknown-unknown/release/svo_engine.wasm`

## 3) Generate browser JS glue

```bash
wasm-bindgen \
  --target web \
  --out-dir web/dist \
  target/wasm32-unknown-unknown/release/svo_engine.wasm
```

This generates files such as:

- `web/dist/svo_engine.js`
- `web/dist/svo_engine_bg.wasm`

## 4) Create a minimal web runner

Create `web/index.html`:

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>SVO Engine (WebGPU)</title>
    <style>
      html, body, canvas { margin: 0; width: 100%; height: 100%; background: #000; }
      canvas { display: block; }
    </style>
  </head>
  <body>
    <script type="module">
      import init from './dist/svo_engine.js';
      await init();
    </script>
  </body>
</html>
```

## 5) Serve locally (required for browser wasm/WebGPU)

```bash
python3 -m http.server 8080 -d web
```

Then open:

- `http://localhost:8080`

## 6) Browser requirements

- Use a current Chromium-based browser with WebGPU enabled (latest Chrome/Edge recommended).
- WebGPU requires HTTPS in production (localhost is allowed for development).

## Notes

- If rendering appears black, check browser console for WebGPU adapter/device errors.
- Current asset loading in the app uses a filesystem path (`assets/models/house.vox`); for a production web build, package assets for HTTP loading or embed bytes at compile time.
