# TPT Armature — Build Checklist

Tracks implementation of `spec.txt` (System Design Document v1.0). Organized by phase; each phase maps to one or more crates in the `crates/` workspace.

## Phase 0 — Project Scaffolding

- [x] Workspace `Cargo.toml` (resolver 2, edition 2021, rust-version 1.85)
- [x] `crates/` layout: `armature-formats`, `armature-ir`, `armature-disasm`, `armature-analysis`, `armature-gui`, `armature-ext`, `armature-cli`
- [x] `LICENSE-MIT` + `LICENSE-APACHE` (dual license, matches sibling tpt-* convention)
- [x] `README.md` (project overview, links to spec.txt)
- [x] `CHANGELOG.md`
- [x] `CLAUDE.md` (repo conventions for future agent sessions)
- [x] `rustfmt.toml`
- [x] `deny.toml` / `audit.toml` (cargo-deny / cargo-audit config)
- [x] `justfile` (build/test/fmt/lint recipes)
- [x] `.github/workflows/ci.yml` (fmt check, build, docs build — mirrors tpt-appfront CI)
- [ ] `cargo build --workspace` succeeds on stub crates
- [ ] `cargo fmt --check` passes

## Phase 1 — Layer 1: Ingestion (`armature-formats`)

"Stripping the Casing" — raw bytes in, standardized memory map out.

- [ ] Add `goblin` dependency
- [ ] PE parsing (sections, imports/exports, entry point)
- [ ] ELF parsing (sections, imports/exports, entry point)
- [ ] Mach-O parsing (sections, imports/exports, entry point)
- [ ] Architecture detection (x86, x64, ARM, ARM64)
- [ ] Extract `.text` (code) and `.data` (variables) sections
- [ ] Map imported/exported symbols
- [ ] Define standardized `MemoryMap` output type consumed by `armature-analysis`
- [ ] Unit tests against sample binaries per format

## Phase 2 — Layer 2: Analysis (`armature-ir`, `armature-disasm`, `armature-analysis`)

"Exposing the Framework" — machine code → understandable logic.

### `armature-ir`
- [ ] Design custom IR types (instructions, operands, basic blocks)
- [ ] IR builder API shared by both disassembler backends

### `armature-disasm`
- [ ] Add `iced-x86` dependency, wrap for x86/x64 disassembly
- [ ] Add `yaxpeax` dependency, wrap for ARM/other architectures
- [ ] Lower `iced`/`yaxpeax` output into `armature-ir` types
- [ ] Produce human-readable assembly text output

### `armature-analysis`
- [ ] Control Flow Graph (CFG) construction: jumps, calls, loops → graph
- [ ] Data-flow analysis: track variable creation/modification/destruction
- [ ] Cross-reference (X-ref) index (who calls/reads what)
- [ ] Unit tests on known control-flow patterns (loops, branches, recursion)

## Phase 3 — Layer 3: Presentation (`armature-gui`)

"The Canvas" — powered by `tpt-appfront` (path dep: `../tpt-appfront/crates/tpt-appfront-core`, `../tpt-appfront/crates/tpt-appfront-canvas`).

- [ ] Wire `tpt-appfront-core` + `tpt-appfront-canvas` path dependencies
- [ ] App shell / window bootstrap (native + optional wasm32 target)
- [ ] Top-level panel layout (flex containers via `UITree`, no built-in docking — hand-compose)
- [ ] Hex Editor view: raw byte viewer with inline assembly mapping
- [ ] Assembly Viewer: syntax-highlighted asm text with clickable X-refs
- [ ] Graph Visualizer: CFG node-and-edge canvas, built via the raw-egui escape hatch (reference: `tpt-appfront/examples/node-graph`)
- [ ] Wire GUI views to `armature-analysis` output (live binary load → render)
- [ ] Basic navigation between Hex / Assembly / Graph views

## Phase 4 — Layer 4: Extension (`armature-ext`)

"The Custom Gears" — community extensibility without touching core Rust code.

### Rhai scripting
- [ ] Add `rhai` dependency, embed scripting host
- [ ] Bind core Rust data structures (functions, symbols, IR) into Rhai scope
- [ ] Example script: auto-rename functions that call `printf`
- [ ] Script console/runner UI hook in `armature-gui`

### Wasm plugins
- [ ] Add `wasmtime` dependency, sandboxed plugin host
- [ ] Define plugin API/ABI (host functions exposed to guest Wasm)
- [ ] Example Wasm plugin (e.g. minimal unpacker or debugger stub)
- [ ] Plugin discovery/loading mechanism

## Phase 5 — Integration (`armature-cli`)

- [ ] CLI argument parsing (binary path, headless mode flags)
- [ ] Wire pipeline: `armature-formats` → `armature-disasm`/`armature-analysis` → `armature-gui`
- [ ] End-to-end smoke test: load a real binary, render Hex + Assembly + CFG
- [ ] Headless/CLI-only analysis mode (no GUI) for scripting/CI use

## Phase 6 — Polish / Release

- [ ] Test coverage per crate (`armature-formats`, `armature-ir`, `armature-disasm`, `armature-analysis`, `armature-ext`)
- [ ] `cargo deny check` clean
- [ ] `cargo audit` clean
- [ ] API docs (`cargo doc`) for public crate surfaces
- [ ] User-facing docs: getting started, Rhai scripting guide, Wasm plugin guide
- [ ] Packaging: Windows, macOS, Linux native builds
- [ ] Optional: wasm32 build of `armature-gui` for browser demo
- [ ] Sample binary corpus for manual QA (PE/ELF/Mach-O, x86/ARM)
