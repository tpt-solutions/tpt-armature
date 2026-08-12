# CLAUDE.md — Repository conventions for agent sessions

This file records the conventions of the TPT Armature repository so future
agent sessions (Claude, Kilo, or otherwise) operate consistently.

## Project

TPT Armature is a 100% Rust reverse engineering suite. Workspace + crates under
`crates/*`. Follow `todo.md` (build checklist mapped to `spec.txt`).

## Commands

- `cargo build --workspace` — build everything (default features only).
- `cargo fmt --check` — formatting gate. Run `cargo fmt` to fix.
- `cargo clippy --workspace --all-targets` — lint.
- `just fmt`, `just lint`, `just test`, `just build` — task recipes.
- Feature-gated heavy deps: `arm` (ARM disasm), `app` (GUI), `rhai` and `wasm`
  (extension layer). Default workspace build must stay light and green.

## Conventions

- Edition 2021, rust-version 1.85, `rustfmt.toml` max_width 100.
- Public crate surfaces must be documented (`///`) for `cargo doc`.
- Errors use `thiserror`; fallible public APIs return `Result`.
- Never introduce foreign-language runtimes (Python/Node) into the core.
- Path dependencies on sibling `tpt-*` crates are intentionally avoided in this
  environment; the GUI targets egui directly (the engine tpt-appfront uses).
- Dual licensed: MIT OR Apache-2.0.
