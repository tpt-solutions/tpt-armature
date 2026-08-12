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
```

The GUI lives in `armature-gui`; build and run it with the `app` feature:

```sh
cargo run -p armature-gui --features app -- path/to/binary
```

## Status

This is an active build following `todo.md`. The ingestion, IR, disassembly,
analysis, and CLI layers are functional for x86/x64. ARM/AArch64 disassembly is
available behind the `arm` feature, and the extension layer (Rhai / wasmtime)
behind the `rhai` and `wasm` features.
