# Packaging & Distribution

TPT Armature ships two primary artifacts:

- **`armature`** — the headless CLI driver (`crates/armature-cli`). Builds anywhere
  Rust does, no native windowing backend required.
- **`armature-gui`** — the desktop GUI (`crates/armature-gui`, feature `app` or
  `scripts`). Builds on Windows, macOS, and Linux using `egui`/`eframe`'s native
  backend.

## Native builds (Windows / macOS / Linux)

The workspace `Cargo.toml` defines a tuned `[profile.release]` (LTO, single
codegen unit, stripped symbols). Build the release binaries with:

```sh
# Headless CLI only (lightest; recommended for CI / servers):
cargo build --release -p armature-cli

# CLI + Rhai scripting subcommand:
cargo build --release -p armature-cli --features rhai

# Desktop GUI (needs a windowing system):
cargo build --release -p armature-gui --features scripts
```

`cargo build --release --workspace` produces every crate's binary. Artifacts land
in `target/release/` (`target/release/armature` and
`target/release/armature-gui`).

### Cross-target triple quick reference

| Platform   | Target triple                        | Notes                                  |
|------------|--------------------------------------|----------------------------------------|
| Windows    | `x86_64-pc-windows-msvc`             | default on Windows                     |
| macOS      | `x86_64-apple-darwin` / `aarch64-apple-darwin` | use `--target` for the arch    |
| Linux      | `x86_64-unknown-linux-gnu`           | needs `libssl`/`xcb` etc. for the GUI  |

The GUI uses `eframe`, which on Linux pulls in system libraries (X11/Wayland,
fontconfig). On a headless Linux CI runner, install the equivalents of
`libx11-dev libxkbcommon-dev libwayland-dev libfontconfig1-dev` before building
the GUI.

### CI release matrix

`.github/workflows/release.yml` builds `armature-cli` (and `armature-gui
--features scripts`) on a 3-OS matrix (`ubuntu-latest`, `macos-latest`,
`windows-latest`) and uploads the stripped binaries as workflow artifacts. Run it
manually from the Actions tab, or fold it into a tag-push trigger for GitHub
Releases.

## WebAssembly (browser demo) — optional

The GUI can also target `wasm32-unknown-unknown` for an in-browser demo:

```sh
rustup target add wasm32-unknown-unknown
just build-wasm-gui          # -> target/wasm32-unknown-unknown/release/armature_gui.wasm
```

`armature-gui`'s `run()` automatically selects `eframe::start_web` when compiled
for `wasm32`, mounting into a canvas element with id `armature_canvas`. Serve the
resulting `.wasm` (e.g. via `trunk` or `wasm-pack` + a tiny HTML shim) to load a
binary with the file picker.

## Notes on optional features

- `armature-ext` is feature-gated; `wasmtime` (the plugin host) is only compiled
  when the `wasm` feature is enabled, keeping the default build light.
- `cargo deny check` and `cargo audit` must stay clean before a release (see
  `deny.toml` and `.cargo/audit.toml`).
