//! Lightweight data-flow analysis.
//!
//! Tracks where registers (our proxy for "variables") are created (defined),
//! modified (re-defined), and used, at instruction granularity. This is a
//! deliberately simple pass — sufficient to power variable-lifetime views and
//! rename heuristics without a full SSA construction.

use armature_ir::Instruction;
use std::collections::HashMap;

/// Per-register definition and use sites across a linear instruction stream.
#[derive(Debug, Clone, Default)]
pub struct DataFlow {
    /// Register name -> addresses where it is defined (created/modified).
    pub defs: HashMap<String, Vec<u64>>,
    /// Register name -> addresses where it is read.
    pub uses: HashMap<String, Vec<u64>>,
}

impl DataFlow {
    /// Compute data-flow summary for a sequence of instructions (address order).
    pub fn analyze(instructions: &[Instruction]) -> DataFlow {
        let mut defs: HashMap<String, Vec<u64>> = HashMap::new();
        let mut uses: Vec<(String, u64)> = Vec::new();

        for ins in instructions {
            for d in ins.defs() {
                defs.entry(d).or_default().push(ins.address);
            }
            for u in ins.uses() {
                uses.push((u, ins.address));
            }
        }

        let mut uses_map: HashMap<String, Vec<u64>> = HashMap::new();
        for (reg, addr) in uses {
            uses_map.entry(reg).or_default().push(addr);
        }

        DataFlow {
            defs,
            uses: uses_map,
        }
    }

    /// Addresses where a register is first defined (created).
    pub fn creation_sites(&self, reg: &str) -> &[u64] {
        self.defs.get(reg).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Addresses where a register is read.
    pub fn use_sites(&self, reg: &str) -> &[u64] {
        self.uses.get(reg).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// All registers that appear in the summary.
    pub fn registers(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .defs
            .keys()
            .chain(self.uses.keys())
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

/// Convenience wrapper that analyzes every instruction in a module.
pub fn analyze(module: &armature_ir::Module) -> DataFlow {
    let mut all: Vec<&Instruction> = Vec::new();
    for f in &module.functions {
        for b in &f.blocks {
            for i in &b.instructions {
                all.push(i);
            }
        }
    }
    let instructions: Vec<Instruction> = all.into_iter().cloned().collect();
    DataFlow::analyze(&instructions)
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
    fn tracks_defs_and_uses() {
        let insns = vec![
            ins(0x0, Mnemonic::Mov, vec![Operand::Reg("rax".into()), Operand::Imm(1)]),
            ins(0x4, Mnemonic::Add, vec![Operand::Reg("rax".into()), Operand::Reg("rbx".into())]),
            ins(0x8, Mnemonic::Mov, vec![Operand::Reg("rbx".into()), Operand::Reg("rax".into())]),
        ];
        let df = DataFlow::analyze(&insns);
        assert_eq!(df.creation_sites("rax"), &[0x0, 0x4]);
        assert_eq!(df.creation_sites("rbx"), &[0x8]);
        assert!(df.use_sites("rbx").contains(&0x4));
        assert!(df.registers().contains(&"rax".to_string()));
    }
}
