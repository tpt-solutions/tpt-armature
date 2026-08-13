# Changelog

All notable changes to `tpt-armature-analysis` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Control-flow graph construction (`build_cfg`, `Cfg`, `Edge`, `EdgeKind`).
- Cross-reference index (`build_xrefs`, `Xref`, `XrefIndex`, `XrefKind`).
- Lightweight data-flow summary (`analyze`, `DataFlow`).
- String / constant extraction (`extract_strings`, `extract_constants`).
- IR → C-like pseudocode view (`decompile_function`, `decompile_module`).
- Top-level `analyze_binary` / `analyze_map` pipeline and `Analysis` result.
- Rename export and JSON serialization (`export_renames`, `analysis_to_json`).
- Recursive-descent `recover_functions` (re-exported from `tpt-armature-ir`).

## [0.1.0]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
