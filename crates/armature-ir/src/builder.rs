//! IR builder: assembles raw [`Instruction`]s into basic blocks and modules.
//!
//! Both disassembler backends feed decoded instructions into an [`IrBuilder`];
//! once the code region is fully disassembled, [`IrBuilder::build_blocks`] and
//! [`IrBuilder::build_module`] perform a linear-sweep basic-block partition that
//! the analysis layer then turns into a control-flow graph.

use crate::instr::{BasicBlock, Function, Instruction, Mnemonic};
use crate::operand::Operand;
use std::collections::HashSet;

/// Accumulates decoded instructions and partitions them into basic blocks.
#[derive(Debug, Default)]
pub struct IrBuilder {
    instructions: Vec<Instruction>,
}

impl IrBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        IrBuilder {
            instructions: Vec::new(),
        }
    }

    /// Append a single instruction.
    pub fn push(&mut self, ins: Instruction) {
        self.instructions.push(ins);
    }

    /// Append many instructions.
    pub fn extend(&mut self, ins: impl IntoIterator<Item = Instruction>) {
        self.instructions.extend(ins);
    }

    /// All decoded instructions, in address order.
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    /// Number of decoded instructions.
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Whether no instructions have been decoded.
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Partition the decoded instructions into maximal basic blocks.
    ///
    /// Leaders are: the entry address, every jump/call target, and every
    /// fall-through address following a non-unconditional terminator.
    pub fn build_blocks(&self) -> Vec<BasicBlock> {
        if self.instructions.is_empty() {
            return Vec::new();
        }

        let mut leaders: HashSet<u64> = HashSet::new();
        leaders.insert(self.instructions[0].address);

        for ins in &self.instructions {
            let target = branch_target(ins);
            if matches!(ins.mnemonic, Mnemonic::Jmp | Mnemonic::Jcc(_) | Mnemonic::Call) {
                if let Some(t) = target {
                    leaders.insert(t);
                }
            }
            if !ins.mnemonic.is_unconditional_branch() {
                leaders.insert(ins.address + ins.size as u64);
            }
        }

        let mut blocks: Vec<BasicBlock> = Vec::new();
        let mut current: Option<(u64, Vec<Instruction>)> = None;

        for ins in &self.instructions {
            if leaders.contains(&ins.address) {
                if let Some((start, insts)) = current.take() {
                    blocks.push(make_block(start, insts));
                }
            }
            if current.is_none() {
                current = Some((ins.address, Vec::new()));
            }
            current.as_mut().unwrap().1.push(ins.clone());

            if ins.mnemonic.is_unconditional_branch() {
                if let Some((start, insts)) = current.take() {
                    blocks.push(make_block(start, insts));
                }
            }
        }
        if let Some((start, insts)) = current.take() {
            blocks.push(make_block(start, insts));
        }

        for (id, block) in blocks.iter_mut().enumerate() {
            block.id = id;
        }
        blocks
    }

    /// Build a single-function [`Module`] from the decoded instructions.
    pub fn build_module(&self) -> Module {
        let blocks = self.build_blocks();
        let start = blocks.first().map(|b| b.start).unwrap_or(0);
        let function = Function {
            id: 0,
            start,
            name: None,
            blocks,
        };
        Module {
            functions: vec![function],
        }
    }
}

/// The branch destination of a control-flow instruction, if it is an immediate.
fn branch_target(ins: &Instruction) -> Option<u64> {
    match ins.mnemonic {
        Mnemonic::Jmp | Mnemonic::Jcc(_) | Mnemonic::Call => ins
            .operands
            .iter()
            .find_map(|o| match o {
                Operand::Imm(v) => Some(*v),
                _ => None,
            }),
        _ => None,
    }
}

fn make_block(start: u64, instructions: Vec<Instruction>) -> BasicBlock {
    let end = instructions
        .last()
        .map(|i| i.address + i.size as u64)
        .unwrap_or(start);
    BasicBlock {
        id: 0,
        start,
        end,
        instructions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operand::Operand;

    fn ins(addr: u64, mnem: Mnemonic, ops: Vec<Operand>) -> Instruction {
        let text = format!("{mnem} {}", ops.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(", "));
        Instruction {
            address: addr,
            size: 5,
            mnemonic: mnem,
            operands: ops,
            raw: vec![0u8; 5],
            text,
        }
    }

    #[test]
    fn mnemonic_classification() {
        assert!(Mnemonic::from_str("RET").is_terminator());
        assert!(Mnemonic::from_str("jmp").is_unconditional_branch());
        assert!(Mnemonic::from_str("je").is_terminator());
        assert!(!Mnemonic::from_str("add").is_terminator());
        assert_eq!(Mnemonic::from_str("je"), Mnemonic::Jcc("e".into()));
        assert_eq!(Mnemonic::from_str("CALL"), Mnemonic::Call);
    }

    #[test]
    fn block_partition_on_branch() {
        let mut b = IrBuilder::new();
        b.push(ins(0x1000, Mnemonic::Mov, vec![Operand::Reg("rax".into()), Operand::Imm(1)]));
        b.push(ins(0x1005, Mnemonic::Cmp, vec![Operand::Reg("rax".into()), Operand::Imm(0)]));
        b.push(ins(0x100a, Mnemonic::Jcc("z".into()), vec![Operand::Imm(0x1020)]));
        b.push(ins(0x100f, Mnemonic::Add, vec![Operand::Reg("rax".into()), Operand::Imm(1)]));
        b.push(ins(0x1014, Mnemonic::Jmp, vec![Operand::Imm(0x1000)]));
        b.push(ins(0x1019, Mnemonic::Nop, vec![]));
        b.push(ins(0x1020, Mnemonic::Ret, vec![]));

        let blocks = b.build_blocks();
        // Leaders: 0x1000, 0x100f (fallthrough of jz), 0x1019 (fallthrough of jmp? no, jmp is
        // unconditional -> closes), 0x1020 (target of jz).
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].start, 0x1000);
        assert_eq!(blocks[0].end, 0x100f); // ends at the je
        assert_eq!(blocks[1].start, 0x100f);
        assert_eq!(blocks[2].start, 0x1019);
        assert_eq!(blocks[3].start, 0x1020);
    }

    #[test]
    fn defs_and_uses() {
        let i = ins(
            0,
            Mnemonic::Add,
            vec![Operand::Reg("rax".into()), Operand::Reg("rbx".into())],
        );
        assert_eq!(i.defs(), vec!["rax".to_string()]);
        assert!(i.uses().contains(&"rbx".to_string()));
        assert!(i.uses().contains(&"rax".to_string()));
    }
}
