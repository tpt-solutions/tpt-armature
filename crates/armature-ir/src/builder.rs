//! IR builder: assembles raw [`Instruction`]s into basic blocks and modules.
//!
//! Both disassembler backends feed decoded instructions into an [`IrBuilder`];
//! once the code region is fully disassembled, [`IrBuilder::build_blocks`] and
//! [`IrBuilder::build_module`] perform a linear-sweep basic-block partition that
//! the analysis layer then turns into a control-flow graph. [`recover_functions`]
//! upgrades the flat instruction stream into proper functions via recursive
//! descent from known entry points.

use crate::instr::{BasicBlock, Function, Instruction, Mnemonic};
use crate::operand::Operand;
use std::collections::{HashMap, HashSet};

/// A collection of analyzed functions produced by the builder.
#[derive(Debug, Clone, Default)]
pub struct Module {
    /// Functions discovered during disassembly (recursive-descent recovery).
    pub functions: Vec<Function>,
}

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
            if matches!(
                ins.mnemonic,
                Mnemonic::Jmp | Mnemonic::Jcc(_) | Mnemonic::Call
            ) {
                if let Some(t) = target {
                    leaders.insert(t);
                }
            }
            // Only conditional branches and calls fall through to the next
            // sequential instruction; jmp/ret do not.
            if matches!(ins.mnemonic, Mnemonic::Jcc(_) | Mnemonic::Call) {
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

            if ins.mnemonic.is_terminator() {
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

/// Recover functions from a flat, address-ordered instruction stream using
/// recursive descent from the supplied entry points.
///
/// Flow rules within a function:
/// * fall-through (next sequential instruction) continues in the same function;
/// * conditional-branch and intra-section unconditional-jump targets continue
///   in the same function;
/// * a `call` target that falls inside the code region starts a *new* function
///   (discovered lazily as an entry point), while the call site keeps its own
///   fall-through;
/// * `ret` / jumps to external addresses terminate the current function.
///
/// After the entry-point graph is walked, any remaining undecoded instructions
/// are swept up as maximal contiguous runs so analysis never loses coverage.
pub fn recover_functions(instructions: &[Instruction], entries: &[u64]) -> Vec<Function> {
    if instructions.is_empty() {
        return Vec::new();
    }

    let by_addr: HashMap<u64, &Instruction> =
        instructions.iter().map(|i| (i.address, i)).collect();
    let mut assigned: HashSet<u64> = HashSet::new();
    let mut functions: Vec<Function> = Vec::new();
    let mut pending: Vec<u64> = entries.to_vec();
    let mut next_id: usize = 0;

    while let Some(entry) = pending.pop() {
        if assigned.contains(&entry) || !by_addr.contains_key(&entry) {
            continue;
        }
        let (func_insts, discovered) = walk_function(entry, &by_addr, &mut assigned);
        let mut func_insts = func_insts;
        func_insts.sort_by_key(|i| i.address);
        let mut builder = IrBuilder::new();
        builder.extend(func_insts);
        let blocks = builder.build_blocks();
        functions.push(Function {
            id: next_id,
            start: entry,
            name: None,
            blocks,
        });
        next_id += 1;
        pending.extend(discovered);
    }

    // Linear-sweep fallback: absorb every instruction not yet claimed, grouping
    // maximal contiguous runs (by sequential fall-through) into functions.
    for ins in instructions {
        if assigned.contains(&ins.address) {
            continue;
        }
        let mut run: Vec<Instruction> = Vec::new();
        let mut cur = ins.address;
        while let Some(i) = by_addr.get(&cur) {
            if assigned.contains(&cur) {
                break;
            }
            assigned.insert(cur);
            run.push((*i).clone());
            cur += i.size as u64;
        }
        let mut builder = IrBuilder::new();
        builder.extend(run);
        let blocks = builder.build_blocks();
        functions.push(Function {
            id: next_id,
            start: ins.address,
            name: None,
            blocks,
        });
        next_id += 1;
    }

    functions
}

/// Walk one function from `entry`, returning its instructions and any newly
/// discovered call-target entry points. See [`recover_functions`] for the flow
/// rules.
fn walk_function(
    entry: u64,
    by_addr: &HashMap<u64, &Instruction>,
    assigned: &mut HashSet<u64>,
) -> (Vec<Instruction>, Vec<u64>) {
    let mut func_insts: Vec<Instruction> = Vec::new();
    let mut discovered: Vec<u64> = Vec::new();
    let mut stack = vec![entry];
    while let Some(addr) = stack.pop() {
        let ins = match by_addr.get(&addr) {
            Some(i) => *i,
            None => continue,
        };
        if assigned.contains(&addr) {
            continue;
        }
        assigned.insert(addr);
        func_insts.push(ins.clone());

        match &ins.mnemonic {
            Mnemonic::Ret => {}
            Mnemonic::Jmp => {
                if let Some(t) = branch_target(ins) {
                    if by_addr.contains_key(&t) {
                        stack.push(t);
                    }
                }
            }
            Mnemonic::Jcc(_) => {
                if let Some(t) = branch_target(ins) {
                    if by_addr.contains_key(&t) {
                        stack.push(t);
                    }
                }
                let fall = addr + ins.size as u64;
                if by_addr.contains_key(&fall) {
                    stack.push(fall);
                }
            }
            Mnemonic::Call => {
                if let Some(t) = branch_target(ins) {
                    if by_addr.contains_key(&t) && !assigned.contains(&t) {
                        discovered.push(t);
                    }
                }
                let fall = addr + ins.size as u64;
                if by_addr.contains_key(&fall) {
                    stack.push(fall);
                }
            }
            _ => {
                let fall = addr + ins.size as u64;
                if by_addr.contains_key(&fall) {
                    stack.push(fall);
                }
            }
        }
    }
    (func_insts, discovered)
}

/// The branch destination of a control-flow instruction, if it is an immediate.
fn branch_target(ins: &Instruction) -> Option<u64> {
    match ins.mnemonic {
        Mnemonic::Jmp | Mnemonic::Jcc(_) | Mnemonic::Call => {
            ins.operands.iter().find_map(|o| match o {
                Operand::Imm(v) => Some(*v),
                _ => None,
            })
        }
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
        let text = format!(
            "{mnem} {}",
            ops.iter()
                .map(|o| o.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
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
        b.push(ins(
            0x1000,
            Mnemonic::Mov,
            vec![Operand::Reg("rax".into()), Operand::Imm(1)],
        ));
        b.push(ins(
            0x1005,
            Mnemonic::Cmp,
            vec![Operand::Reg("rax".into()), Operand::Imm(0)],
        ));
        b.push(ins(
            0x100a,
            Mnemonic::Jcc("z".into()),
            vec![Operand::Imm(0x1020)],
        ));
        b.push(ins(
            0x100f,
            Mnemonic::Add,
            vec![Operand::Reg("rax".into()), Operand::Imm(1)],
        ));
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

    #[test]
    fn recover_splits_functions_on_call() {
        // Function A (0x1000): mov; call 0x2000; ret
        // Function B (0x2000): mov; ret   (reached via the call)
        let mut instrs = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(1)],
            ),
            ins(0x1005, Mnemonic::Call, vec![Operand::Imm(0x2000)]),
            ins(0x100a, Mnemonic::Ret, vec![]),
            ins(
                0x2000,
                Mnemonic::Mov,
                vec![Operand::Reg("rbx".into()), Operand::Imm(2)],
            ),
            ins(0x2005, Mnemonic::Ret, vec![]),
        ];
        instrs.sort_by_key(|i| i.address);

        let functions = recover_functions(&instrs, &[0x1000]);
        assert_eq!(functions.len(), 2, "expected two recovered functions");

        let a = functions.iter().find(|f| f.start == 0x1000).unwrap();
        assert_eq!(a.instruction_count(), 3);
        assert!(!a.blocks.is_empty());

        let b = functions.iter().find(|f| f.start == 0x2000).unwrap();
        assert_eq!(b.instruction_count(), 2);
    }

    #[test]
    fn recover_lazy_call_target_is_entry() {
        // A single entry point; the called function must still be discovered.
        let instrs = vec![
            ins(0x10, Mnemonic::Call, vec![Operand::Imm(0x40)]),
            ins(0x15, Mnemonic::Ret, vec![]),
            ins(0x40, Mnemonic::Nop, vec![]),
            ins(0x45, Mnemonic::Ret, vec![]),
        ];
        let functions = recover_functions(&instrs, &[0x10]);
        assert_eq!(functions.len(), 2);
        assert!(functions.iter().any(|f| f.start == 0x40));
    }
}
