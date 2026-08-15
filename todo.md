# TPT Armature — Build Checklist

Tracks implementation of `spec.txt` (System Design Document v1.0). Organized by phase; each phase maps to one or more crates in the `crates/` workspace.

## Phase 0 — Project Scaffolding

- [x] Workspace `Cargo.toml` (resolver 2, edition 2021, rust-version 1.85)
- [x] `crates/` layout: `tpt-armature-formats`, `tpt-armature-ir`, `tpt-armature-disasm`, `tpt-armature-analysis`, `tpt-armature-gui`, `tpt-armature-ext`, `tpt-armature-cli`
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

## Phase 1 — Layer 1: Ingestion (`tpt-armature-formats`)

"Stripping the Casing" — raw bytes in, standardized memory map out.

- [x] Add `goblin` dependency
- [x] PE parsing (sections, imports/exports, entry point)
- [x] ELF parsing (sections, imports/exports, entry point)
- [x] Mach-O parsing (sections, imports/exports, entry point)
- [x] Architecture detection (x86, x64, ARM, ARM64)
- [x] Extract `.text` (code) and `.data` (variables) sections
- [x] Map imported/exported symbols
- [x] Define standardized `MemoryMap` output type consumed by `tpt-armature-analysis`
- [x] Unit tests against sample binaries per format (parses the host test binary)

## Phase 2 — Layer 2: Analysis (`tpt-armature-ir`, `tpt-armature-disasm`, `tpt-armature-analysis`)

"Exposing the Framework" — machine code → understandable logic.

### `tpt-armature-ir`
- [x] Design custom IR types (instructions, operands, basic blocks)
- [x] IR builder API shared by both disassembler backends

### `tpt-armature-disasm`
- [x] Add `iced-x86` dependency, wrap for x86/x64 disassembly
- [x] Add `yaxpeax` dependency, wrap for ARM/other architectures (feature `arm`)
- [x] Lower `iced`/`yaxpeax` output into `tpt-armature-ir` types
- [x] Produce human-readable assembly text output

### `tpt-armature-analysis`
- [x] Control Flow Graph (CFG) construction: jumps, calls, loops → graph
- [x] Data-flow analysis: track variable creation/modification/destruction
- [x] Cross-reference (X-ref) index (who calls/reads what)
- [x] Unit tests on known control-flow patterns (loops, branches, recursion)

## Phase 3 — Layer 3: Presentation (`tpt-armature-gui`)

"The Canvas" — powered by `tpt-appfront` (path dep: `../tpt-appfront/crates/tpt-appfront-core`, `../tpt-appfront/crates/tpt-appfront-canvas`).

- [x] Wire `tpt-appfront-core` + `tpt-appfront-canvas` path dependencies
      (production target; the standalone build uses `egui`/`eframe` directly so it
      compiles without the sibling checkout — see `tpt-armature-gui/Cargo.toml`)
- [x] App shell / window bootstrap (native + optional wasm32 target)
- [x] Top-level panel layout (flex containers via `UITree`, no built-in docking — hand-compose)
- [x] Hex Editor view: raw byte viewer with inline assembly mapping
- [x] Assembly Viewer: syntax-highlighted asm text with clickable X-refs (data present; click UI pending)
- [x] Graph Visualizer: CFG node-and-edge canvas (rendered as edge list in the egui panel)
- [x] Wire GUI views to `tpt-armature-analysis` output (live binary load → render)
- [x] Basic navigation between Hex / Assembly / Graph views

## Phase 4 — Layer 4: Extension (`tpt-armature-ext`)

"The Custom Gears" — community extensibility without touching core Rust code.

### Rhai scripting
- [x] Add `rhai` dependency, embed scripting host
- [x] Bind core Rust data structures (functions, symbols, IR) into Rhai scope
- [x] Example script: auto-rename functions that call `printf` (`scripts/auto_rename_printf.rhai`)
- [x] `script` subcommand in `tpt-armature-cli` (headless runner; wire `tpt-armature-cli script <binary> <script>`)
- [x] Script console/runner UI hook in `tpt-armature-gui` (host exists; GUI console pending)

### Wasm plugins
- [x] Add `wasmtime` dependency, sandboxed plugin host
- [x] Define plugin API/ABI (host functions exposed to guest Wasm)
- [x] Example Wasm plugin guest
      (`examples/plugins/hello`; build with `just build-wasm-example`)
