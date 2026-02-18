# Build for WebGPU + WebAssembly

Use the one-command helper script to compile this engine for `wasm32-unknown-unknown`, generate browser bindings, and create a runnable `web/index.html`.

## Quick start

From the repository root:

```bash
./scripts/build_webgpu_wasm.sh
```

The script will:

1. Ensure the Rust target `wasm32-unknown-unknown` is installed.
2. Ensure `wasm-bindgen-cli` is installed.
3. Build a release `.wasm` for this crate.
4. Generate browser JS glue into `web/dist`.
5. Write `web/index.html` that loads the generated module.

## Output artifacts

After a successful run:

- `web/dist/svo_engine.js`
- `web/dist/svo_engine_bg.wasm`
- `web/index.html`

## Run locally

```bash
python3 -m http.server 8080 -d web
```

Open:

- `http://localhost:8080`

## Browser requirements

- Use a current Chromium-based browser with WebGPU enabled (latest Chrome/Edge recommended).
- WebGPU requires HTTPS in production (localhost is allowed for development).

## Notes

- If rendering appears black, check browser console for WebGPU adapter/device errors.
- Current asset loading in the app uses a filesystem path (`assets/models/house.vox`); for a production web build, package assets for HTTP loading or embed bytes at compile time.
