# Changelog

All notable changes to `tpt-armature-ir` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Custom IR types: `Instruction`, `Mnemonic`, `Operand`, `BasicBlock`,
  `Function`, `Module`.
- `IrBuilder` for incremental construction.
- Recursive-descent `recover_functions`: splits the code section into functions
  from the entry point, exported symbols, and discovered call targets, with a
  linear-sweep fallback so no code is lost.

## [0.1.0]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
