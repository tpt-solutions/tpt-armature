# Getting Started

TPT Armature is a 100% Rust reverse engineering and binary analysis suite. This
guide gets you from clone to your first analysis in a few minutes.

## Prerequisites

- Rust 1.85+ (`rustup default stable`)
- Optional: [`just`](https://github.com/casey/just) for the task recipes
- Optional (for extras): `rustup target add wasm32-unknown-unknown` (Wasm
  plugins), a C/compiler (not required).

## Build

```sh
cargo build --workspace            # default features (x86/x64 CLI)
cargo build --workspace --all-features   # everything (ARM, GUI, Rhai, Wasm)
```

## Analyze a binary

The CLI is `armature`. The supported subcommands are:

| Command  | Purpose                                                      |
| -------- | ----------------------------------------------------------- |
| `analyze`| High-level summary: format, arch, sections, CFG, xrefs.     |
| `disasm` | Print assembly text (`-n` limits instructions).            |
| `cfg`    | Print CFG statistics and edges.                             |
| `script` | Run a Rhai automation script (needs `--features rhai`).    |

```sh
# No sample binary? Analyze the CLI you just built:
cargo run -p armature-cli -- analyze target/debug/armature

cargo run -p armature-cli -- disasm target/debug/armature -n 32
```

## Try the GUI

```sh
cargo run -p armature-gui --features app -- path/to/binary
```

This tiles three views — Hex, Assembly, Graph — over the analyzed code section.

## Feature flags

| Flag    | Enables                                       |
| ------- | --------------------------------------------- |
| `arm`   | ARM / AArch64 disassembly (yaxpeax)           |
| `app`   | Native `egui` GUI                             |
| `rhai`  | Rhai scripting + `script` subcommand          |
| `wasm`  | Sandboxed Wasm plugins                        |

## Next steps

- See [SCRIPTING.md](./SCRIPTING.md) for writing Rhai automation.
- See `crates/armature-ext/scripts/` for ready-made script templates.
- See `crates/armature-ext/examples/plugins/hello` for a Wasm plugin template.
