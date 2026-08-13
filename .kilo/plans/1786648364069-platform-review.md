# TPT Armature — Platform Review (Bugs · Todos · Missing Features · Innovation · Adoption)

Scope: full workspace review against `todo.md` (Phase 8). The project is mature and
well-tested for **x86/x64**; most Phase 0–8 items are genuinely done. This document
captures what remains: real correctness gaps, the two unchecked backlog items,
missing capabilities, innovation, and adoption/usability improvements.

---

## 1. Bugs / Correctness (highest priority)

### B1 — ARM/AArch64 analysis is effectively dead  ⛔
`crates/tpt-armature-disasm/src/yaxpeax_arm.rs:43` emits **every** instruction as
`Mnemonic::Other(mnem)` with `operands: Vec::new()` and a hardcoded `size: 4`.
Consequences for ARM/AArch64:
- CFG (`cfg.rs`) never sees `Jmp`/`Jcc`/`Call`/`Ret`, so **no edges, no loops**.
- `recover_functions` (`builder.rs`) never splits on calls/branches → **one giant function**.
- Decompiler (`decompile.rs`), data-flow `defs()`/`uses()`, and X-refs are all **no-ops**.
- `size: 4` fixed → **Thumb (16-bit) and mixed T32 are mis-decoded**; only straight A32/A64.
So the `arm` feature "works" only in the sense that it prints text. This is the most
important gap. Fix: lower yaxpeax decoded instructions into real `Mnemonic` variants
(classify by mnemonic string: `b`/`bl`→Jmp/Call, `ret`/`bx`/`bx lr`→Ret, `cmp`/`mov`/…→
arithmetic, `b.eq`/`b.ne`→`Jcc`), populate `Operand`s (reg/mem/imm), and step by the
**actual** decoded length (`insn.len()`), not a constant 4.

### B2 — Combined CFG `loop_count` can be inflated (CLI only)
`analysis.rs::build_cfg` (`cfg.rs:167`) concatenates *all* functions' blocks into one
`Cfg`. `from_blocks` adds an edge whenever a branch/call target address exists in the
combined `addr_to_idx`, including **cross-function** tail `jmp`/`call`. `count_back_edges`
skips `Call` edges but counts cross-function unconditional jumps to a lower node index
as loops. Reported by `analyze`/`cfg` subcommands. Per-function GUI CFG is correct
(it rebuilds per function). Fix: build the combined graph but **exclude edges that leave
the function** when counting loops, or only count back-edges within the same function.

### B3 — Indirect control flow not followed
`branch_target` (`builder.rs:286`, `cfg.rs:175`) only reads `Operand::Imm`. `call rax`,
`jmp [table]`, and PLT/GOT thunks end recovery / leave callees unrecoverable. The
fallthrough-keep mitigation helps a little but **indirect call targets are never
recovered as functions**. Enhancement (not strictly a bug): best-effort PLT/GOT and
jump-table resolution, or at minimum document the limitation.

### B4 — `analyze --json` never carries renames
`cli/main.rs:205` always passes `&HashMap::new()`. Renames only exist after `script`/
`plugin`. For CI automation that runs `analyze` then exports, renames can't round-trip.
Consider loading a rename file into `analyze` (`--rename-file`).

### B5 — Address types use `i64` in the extension layer
Rhai `rename(addr: i64, …)`, `symbol_name(addr: i64)`, and Wasm `rename(addr: i64, …)`
overflow for addresses ≥ 2⁶³ (kernel / HIGH_ADDR space). Edge case for userland today,
but should use `u64` (or document). Low priority.

---

## 2. Remaining todos (`todo.md` Phase 8, unchecked)

- [ ] `armature serve` — headless web UI (serve analysis over HTTP / embed the GUI).
- [ ] Shared script/template marketplace repo (community recipes + plugin registry).

---

## 3. Missing features (prioritized)

### P0
- **DWARF debug info** — `debuginfo.rs` only does PDB + inline ELF `.symtab`. The
  Phase 8 line "PDB/DWARF" is half-done. DWARF gives vastly better function/type/var
  names for unstripped ELF/obj.
- **Interactive GUI renaming & comments** — renames only come from the script console
  (`app.rs`); no click-to-rename or comment UI, and no **import/load** of rename files
  (we can export JSON/CSV/IDC but never reload). Add `analyze --rename-file` + GUI edit.
- **Project/session save-load** — persist analysis (rename/comment/selection) to disk.

### P1
- **Pseudocode control-flow structuring** — `decompile.rs` is linear (block-by-block).
  Add `if`/`while`/`for` reconstruction and variable typing from data-flow.
