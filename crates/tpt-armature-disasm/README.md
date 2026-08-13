# tpt-armature-disasm

**TPT Armature — Layer 2 (Disassembly): "Exposing the Framework".**

Wraps the native Rust disassembler backends and lowers their output into the
shared [`tpt_armature_ir::Instruction`] representation.

- **iced-x86** for `x86` / `x86_64` — always available.
- **yaxpeax-arm** for `ARM` / `AArch64` — behind the `arm` feature.

## Getting started

```rust
use tpt_armature_disasm::for_architecture;
use tpt_armature_formats::{parse, Architecture};

let bytes = std::fs::read("target/release/tpt-armature")?;
let map = parse(&bytes)?;
let disassembler = for_architecture(map.arch);
let section = &map.sections[0];
let instrs = disassembler.disassemble(&section.bytes, section.vaddr)?;
println!("decoded {} instructions", instrs.len());
```

## Key APIs

| Item | Purpose |
| ---- | ------- |
| `Disassembler` | Trait implemented by each backend. |
| `for_architecture` | Pick a backend for a given `Architecture`. |
| `disassemble` | Lower raw bytes at a virtual address into `Vec<Instruction>`. |

## Feature flags

| Flag | Enables |
| ---- | ------- |
| `arm` | ARM/AArch64 disassembly via yaxpeax (enables `tpt-armature-formats/arm`). |

## Minimum supported Rust version

1.85 (edition 2021).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
