# Changelog

All notable changes to `tpt-armature-cli` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- End-to-end headless pipeline driver with `analyze`, `disasm`, `cfg`,
  `strings`, `decompile`, `script` (feature `rhai`), `plugin` / `plugins`
  (feature `wasm`), and `watch` subcommands.
- `analyze --json` CI-friendly summary.
- `--features debuginfo` → `analyze --pdb <file.pdb>` PDB public-symbol merge.
- `arm` feature forwarding to the disassembly backend.

## [0.1.0]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
