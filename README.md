# TPT Armature

TPT Armature is a memory-safe, 100% Rust reverse engineering and binary analysis
suite. It ingests raw executables (PE, ELF, Mach-O), disassembles machine code
into a custom intermediate representation, builds control-flow and data-flow
analyses, and presents the results through a native GUI with scripting and
sandboxed plugin extensibility.

"Expose the hidden framework."

See [`spec.txt`](./spec.txt) for the full System Design Document.

## Workspace layout

| Crate                | Layer | Responsibility                                            |
| -------------------- | ----- | --------------------------------------------------------- |
| `armature-formats`   | 1     | Binary parsing (PE/ELF/Mach-O) -> standardized `MemoryMap` |
| `armature-ir`        | 2     | Custom IR (instructions, operands, basic blocks) + builder |
| `armature-disasm`    | 2     | Disassembly backends (iced-x86/x64, yaxpeax ARM) -> IR     |
| `armature-analysis`  | 2     | CFG construction, data-flow, cross-reference index        |
| `armature-gui`       | 3     | Presentation (hex / assembly / graph views)               |
| `armature-ext`       | 4     | Rhai scripting + wasmtime sandboxed plugins                |
| `armature-cli`       | 5     | Headless pipeline + CLI driver                            |

## Quick start

```sh
cargo build --workspace
cargo run -p armature-cli -- analyze path/to/binary
cargo run -p armature-cli -- disasm path/to/binary
# Run a Rhai automation script (needs the `rhai` feature):
cargo run -p armature-cli --features rhai -- script path/to/binary crates/armature-ext/scripts/summary.rhai
```

Feature flags (keep the default workspace build light):

| Flag    | Crate             | Enables                                              |
| ------- | ----------------- | ---------------------------------------------------- |
| `arm`   | `armature-cli`, `armature-disasm` | ARM / AArch64 disassembly (yaxpeax)         |
| `app`   | `armature-gui`    | Native `egui` GUI (also enables the `scripts` console) |
| `scripts` | `armature-gui`  | In-app Rhai script console (implies `app`)          |
| `rhai`  | `armature-cli`, `armature-ext` | Rhai scripting + `script` subcommand     |
| `wasm`  | `armature-cli`, `armature-ext` | Sandboxed Wasm plugins + `plugin` subcommand |

All flags are also available together via `cargo build --workspace --all-features`.

The GUI lives in `armature-gui`; build and run it with the `app` feature:

```sh
cargo run -p armature-gui --features app -- path/to/binary
```

Run a sandboxed Wasm plugin against a binary (needs the `wasm` feature):

```sh
cargo run -p armature-cli --features wasm -- plugin path/to/binary \
    crates/armature-ext/examples/plugins/hello/target/wasm32-unknown-unknown/release/armature_hello_plugin.wasm
```

### Demo with no sample binary required

`just demo` builds the CLI and analyzes the freshly built binary itself, so you
can see output immediately without supplying a target.

## Examples & templates

- Rhai scripts in [`crates/armature-ext/scripts/`](./crates/armature-ext/scripts):
  `auto_rename_printf`, `list_imports`, `find_crypto`, `rename_by_prefix`, `summary`.
- A Wasm plugin guest template in
  [`crates/armature-ext/examples/plugins/hello`](./crates/armature-ext/examples/plugins/hello).
- Getting-started and scripting guides in [`docs/`](./docs).

## Status

This is an active build following `todo.md`. The ingestion, IR, disassembly,
analysis, and CLI layers are functional for x86/x64. ARM/AArch64 disassembly is
available behind the `arm` feature, and the extension layer (Rhai / wasmtime)
behind the `rhai` and `wasm` features. Mach-O entry points and exports are now
resolved to virtual addresses.
