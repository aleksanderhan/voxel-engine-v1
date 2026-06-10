# voxel-engine-v1

SVO voxel engine.

## Build and run natively

```sh
cargo run
```

## Build and run the WebGPU/WASM version in Firefox

Firefox needs WebGPU available for `navigator.gpu` in a secure context. `localhost`
counts as secure; if your Firefox channel still gates WebGPU, enable the browser's
WebGPU preference before opening the page.

Install the matching `wasm-bindgen` CLI once:

```sh
cargo install wasm-bindgen-cli --version 0.2.108
```

Build the web bundle:

```sh
./scripts/build-web.sh
```

Serve the repository root and open the web page:

```sh
python3 -m http.server 8000
```

Then visit <http://localhost:8000/web/> in Firefox.
