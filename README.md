# voxel-engine-v1
svo engine

## Build for WebGPU + WebAssembly

Run:

```bash
./scripts/build_webgpu_wasm.sh
```

Then serve the generated web output:

```bash
python3 -m http.server 8080 -d web
```

Open `http://localhost:8080` in a browser with WebGPU support.

For additional details, see `docs/webgpu-wasm.md`.
