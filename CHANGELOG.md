# Changelog

All notable changes to TPT Armature are documented here.

## [Unreleased]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
- `armature-formats`: PE/ELF/Mach-O parsing, architecture detection, `MemoryMap`.
- `armature-ir`: custom IR types and builder API.
- `armature-disasm`: iced-x86/x64 backend, yaxpeax ARM backend (feature `arm`).
- `armature-analysis`: control-flow graph, data-flow, cross-reference index.
- `armature-cli`: end-to-end headless pipeline driver with `analyze`, `disasm`,
  `cfg`, and `script` (feature `rhai`) subcommands.
- Recursive-descent **function recovery** (`armature-ir::recover_functions`): the
  code section is now split into proper functions from the entry point, exported
  symbols, and discovered call targets, with a linear-sweep fallback so no code
  is lost. Replaces the previous single linear-sweep function.
- `function_count` binding for Rhai scripts.
- GUI `Graph` view now renders an actual scrollable node-and-edge canvas (one
  function at a time, selected via a dropdown) instead of a raw edge list;
  large functions are capped at 600 blocks with a notice.
- GUI interactivity: clicking a graph node jumps the Assembly view to that
  block (highlighted); the Assembly view shows clickable X-refs (incoming
  references and branch/call targets) for the selected instruction. Graph edges
  now draw arrowheads.
- CLI `cfg` caps its edge dump at 200 lines (with an overflow notice) so it no
  longer prints the full ~167k-edge graph.
- `armature-gui`: panel layout + egui application (feature `app`).
- `armature-ext`: Rhai scripting host (feature `rhai`) and wasmtime plugin host
  (feature `wasm`) whose `rename` ABI now records renames.
- Rhai script templates (`scripts/`) and a Wasm plugin guest template
  (`examples/plugins/hello`).
- `just` recipes: `demo`, `smoke`, `script`, `build-wasm-example`.
- CI smoke test that analyzes the built binary.

### Fixed
- Clippy is now clean under `-D warnings` (CI will pass).
- Rhai `symbol_names` lookup: scripts now use the native `symbol_name(addr)`
  function; the `auto_rename_printf` example works end-to-end.
- Mach-O entry point is resolved to a virtual address and exports carry their
  real `n_value` address.
- Wasm plugin `rename` host function stores renames and `run()` returns them.
