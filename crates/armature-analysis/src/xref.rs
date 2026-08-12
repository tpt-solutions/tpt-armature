//! Cross-reference (X-ref) indexing.
//!
//! Annotates which instruction addresses reference which other addresses
//! (intra-code jumps/calls, or named symbols). Powers clickable X-refs in the
//! assembly view and call/use graphs.

use armature_formats::Export;
use armature_ir::{Instruction, Operand};
use std::collections::HashMap;

/// The nature of a cross-reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrefKind {
    /// Reference to another code address (jump / call target).
    Code,
    /// Reference to a named/exported symbol.
    Symbol,
}

/// A single reference originating at `from` and pointing at some address.
#[derive(Debug, Clone)]
pub struct Xref {
    /// Address of the instruction that makes the reference.
    pub from: u64,
    /// Classification of the reference.
    pub kind: XrefKind,
}

/// Index of addresses -> the references that point at them.
#[derive(Debug, Clone, Default)]
pub struct XrefIndex {
    /// Target address -> list of references to it.
    pub refs_to: HashMap<u64, Vec<Xref>>,
}

impl XrefIndex {
    /// All known reference target addresses.
    pub fn targets(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self.refs_to.keys().copied().collect();
        v.sort();
        v
    }

    /// References pointing at a specific address.
    pub fn refs_to_addr(&self, addr: u64) -> &[Xref] {
        self.refs_to.get(&addr).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Total number of recorded references.
    pub fn count(&self) -> usize {
        self.refs_to.values().map(|v| v.len()).sum()
    }
}

/// Build an X-ref index from a flat instruction list and a symbol table.
pub fn build_xrefs(instructions: &[Instruction], symbols: &[Export]) -> XrefIndex {
    let code_addrs: std::collections::HashSet<u64> =
        instructions.iter().map(|i| i.address).collect();

    let symbol_addrs: std::collections::HashSet<u64> =
        symbols.iter().filter(|s| s.addr != 0).map(|s| s.addr).collect();

    let mut refs_to: HashMap<u64, Vec<Xref>> = HashMap::new();
    for ins in instructions {
        for op in &ins.operands {
            let target = match op {
                Operand::Imm(v) => *v,
                _ => continue,
            };
            let kind = if code_addrs.contains(&target) {
                XrefKind::Code
            } else if symbol_addrs.contains(&target) {
                XrefKind::Symbol
            } else {
                continue;
            };
            refs_to.entry(target).or_default().push(Xref {
                from: ins.address,
                kind,
            });
        }
    }

    XrefIndex { refs_to }
}

/// Build an X-ref index over every instruction in a module.
pub fn build_xrefs_for_module(
    module: &armature_ir::Module,
    symbols: &[Export],
) -> XrefIndex {
    let mut instructions = Vec::new();
    for f in &module.functions {
        for b in &f.blocks {
            instructions.extend(b.instructions.iter().cloned());
        }
    }
    build_xrefs(&instructions, symbols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use armature_ir::{Instruction, Mnemonic, Operand};

    fn ins(addr: u64, m: Mnemonic, ops: Vec<Operand>) -> Instruction {
        Instruction {
            address: addr,
            size: 4,
            mnemonic: m,
            operands: ops,
            raw: vec![0; 4],
            text: format!("{m}"),
        }
    }

    #[test]
    fn code_and_symbol_xrefs() {
        let insns = vec![
            ins(0x100, Mnemonic::Call, vec![Operand::Imm(0x200)]),
            ins(0x104, Mnemonic::Mov, vec![Operand::Reg("rax".into()), Operand::Imm(0x500)]),
            ins(0x200, Mnemonic::Ret, vec![]),
        ];
        let symbols = vec![Export {
            name: "printf".into(),
            addr: 0x500,
        }];
        let idx = build_xrefs(&insns, &symbols);
        assert_eq!(idx.refs_to_addr(0x200).len(), 1);
        assert_eq!(idx.refs_to_addr(0x200)[0].kind, XrefKind::Code);
        assert_eq!(idx.refs_to_addr(0x500)[0].kind, XrefKind::Symbol);
        assert_eq!(idx.count(), 2);
    }
}
