# Changelog

All notable changes to `tpt-armature-ext` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Rhai scripting host (`ScriptHost`) behind the `rhai` feature, with bindings
  (`format`, `arch`, `entry`, `instruction_count`, `function_count`, `imports`,
  `exports`, `symbol_xrefs`, `symbol_names`) and native `rename` / `symbol_name`.
- wasmtime sandboxed plugin host (`PluginHost`, `PluginApi`, `PluginOutput`)
  behind the `wasm` feature, implementing the `tpt-armature` guest ABI
  (`log`, `get_instruction_count`, `rename`).
- `default_rename_script` example automation.
- Ready-made Rhai scripts (`scripts/`): `auto_rename_printf`, `list_imports`,
  `find_crypto`, `rename_by_prefix`, `summary`.

## [0.1.0]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
