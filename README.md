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
| `tpt-armature-formats`   | 1     | Binary parsing (PE/ELF/Mach-O) -> standardized `MemoryMap` |
| `tpt-armature-ir`        | 2     | Custom IR (instructions, operands, basic blocks) + builder |
| `tpt-armature-disasm`    | 2     | Disassembly backends (iced-x86/x64, yaxpeax ARM) -> IR     |
| `tpt-armature-analysis`  | 2     | CFG construction, data-flow, cross-reference index        |
| `tpt-armature-gui`       | 3     | Presentation (hex / assembly / graph views)               |
| `tpt-armature-ext`       | 4     | Rhai scripting + wasmtime sandboxed plugins                |
| `tpt-armature-cli`       | 5     | Headless pipeline + CLI driver                            |

## Quick start

```sh
cargo build --workspace
cargo run -p tpt-armature-cli -- analyze path/to/binary
cargo run -p tpt-armature-cli -- disasm path/to/binary
cargo run -p tpt-armature-cli -- strings path/to/binary      # strings + constants
cargo run -p tpt-armature-cli -- decompile path/to/binary     # C-like pseudocode
# Run a Rhai automation script (needs the `rhai` feature):
cargo run -p tpt-armature-cli --features rhai -- script path/to/binary crates/tpt-armature-ext/scripts/summary.rhai
```

For the full command reference and a walkthrough, see
[`docs/QUICKSTART.md`](./docs/QUICKSTART.md) and
[`docs/GETTING_STARTED.md`](./docs/GETTING_STARTED.md).

Feature flags (keep the default workspace build light):

| Flag       | Crate             | Enables                                              |
| ---------- | ----------------- | ---------------------------------------------------- |
| `arm`      | `tpt-armature-cli`, `tpt-armature-disasm` | ARM / AArch64 disassembly (yaxpeax)         |
| `app`      | `tpt-armature-gui`    | Native `egui` GUI (also enables the `scripts` console) |
| `scripts`  | `tpt-armature-gui`    | In-app Rhai script console (implies `app`)          |
| `rhai`     | `tpt-armature-cli`, `tpt-armature-ext` | Rhai scripting + `script` subcommand     |
| `wasm`     | `tpt-armature-cli`, `tpt-armature-ext` | Sandboxed Wasm plugins + `plugin`/`plugins` subcommands |
| `debuginfo`| `tpt-armature-cli`, `tpt-armature-formats` | ELF `.symtab` + PE PDB symbol import (`analyze --pdb`) |

All flags are also available together via `cargo build --workspace --all-features`.

The GUI lives in `tpt-armature-gui`; build and run it with the `app` feature:

```sh
cargo run -p tpt-armature-gui --features app -- path/to/binary
```

Run a sandboxed Wasm plugin against a binary (needs the `wasm` feature):

```sh
cargo run -p tpt-armature-cli --features wasm -- plugin path/to/binary \
    crates/tpt-armature-ext/examples/plugins/tpt-armature-hello-plugin/target/wasm32-unknown-unknown/release/tpt_armature_hello_plugin.wasm
```

### Demo with no sample binary required

`just demo` builds the CLI and analyzes the freshly built binary itself, so you
can see output immediately without supplying a target.

## Prebuilt binaries

CI produces release artifacts automatically:

- **GitHub Releases** — pushing a `v*` tag triggers
  [`.github/workflows/release.yml`](./.github/workflows/release.yml), which
  builds `tpt-armature` (CLI, `rhai` feature) and `tpt-armature-gui` (`scripts` feature)
  on Windows, macOS, and Linux and attaches the stripped binaries to the
  release.
- **Workflow artifacts** — the same job uploads the binaries as downloadable
  artifacts on every manual run (Actions → Release → run workflow), useful for
  grabbing a preview build without a tag.

Download the archive for your OS, extract it, and run `./tpt-armature analyze
<binary>`. The GUI artifact is `tpt-armature-gui` (run `./tpt-armature-gui <binary>`).

## Examples & templates

- Rhai scripts in [`crates/tpt-armature-ext/scripts/`](./crates/tpt-armature-ext/scripts):
  `auto_rename_printf`, `list_imports`, `find_crypto`, `rename_by_prefix`, `summary`.
- A Wasm plugin guest template in
  [`crates/tpt-armature-ext/examples/plugins/tpt-armature-hello-plugin`](./crates/tpt-armature-ext/examples/plugins/tpt-armature-hello-plugin).
- A first-analysis Rhai script and Rhai API cheat-sheet in
  [`templates/`](./templates).
- Getting-started, scripting, and quickstart guides in [`docs/`](./docs).

## Status

This is an active build following `todo.md`. The ingestion, IR, disassembly,
analysis, and CLI layers are functional for x86/x64. ARM/AArch64 disassembly is
available behind the `arm` feature, and the extension layer (Rhai / wasmtime)
behind the `rhai` and `wasm` features. Mach-O entry points and exports are now
resolved to virtual addresses.

Recent additions (see `todo.md`): string/constant extraction
(`tpt-armature strings`), an IR→C-like pseudocode view (`tpt-armature decompile` and the
GUI's Pseudocode panel), debug-information import for ELF/PDB
(`--features debuginfo`, `analyze --pdb`), Wasm plugin directory auto-discovery
(`tpt-armature plugins`), and `tpt-armature watch` for re-analysis on rebuild. The GUI
adds goto-address, search, keyboard navigation, a Strings panel, and the
Pseudocode panel.
