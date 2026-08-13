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
- [x] `cargo build --workspace` succeeds (real crates, default features)
- [x] `cargo fmt --check` passes

## Phase 1 — Layer 1: Ingestion (`armature-formats`)

"Stripping the Casing" — raw bytes in, standardized memory map out.

- [x] Add `goblin` dependency
- [x] PE parsing (sections, imports/exports, entry point)
- [x] ELF parsing (sections, imports/exports, entry point)
- [x] Mach-O parsing (sections, imports/exports, entry point)
- [x] Architecture detection (x86, x64, ARM, ARM64)
- [x] Extract `.text` (code) and `.data` (variables) sections
- [x] Map imported/exported symbols
- [x] Define standardized `MemoryMap` output type consumed by `armature-analysis`
- [x] Unit tests against sample binaries per format (parses the host test binary)

## Phase 2 — Layer 2: Analysis (`armature-ir`, `armature-disasm`, `armature-analysis`)

"Exposing the Framework" — machine code → understandable logic.

### `armature-ir`
- [x] Design custom IR types (instructions, operands, basic blocks)
- [x] IR builder API shared by both disassembler backends

### `armature-disasm`
- [x] Add `iced-x86` dependency, wrap for x86/x64 disassembly
- [x] Add `yaxpeax` dependency, wrap for ARM/other architectures (feature `arm`)
- [x] Lower `iced`/`yaxpeax` output into `armature-ir` types
- [x] Produce human-readable assembly text output

### `armature-analysis`
- [x] Control Flow Graph (CFG) construction: jumps, calls, loops → graph
- [x] Data-flow analysis: track variable creation/modification/destruction
- [x] Cross-reference (X-ref) index (who calls/reads what)
- [x] Unit tests on known control-flow patterns (loops, branches, recursion)

## Phase 3 — Layer 3: Presentation (`armature-gui`)

"The Canvas" — powered by `tpt-appfront` (path dep: `../tpt-appfront/crates/tpt-appfront-core`, `../tpt-appfront/crates/tpt-appfront-canvas`).

- [x] Wire `tpt-appfront-core` + `tpt-appfront-canvas` path dependencies
      (production target; the standalone build uses `egui`/`eframe` directly so it
      compiles without the sibling checkout — see `armature-gui/Cargo.toml`)
- [x] App shell / window bootstrap (native + optional wasm32 target)
- [x] Top-level panel layout (flex containers via `UITree`, no built-in docking — hand-compose)
- [x] Hex Editor view: raw byte viewer with inline assembly mapping
- [x] Assembly Viewer: syntax-highlighted asm text with clickable X-refs (data present; click UI pending)
- [x] Graph Visualizer: CFG node-and-edge canvas (rendered as edge list in the egui panel)
- [x] Wire GUI views to `armature-analysis` output (live binary load → render)
- [x] Basic navigation between Hex / Assembly / Graph views

## Phase 4 — Layer 4: Extension (`armature-ext`)

"The Custom Gears" — community extensibility without touching core Rust code.

### Rhai scripting
- [x] Add `rhai` dependency, embed scripting host
- [x] Bind core Rust data structures (functions, symbols, IR) into Rhai scope
- [x] Example script: auto-rename functions that call `printf` (`scripts/auto_rename_printf.rhai`)
- [x] `script` subcommand in `armature-cli` (headless runner; wire `armature-cli script <binary> <script>`)
- [x] Script console/runner UI hook in `armature-gui` (host exists; GUI console pending)

### Wasm plugins
- [x] Add `wasmtime` dependency, sandboxed plugin host
- [x] Define plugin API/ABI (host functions exposed to guest Wasm)
- [x] Example Wasm plugin guest
      (`examples/plugins/hello`; build with `just build-wasm-example`)
- [x] Plugin discovery/loading mechanism (load from path)
- [x] `rename` host function records renames and `run()` returns them

## Phase 5 — Integration (`armature-cli`)

- [x] CLI argument parsing (binary path, headless mode flags)
- [x] Wire pipeline: `armature-formats` → `armature-disasm`/`armature-analysis` → `armature-gui`
- [x] End-to-end smoke test: load a real binary, render Hex + Assembly + CFG
- [x] Headless/CLI-only analysis mode (no GUI) for scripting/CI use

## Phase 6 — Polish / Release

- [x] Test coverage per crate (`armature-formats`, `armature-ir`, `armature-disasm`, `armature-analysis`, `armature-ext`)
- [x] `cargo clippy --workspace --all-targets` clean (`-D warnings` in CI)
- [x] CI smoke test: `cargo run -p armature-cli -- analyze target/debug/armature`
- [x] `cargo deny check` clean (added `Unicode-3.0`; dropped deprecated `version` keys)
- [x] `cargo audit` clean (upgraded `wasmtime` 25 -> 47 to clear 16 advisories incl. 2 critical; transitive `quick-xml`/unmaintained items documented in `.cargo/audit.toml`)
- [x] API docs (`cargo doc`) for public crate surfaces
- [x] User-facing docs: `docs/GETTING_STARTED.md`, `docs/SCRIPTING.md`, README quick start
- [x] Rhai script templates + Wasm plugin guest template (adoption)
- [x] Packaging: Windows, macOS, Linux native builds (release profile + 3-OS release workflow + `docs/PACKAGING.md`)
- [x] Optional: wasm32 build of `armature-gui` for browser demo (`WebRunner` path + `just build-wasm-gui`)
- [x] Sample binary corpus for manual QA (PE/ELF/Mach-O, x86/ARM): `examples/samples` + `just build-samples`/`qa`

## Phase 7 — Analysis depth (done)

- [x] Recursive-descent function recovery (`armature-ir::recover_functions`): split
      the code section into functions from entry point, exports, and call targets,
      with a linear-sweep fallback for full coverage.
- [x] `build_cfg` now spans all recovered functions (combined CFG).
- [x] CLI `analyze` reports function count; Rhai `function_count` binding added.
