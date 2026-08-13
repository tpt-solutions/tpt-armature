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

The CLI is `tpt-armature`. The supported subcommands are:

| Command    | Purpose                                                            |
| ---------- | ------------------------------------------------------------------ |
| `analyze`  | High-level summary: format, arch, sections, CFG, xrefs, debug syms. |
| `disasm`   | Print assembly text (`-n` limits instructions).                    |
| `cfg`      | Print CFG statistics and edges.                                    |
| `strings`  | Extract printable strings and immediate constants.                 |
| `decompile`| Render a C-like pseudocode view of recovered functions.            |
| `script`   | Run a Rhai automation script (needs `--features rhai`).            |
| `plugin`   | Run one sandboxed Wasm plugin (needs `--features wasm`).           |
| `plugins`  | Run every `.wasm` in a directory (needs `--features wasm`).        |
| `watch`    | Re-analyze a binary whenever it changes on disk.                   |

`analyze` also accepts `--json` (CI-friendly summary) and, with
`--features debuginfo`, `--pdb <file.pdb>` to merge PDB public symbols into the
analysis.

```sh
# No sample binary? Analyze the CLI you just built:
cargo run -p tpt-armature-cli -- analyze target/debug/tpt-armature

cargo run -p tpt-armature-cli -- disasm target/debug/tpt-armature -n 32

# Strings, constants, and a pseudocode listing:
cargo run -p tpt-armature-cli -- strings target/debug/tpt-armature
cargo run -p tpt-armature-cli -- decompile target/debug/tpt-armature -n 2
```

Prefer a guided tour? See [QUICKSTART.md](./QUICKSTART.md).

## Try the GUI

```sh
cargo run -p tpt-armature-gui --features app -- path/to/binary
```

This tiles three views — Hex, Assembly, Graph — over the analyzed code section,
plus a right-hand info panel with **Strings** and **Pseudocode** tabs. Use the
**goto** box to jump to an address, the **search** box to find instructions, and
the **↑/↓** arrow keys to step through the Assembly view. With `--features
scripts` an in-app Rhai console is also available.

## Feature flags

| Flag       | Enables                                                |
| ---------- | ------------------------------------------------------ |
| `arm`      | ARM / AArch64 disassembly (yaxpeax)                    |
| `app`      | Native `egui` GUI                                      |
| `rhai`     | Rhai scripting + `script` subcommand                   |
| `wasm`     | Sandboxed Wasm plugins (`plugin`/`plugins`)            |
| `debuginfo`| ELF `.symtab` + PE PDB symbol import (`analyze --pdb`) |

## Next steps

- See [SCRIPTING.md](./SCRIPTING.md) for writing Rhai automation.
- See `crates/tpt-armature-ext/scripts/` for ready-made script templates.
- See `crates/tpt-armature-ext/examples/plugins/tpt-armature-hello-plugin` for a Wasm plugin template.
