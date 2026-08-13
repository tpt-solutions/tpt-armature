# tpt-armature-ir

**TPT Armature — Layer 2 (IR): "Exposing the Framework".**

The intermediate representation (IR) is the lingua franca between the
disassembly backends and the analysis passes. Every backend lowers its native
instruction representation into the `Instruction` / `Operand` / `Mnemonic`
types defined here, and every analysis (CFG, data-flow, X-refs, decompilation)
consumes them.

## Getting started

```rust
use tpt_armature_ir::{Instruction, Mnemonic, Operand, BasicBlock, Function, Module};

let mut module = Module::new();
let mut block = BasicBlock::new(0x1000);
block.push(Instruction {
    address: 0x1000,
    mnemonic: Mnemonic::Mov,
    operands: vec![Operand::Reg("rax".into()), Operand::Imm(1)],
    bytes: vec![0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00],
});
module.functions.push(Function { entry: 0x1000, blocks: vec![block] });
```

## Key types

| Type | Purpose |
| ---- | ------- |
| `Instruction` | A single decoded instruction (address, mnemonic, operands, raw bytes). |
| `Mnemonic` | The operation family (enum). |
| `Operand` | A register, immediate, or memory operand. |
| `BasicBlock` | A linear sequence of instructions with one entry. |
| `Function` | A recovered function: entry address + basic blocks. |
| `Module` | The full IR container. |
| `IrBuilder` / `recover_functions` | Incremental construction and recursive-descent function recovery. |

## Function recovery

`recover_functions` splits the code section into proper functions from the
entry point, exported symbols, and discovered call targets, with a
linear-sweep fallback so no code is lost.

## Minimum supported Rust version

1.85 (edition 2021).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
