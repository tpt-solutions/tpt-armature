# Changelog

All notable changes to `tpt-armature-formats` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Initial workspace crate: PE/ELF/Mach-O parsing into a standardized `MemoryMap`.
- Architecture detection via `Architecture` (`x86`, `x86_64`, `ARM`, `AArch64`).
- Section, export, and import extraction with virtual-address resolution
  (including Mach-O entry point and exports).
- `debuginfo` feature: ELF `.symtab` recovery and PE PDB public-symbol merge
  via the `pdb` crate (`DebugSymbol` / `DebugSymbolKind`).
- `arm` feature: ARM/AArch64 awareness (`Architecture::is_disassemblable`)
  without pulling in the disassembly backend.
- `MemoryMap`, `Section`, `Export`, `Import`, `BinaryFormat` public types and a
  `parse` entry point with `thiserror`-based `FormatError`.

## [0.1.0]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
