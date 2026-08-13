# Changelog

All notable changes to `tpt-armature-disasm` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- `Disassembler` abstraction over native backends.
- iced-x86/x64 backend (default).
- yaxpeax ARM backend behind the `arm` feature (ARM / AArch64).
- `for_architecture` helper that selects the appropriate backend.
- Lowered output into `tpt_armature_ir::Instruction`.

## [0.1.0]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