- **Stack frame / local variable recovery** — currently no SP/frame analysis; `push`/
  `pop` ignore `rsp` (`instr.rs:174`).
- **Hex-view search & cross-highlight** — `search` works only in Assembly; add hex search
  and "follow in hex from asm" / "follow in asm from hex".
- **Recent files + drag-and-drop open** in GUI.

### P2
- **Relocation / PLT / GOT resolution** for external calls (better X-refs than immediate match).
- **Mach-O fat multi-arch selection** (currently first slice only, `parse.rs:220`).
- **Diff export / baseline pinning** for `diff` (currently stdout only).
- **Watch-diff between two build artifacts over time** (combine `watch` + `diff`).

---

## 4. Innovation / Differentiators

- **`armature serve`** (unchecked) — a headless web UI makes the tool usable from CI
  containers and for collaboration; high adoption value.
- **Shared template marketplace** (unchecked) — recipe gallery + plugin registry.
- **AI-assisted summarization (2026-relevant)**: feed a function's pseudocode to an LLM
  and attach a natural-language summary/comment. Cheap to add behind the existing
  `script`/`ext` layer; strong differentiator vs. Ghidra/r2.
- **Import/export interoperability**: emit/consume Ghidra XML, radare2 (`r2`/`sdb`),
  Binary Ninja MLIL-ish JSON — lowers switching cost.
- **Collaborative annotation DB**: shared rename/comment store per binary hash.
- **Semantic function clustering / similarity** for malware families or refactor diffing.

---

## 5. Usability & Automation

- **Setup recipe**: add `just setup` that runs `rustup target add wasm32-unknown-unknown`
  and installs git hooks (`cargo fmt --check` pre-push). Lower first-run friction.
- **Clearer feature-missing errors**: `analyze --pdb` is `#[cfg(feature="debuginfo")]`
  only, so without the feature the flag is silently absent. Make the CLI print
  "enable the `debuginfo` feature" instead of "unexpected argument".
- **`just` coverage**: add `just ci` (fmt + clippy --all-features + test --all-features)
  mirroring CI so contributors reproduce gating locally.
- **GUI onboarding**: first-run walkthrough / "load sample" already exists; add an
  in-app command palette and a help panel listing shortcuts (goto/search/nav).
- **Dockerfile** for CI and zero-install adoption; **package managers** (`brew`, `scoop`)
  recipes in `docs/PACKAGING.md` (exists but could add formulas).
- **Browser demo hosting**: `build-wasm-gui` exists; publish the artifact + link in README
  so users try it with no install.

---

## 6. Adoption — examples & templates (already strong; extend)

Existing and good: 5 Rhai scripts (`auto_rename_printf`, `list_imports`, `find_crypto`,
`rename_by_prefix`, `summary`), hello Wasm plugin, `templates/` (cheat-sheet +
`first_analysis.rhai`), `docs/QUICKSTART.md`, sample binary + "Open Sample" button,
`just demo`/`qa`/`quickstart`.

Suggested additions:
- **One-click "recipe gallery" doc** (`docs/RECIPES.md`) showing each script's input→output.
- **Copy-paste problem→solution snippets** in the cheat-sheet (e.g. "rename all `sub_*`
  that call `memcpy`").
- **Tutorial binary with known answers** so users can self-verify (`templates/` annotated sample).
- **Video/GIF onboarding** in README (currently text + screenshot mention).
- **`just new-plugin <name>`** scaffolding recipe that generates a Wasm guest from the template.

---

## 7. Recommended first sprint (implementation order)

1. **B1 — ARM analysis** (blocker for the advertised ARM feature; biggest credibility gap).
2. **B2 — combined-CFG loop inflation** (small, fixes CLI `analyze`/`cfg` numbers).
3. **DWARF import (P0)** + **rename import/load (P0)** for real automation round-trip.
4. **`armature serve` (unchecked todo)** for adoption.
5. Usability: `just setup`, clearer feature errors, browser-demo link.

---

## 8. Validation

- `cargo test --workspace --all-features` must stay green; add **ARM unit tests** in
  `yaxpeax_arm.rs` asserting a branch produces `Mnemonic::Jcc`/`Call` and edges appear in
  `build_cfg` (currently none exist for ARM).
- Add a **DWARF** fixture (unstripped ELF) to the sample corpus; assert
  `debug_symbols` populates function names and they flow into `function.name`.
- `just ci` reproduces CI locally; `analyze`/`cfg` loop counts validated against a binary
  with a known single loop (regression for B2).
- `analyze --rename-file` round-trip: export renames, reload, confirm names persist.
