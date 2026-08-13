# tpt-armature-gui

**TPT Armature — Layer 3 (Presentation): "The Canvas".**

Composes the analyst-facing views (hex / assembly / graph) with `egui`. The
panel layout types are always available; the actual application is gated behind
the `app` feature so the workspace builds without a native windowing backend.

## Run

```sh
cargo run -p tpt-armature-gui --features app -- path/to/binary
```

With `--features scripts` an in-app Rhai console is also available.

## Key types

| Type | Purpose |
| ---- | ------- |
| `Panel` / `PanelLayout` | Always-available view layout primitives. |
| `run` (feature `app`) | Launch the egui/eframe desktop application. |

The application tiles three views — **Hex**, **Assembly**, **Graph** — over the
analyzed code section, plus a right-hand info panel with **Strings** and
**Pseudocode** tabs. Use the **goto** box to jump to an address, the **search**
box to find instructions, and the arrow keys to step through the Assembly view.

## Feature flags

| Flag | Enables |
| ---- | ------- |
| `app` | Native `egui` / `eframe` desktop GUI. |
| `scripts` | In-app Rhai script console (implies `app`). |

## WebAssembly (browser demo)

With `wasm32-unknown-unknown` the GUI builds for the browser and mounts into the
`#armature_canvas` element:

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p tpt-armature-gui --features scripts
```

## Minimum supported Rust version

1.85 (edition 2021).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