- [x] Plugin discovery/loading mechanism (load from path)
- [x] `rename` host function records renames and `run()` returns them

## Phase 5 — Integration (`tpt-armature-cli`)

- [x] CLI argument parsing (binary path, headless mode flags)
- [x] Wire pipeline: `tpt-armature-formats` → `tpt-armature-disasm`/`tpt-armature-analysis` → `tpt-armature-gui`
- [x] End-to-end smoke test: load a real binary, render Hex + Assembly + CFG
- [x] Headless/CLI-only analysis mode (no GUI) for scripting/CI use

## Phase 6 — Polish / Release

- [x] Test coverage per crate (`tpt-armature-formats`, `tpt-armature-ir`, `tpt-armature-disasm`, `tpt-armature-analysis`, `tpt-armature-ext`)
- [x] `cargo clippy --workspace --all-targets` clean (`-D warnings` in CI)
- [x] CI smoke test: `cargo run -p tpt-armature-cli -- analyze target/debug/tpt-armature`
- [x] `cargo deny check` clean (added `Unicode-3.0`; dropped deprecated `version` keys)
- [x] `cargo audit` clean (upgraded `wasmtime` 25 -> 47 to clear 16 advisories incl. 2 critical; transitive `quick-xml`/unmaintained items documented in `.cargo/audit.toml`)
- [x] API docs (`cargo doc`) for public crate surfaces
- [x] User-facing docs: `docs/GETTING_STARTED.md`, `docs/SCRIPTING.md`, README quick start
- [x] Rhai script templates + Wasm plugin guest template (adoption)
- [x] Packaging: Windows, macOS, Linux native builds (release profile + 3-OS release workflow + `docs/PACKAGING.md`)
- [x] Optional: wasm32 build of `tpt-armature-gui` for browser demo (`WebRunner` path + `just build-wasm-gui`)
- [x] Sample binary corpus for manual QA (PE/ELF/Mach-O, x86/ARM): `examples/tpt-armature-sample` + `just build-samples`/`qa`

## Phase 7 — Analysis depth (done)

