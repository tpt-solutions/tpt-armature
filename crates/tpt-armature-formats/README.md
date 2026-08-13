# tpt-armature-formats

**TPT Armature — Layer 1 (Ingestion): "Stripping the Casing".**

`tpt-armature-formats` parses raw executable bytes (PE, ELF, Mach-O) into a
single, architecture-agnostic [`MemoryMap`] so the rest of the suite can reason
about any binary through one interface.

## Features

- Parses the three mainstream container formats: **PE**, **ELF**, and **Mach-O**.
- Detects the CPU architecture (`x86`, `x86_64`, `ARM`, `AArch64`, …) via
  [`Architecture`].
- Exposes sections, exports, and imports as plain data structures.
- `debuginfo` feature: recovers ELF `.symtab` names and merges PE PDB public
  symbols into [`DebugSymbol`] rows.

## Getting started

```rust
use tpt_armature_formats::{parse, Architecture};

let bytes = std::fs::read("target/release/tpt-armature")?;
let map = parse(&bytes)?;
println!("format = {:?}, arch = {:?}", map.format, map.arch);
for section in &map.sections {
    println!("- {} ({} bytes @ {:x})", section.name, section.size, section.vaddr);
}
```

## Key types

| Type | Purpose |
| ---- | ------- |
| `MemoryMap` | The standardized, in-memory view of a parsed binary. |
| `Architecture` | Detected CPU architecture. |
| `BinaryFormat` | `Pe` / `Elf` / `MachO` / `Unknown`. |
| `Section` | A named, address-ranged chunk of the image. |
| `Export` / `Import` | Symbol tables. |
| `DebugSymbol` | Symbols recovered from debug information (`debuginfo`). |

## Feature flags

| Flag | Enables |
| ---- | ------- |
| `arm` | ARM/AArch64 awareness (`Architecture::is_disassemblable`) without the disassembly backend. |
| `debuginfo` | ELF `.symtab` recovery and PE PDB public-symbol import. |

## Minimum supported Rust version

1.85 (edition 2021).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
