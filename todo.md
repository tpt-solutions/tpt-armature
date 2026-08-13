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

## Phase 8 — Bugs, gaps & adoption (backlog from review)

Tracking the items surfaced in the platform review: correctness bugs, missing
features, innovation ideas, and adoption blockers.

### Bugs (correctness)

- [x] Fix loop/back-edge counting in `cfg.rs`: `count_back_edges` over-reports
       loops (counts any `target_addr < from_start` edge, including `Call` and
       cross-function edges; ignores `blocks`/`addr_to_idx`). Restrict to in-
       function edges, exclude `Call`, or do dominator-based back-edge detection.
- [x] Cache the per-function CFG in the GUI instead of rebuilding it every frame
       (`armature-gui/src/app.rs` `render_graph` calls `build_cfg` + BFS layout on
       each `update()`). Rebuild only on function-selection change.
- [x] Offload analysis off the GUI UI thread (`app.rs` `load()` runs
       `analyze_binary` synchronously and freezes the window on large binaries).
       Use a background thread / `eframe` async poll with a progress state.
- [x] Make `Architecture::is_disassemblable()` feature-aware (currently returns
       `false` for ARM/AArch64 even when the `arm` feature provides a backend).
- [x] Handle indirect branches/calls in function recovery: `branch_target` only
       reads `Operand::Imm`, so `call rax`/`jmp [table]` ends recovery and
       fragments functions. Keep fallthrough for unresolved terminators and/or
       resolve jump tables.
- [x] Disassemble all executable sections, not just the first
       (`map.rs` `code_section()` returns the first executable section only).
       Add a section selector for multi-section / packed / Mach-O stub coverage.
- [x] Cap `disasm` output when no `-n` is given (`main.rs` dumps the entire
       instruction stream); mirror the `cfg` cap + notice behavior.
- [x] Document/strengthen `Instruction::defs()`/`uses()` imprecision (e.g.
       `mov [rax], rbx` wrongly reports `rax` as defined; `push`/`pop` ignore
       `rsp`; no flag registers).

### Missing features

- [x] Persist/export renames (rename + symbol annotations are in-memory only;
       add JSON/CSV/idc export and `analyze --json` for automation/CI).
- [ ] GUI goto-address, search, and keyboard navigation (only click nav exists).
- [ ] String and constant extraction pass.
- [ ] Debug-info import (PDB/DWARF) in `armature-formats` (currently only
      container symbols).
- [ ] Plugin auto-discovery / directory loading for Wasm plugins.
- [ ] Prebuilt-binary download link in README/PACKAGING (release artifacts).

### Adoption blockers (docs / CI / CLI)

- [x] Add a `plugin`/`wasm` CLI subcommand and enable `armature-ext/wasm` on
       `armature-cli` so the documented Wasm plugin ABI is actually runnable.
- [x] CI: add a `--all-features` build + clippy job and a `cargo test` job
       (currently only default features are built/linted; the `rhai` quick-start,
       ARM, Wasm, and GUI code are never verified).
- [x] Fix README feature-flag table: `-p armature-cli --features arm/wasm`
       fails (cli only exposes `rhai`); align docs or expose those features.
- [x] Consistent GUI feature naming (`app` vs `scripts`) across README,
       PACKAGING, justfile, and `just build-wasm-gui`.
- [x] Document the unreleased `function_count` Rhai binding in `docs/SCRIPTING.md`.

### Adoption — examples & templates

- [x] `just` recipe to build + run the `hello` Wasm plugin end-to-end.
- [ ] "5-minute quickstart" doc with a screenshot and expected `analyze` output
       using `examples/samples`.
- [ ] `templates/` area: "first analysis" Rhai cheat-sheet + a minimal annotated
       sample binary.
- [x] GUI "Open Sample" button that auto-loads `examples/samples`.

### Innovation (nice-to-have, differentiators)

- [ ] Pseudocode / decompiler view (IR -> C-like).
- [ ] Bindiff-style binary diffing between two builds.
- [ ] `armature watch` to re-analyze on rebuild.
- [ ] `armature serve` headless web UI.
- [ ] Shared script/template marketplace repo.