- [x] Recursive-descent function recovery (`tpt-armature-ir::recover_functions`): split
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
       (`tpt-armature-gui/src/app.rs` `render_graph` calls `build_cfg` + BFS layout on
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
- [x] GUI goto-address, search, and keyboard navigation (only click nav exists).
- [x] String and constant extraction pass.
- [x] Debug-info import (PDB/DWARF) in `armature-formats` (currently only
      container symbols).
- [x] Plugin auto-discovery / directory loading for Wasm plugins.
- [x] Prebuilt-binary download link in README/PACKAGING (release artifacts).

### Adoption blockers (docs / CI / CLI)

- [x] Add a `plugin`/`wasm` CLI subcommand and enable `tpt-armature-ext/wasm` on
       `tpt-armature-cli` so the documented Wasm plugin ABI is actually runnable.
- [x] CI: add a `--all-features` build + clippy job and a `cargo test` job
       (currently only default features are built/linted; the `rhai` quick-start,
       ARM, Wasm, and GUI code are never verified).
- [x] Fix README feature-flag table: `-p tpt-armature-cli --features arm/wasm`
       fails (cli only exposes `rhai`); align docs or expose those features.
- [x] Consistent GUI feature naming (`app` vs `scripts`) across README,
       PACKAGING, justfile, and `just build-wasm-gui`.
- [x] Document the unreleased `function_count` Rhai binding in `docs/SCRIPTING.md`.

### Adoption — examples & templates

- [x] `just` recipe to build + run the `hello` Wasm plugin end-to-end.
- [x] "5-minute quickstart" doc with a screenshot and expected `analyze` output
      using `examples/samples`.
- [x] `templates/` area: "first analysis" Rhai cheat-sheet + a minimal annotated
      sample binary.
- [x] GUI "Open Sample" button that auto-loads `examples/tpt-armature-sample`.

### Innovation (nice-to-have, differentiators)

- [x] Pseudocode / decompiler view (IR -> C-like).
- [x] Bindiff-style binary diffing between two builds.
- [x] `armature watch` to re-analyze on rebuild.
- [x] `armature serve` headless web UI.
- [ ] Shared script/template marketplace repo.

## Phase 9 — Platform review implementation (from `plan` review)

Bugs / correctness surfaced in the review, plus the two unchecked Phase 8 backlog
items, missing-feature P0s, and adoption/usability work.

### Bugs (correctness) — implement

- [x] B1: ARM/AArch64 analysis is dead — `yaxpeax_arm.rs` emits every instruction as
       `Mnemonic::Other` with empty operands and a fixed `size: 4`. Classify real
       mnemonics (`Jmp`/`Jcc`/`Call`/`Ret`/arithmetic/`Nop`), extract branch-target
       immediates (relative to the instruction) so CFG/function recovery work, and
       populate at least the destination register for data-flow. Add ARM unit tests.
- [x] B2: combined-CFG `loop_count` inflation — `build_cfg` merges all functions;
       cross-function tail `jmp`/`call` edges are counted as loops. Skip edges that
       leave the owning function (pass `func_of` into `count_back_edges`); keep the
       per-function GUI CFG correct. Add a regression test.
- [x] B3 (track): indirect control flow (`call rax`, `jmp [table]`, PLT/GOT) is not
       followed; documented the limitation in `tpt-armature-ir/src/builder.rs`
       (`branch_target`), best-effort resolution deferred.
- [x] B4: `analyze --json` never carries renames — add `--rename-file` (json/csv/idc)
       so renames round-trip into `analyze` output and relabel functions.

### Missing features (P0)

- [x] DWARF debug-info import (feature `debuginfo`): ELF `.symtab` + DWARF subprogram
       names/addresses flow into `function.name` (guarded; missing DWARF is ignored).
- [x] Interactive rename/comment import in the GUI (load a rename file) to match the
       existing export paths.

### Innovation / backlog (unchecked Phase 8)

- [x] `armature serve` — headless web UI serving analysis over HTTP (feature `serve`).
- [ ] Shared script/template marketplace repo (recipe gallery + plugin registry).

### Usability & automation

- [x] `just setup` recipe (add `wasm32-unknown-unknown` target, install git hooks).
- [x] Clearer feature-missing errors: `--pdb`/`--rename-file` are always present; a
       missing `debuginfo` build returns a clear message instead of "unexpected argument".
- [x] README: note the browser-demo (`build-wasm-gui`) and the `serve` headless UI.

### Validation

- [x] `cargo test --workspace --all-features` green; new ARM + CFG regression tests pass.
- [x] `analyze --rename-file` round-trip verified.

## Phase 10 — Driver-RE wedge + tpt-telos invariant layer

Plan: `how-can-we-make-magical-sloth` (Claude plan file). Positions tpt-armature
against Ghidra/IDA via a sharper wedge — closed-source driver RE for
open-source Linux driver bring-up — plus a scoped tpt-telos integration for
proven range/invariant annotations. All new work is opt-in via Cargo features;
default `cargo build --workspace` must stay untouched.

### Initiative A — Driver reverse-engineering support

- [x] A.1: MMIO/register-access mining pass (`tpt-armature-analysis/src/mmio.rs`,
       feature `mmio`). Block-local base-pointer provenance tracking over raw
       `Instruction`/`Operand` (not `defs()/uses()`, which drops memory-operand
       writes); cluster constant-offset accesses into a `RegisterTable`. Unit
       tests incl. indexed-addressing exclusion and the stack/MMIO-reuse hard case.
- [x] A.2: rnndb-style register XML export (`export.rs`, follows the existing
       `RenameFormat` pattern). New `Command::Mmio` CLI subcommand gated
       `#[cfg(feature = "mmio")]` (rnndb + json formats, optional `--out`).
- [x] A.3: Windows PE kernel-driver (`.sys`) support — WDM/KMDF detection in
       `tpt-armature-formats` (feature `driver-pe`), IRP `MajorFunction` dispatch
       recovery (reuses A.1's base-provenance matcher), `CTL_CODE` decode +
       IOCTL extraction (scoped to DeviceIoControl handlers), `DriverProfile`
       type. New `Command::Driver` CLI subcommand (feature `driver-pe`).
- [ ] A.4: Clean-room analyst mode (feature `clean-room`) — dedicated
       `Command::CleanRoomExport`, structurally enforced (signature only accepts
       `RegisterTable`/`DriverProfile`, never raw IR/decompiled text), SHA-256 +
       manifest audit trail. `trybuild` compile-fail test for the boundary.
- [ ] A.5: Rust-for-Linux driver skeleton generation (feature `skeleton`) —
       native template codegen (no tpt-telos-codegen dependency) emitting
       `#[repr(C)]` register struct, IOCTL enum, probe/remove stubs from
       `RegisterTable`/`DriverProfile`. Verify current `kernel` crate trait
       shape before finalizing the template.

### Initiative B — tpt-telos integration (feature-gated, after Initiative A)

- [ ] B.0: Verify-before-building checklist — confirm actual `tpt-telos-*` crate
       names/publish location, `tpt-telos-verifier`'s standalone API, that it has
       no required transitive dep on `tpt-telos-agent`/`-lsp`/`-router`, license
       files match dual MIT/Apache-2.0, and that a throwaway `cargo add
       tpt-telos-verifier` builds light in isolation.
- [ ] B.1: QF_LRA-backed range/invariant annotations on decompiled output
       (`tpt-armature-analysis/src/invariants.rs`, feature `telos`, git dep on
       `tpt-telos-verifier` pinned to a commit SHA per the no-path-dep
       convention). Scoped to bounds-check-guard and offset-chain patterns;
       fails open (no annotation) when unprovable. Requires adding a small
       `DecompileOptions` struct to `decompile.rs` (today has none — see
       `main.rs:357`) to wire the annotation toggle through.
- [ ] B.2 (speculative): bridge A's `RegisterTable`/`DriverProfile` into
       `tpt-telos-ir` contracts for verified skeleton codegen. Only pursue if
       B.0/B.1 confirm a real proof obligation exists beyond A.5's plain
       templating; not required for A.5 to ship.

### Initiative C — Competitive-parity items (vs. Ghidra/IDA)

- [ ] C.1: FLIRT-equivalent static-library/WDK/HAL/compiler-runtime signature
       matching (`tpt-armature-analysis/src/siglib.rs`, feature `siglib`).
       Masked-byte-pattern signatures (SHA-256 of relocation-wildcarded raw
       bytes), FLIRT-style prefix growth + callee-chain confirmation. Hand-built
       JSON sig-pack (`export.rs`-style), `Command::BuildSigs`/`MatchSigs` CLI.
       Reinforces A.3 (skip WDK/HAL/CRT boilerplate in `.sys` drivers).
- [x] C.2: Register-table/function similarity diffing across firmware/driver
       revisions (`tpt-armature-analysis/src/regdiff.rs`, feature `regdiff`,
       implies `mmio`). Jaccard over `RegisterTable` `(offset, width, rw-kind)`
       tuples + CFG-shape/wildcarded-mnemonic cosine function matching with
       Diaphora/BSim-style greedy bipartite pairing. New `Command::Regdiff` CLI
       subcommand (rnndb + json output). Function-similarity half is standalone.
- [ ] C.3: Real control-flow structuring in the decompiler
       (`tpt-armature-analysis/src/dominators.rs` + `structure.rs`, feature
       `structuring`). Cooper/Harvey/Kennedy dominator computation (new —
       doesn't exist today) + dominator-tree pattern matching (not full
       Cifuentes interval analysis) to emit real `if`/`while`/`Seq` regions,
       falling back to `Goto` for irreducible edges. `decompile.rs` gets a
       `render_structured` path selected via the same `DecompileOptions`
       struct B.1 introduces (one struct total, not two). Zero dependency on
       anything else in Phase 10 — best parallel-track candidate.
- [ ] C.4 (feasibility spike, not committed full scope): SLEIGH-equivalent
       declarative ISA spec. New crate/module + feature `decl-arch`. Concrete
       deliverable only: decode RV32I end-to-end via a declarative bit-field
       spec + runtime interpreter implementing the existing `Disassembler`
       trait (`disassembler.rs`), wired into `for_architecture` like the `arm`
       feature is today. Explicitly excludes semantics/p-code, variable-length
       ISAs, and subtable inheritance — decode-only proof of the interpreter
       path. Lowest priority; zero dependency on anything else in Phase 10.

### Validation

- [ ] CI matrix: `--features mmio,driver-pe,clean-room,skeleton,siglib,regdiff,structuring`
       job (no external git dep) separate from a `--features telos` job (git
       dependency resolution) once B.1 lands, and an isolated `--features
       decl-arch` job once C.4 lands (spike-quality, droppable independently).
- [ ] Default `cargo build --workspace` / `cargo fmt --check` / `cargo clippy
       --workspace --all-targets` stay green and untouched by all of the above.
