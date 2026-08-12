# Changelog

All notable changes to TPT Armature are documented here.

## [Unreleased]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
- `armature-formats`: PE/ELF/Mach-O parsing, architecture detection, `MemoryMap`.
- `armature-ir`: custom IR types and builder API.
- `armature-disasm`: iced-x86/x64 backend, yaxpeax ARM backend (feature `arm`).
- `armature-analysis`: control-flow graph, data-flow, cross-reference index.
- `armature-cli`: end-to-end headless pipeline driver.
- `armature-gui`: panel layout + egui application (feature `app`).
- `armature-ext`: Rhai scripting host (feature `rhai`) and wasmtime plugin host
  (feature `wasm`).
