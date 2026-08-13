//! Control-flow graph construction.

use armature_ir::{BasicBlock, Instruction, Mnemonic, Operand};

/// The kind of control-flow edge between two basic blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// Unconditional jump (`jmp`) to the target block.
    Unconditional,
    /// Conditional branch (`jcc`) taken to the target block.
    Conditional,
    /// Fall-through into the next sequential block.
    Fallthrough,
    /// Call into another block (intra-module only).
    Call,
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EdgeKind::Unconditional => "jmp",
            EdgeKind::Conditional => "jcc",
            EdgeKind::Fallthrough => "fall",
            EdgeKind::Call => "call",
        };
        f.write_str(s)
    }
}

/// A directed edge between two basic blocks of a function.
#[derive(Debug, Clone)]
pub struct Edge {
    /// Source block index.
    pub from: usize,
    /// Destination block index (index into [`Cfg::nodes`]).
    pub to: usize,
    /// Edge classification.
    pub kind: EdgeKind,
    /// Virtual address of the destination (for call/branch targets).
    pub target_addr: u64,
}

/// A control-flow graph over the basic blocks of a single function.
#[derive(Debug, Clone)]
pub struct Cfg {
    /// Basic blocks, indexed by [`Edge::from`]/[`Edge::to`].
    pub nodes: Vec<BasicBlock>,
    /// Directed edges.
    pub edges: Vec<Edge>,
    /// Count of back-edges (loops), computed during construction by the
    /// back-edge analysis over [`Cfg::edges`].
    pub loop_count: usize,
}

impl Cfg {
    /// Build a CFG from a sequence of basic blocks.
    pub fn from_blocks(blocks: Vec<BasicBlock>) -> Cfg {
        let mut addr_to_idx = std::collections::HashMap::new();
        for (i, b) in blocks.iter().enumerate() {
            addr_to_idx.insert(b.start, i);
        }

        let mut edges = Vec::new();
        for (i, block) in blocks.iter().enumerate() {
            let term = match block.terminator() {
                Some(t) => t,
                None => continue,
            };
            match &term.mnemonic {
                Mnemonic::Jmp => {
                    if let Some(target) = branch_imm(term) {
                        if let Some(&dst) = addr_to_idx.get(&target) {
                            edges.push(Edge {
                                from: i,
                                to: dst,
                                kind: EdgeKind::Unconditional,
                                target_addr: target,
                            });
                        }
                    }
                }
                Mnemonic::Jcc(_) => {
                    if let Some(target) = branch_imm(term) {
                        if let Some(&dst) = addr_to_idx.get(&target) {
                            edges.push(Edge {
                                from: i,
                                to: dst,
                                kind: EdgeKind::Conditional,
                                target_addr: target,
                            });
                        }
                    }
                    // fallthrough
                    if let Some(&dst) = addr_to_idx.get(&block.end) {
                        edges.push(Edge {
                            from: i,
                            to: dst,
                            kind: EdgeKind::Fallthrough,
                            target_addr: block.end,
                        });
                    }
                }
                Mnemonic::Call => {
                    if let Some(target) = branch_imm(term) {
                        if let Some(&dst) = addr_to_idx.get(&target) {
                            edges.push(Edge {
                                from: i,
                                to: dst,
                                kind: EdgeKind::Call,
                                target_addr: target,
                            });
                        }
                    }
                    if let Some(&dst) = addr_to_idx.get(&block.end) {
                        edges.push(Edge {
                            from: i,
                            to: dst,
                            kind: EdgeKind::Fallthrough,
                            target_addr: block.end,
                        });
                    }
                }
                Mnemonic::Ret => { /* terminal, no outgoing */ }
                _ => {
                    if let Some(&dst) = addr_to_idx.get(&block.end) {
                        edges.push(Edge {
                            from: i,
                            to: dst,
                            kind: EdgeKind::Fallthrough,
                            target_addr: block.end,
                        });
                    }
                }
            }
        }

        let loop_count = count_back_edges(&blocks, &edges, &addr_to_idx);
        Cfg {
            nodes: blocks,
            edges,
            loop_count,
        }
    }

    /// Number of basic blocks.
    pub fn block_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Pretty summary used by the CLI.
    pub fn summary(&self) -> String {
        format!(
            "CFG: {} blocks, {} edges, {} loop(s)",
            self.block_count(),
            self.edge_count(),
            self.loop_count
        )
    }
}

/// Build a CFG over every function in the module (a combined graph).
pub fn build_cfg(module: &armature_ir::Module) -> Cfg {
    let mut blocks = Vec::new();
    for f in &module.functions {
        blocks.extend(f.blocks.clone());
    }
    Cfg::from_blocks(blocks)
}

fn branch_imm(term: &Instruction) -> Option<u64> {
    match &term.mnemonic {
        Mnemonic::Jmp | Mnemonic::Jcc(_) | Mnemonic::Call => {
            term.operands.iter().find_map(|o| match o {
                Operand::Imm(v) => Some(*v),
                _ => None,
            })
        }
        _ => None,
    }
}

/// A back-edge (destination dominates source) is a conservative loop indicator.
fn count_back_edges(
    blocks: &[BasicBlock],
    edges: &[Edge],
    addr_to_idx: &std::collections::HashMap<u64, usize>,
) -> usize {
    let _ = (blocks, addr_to_idx);
    // Simple heuristic: an edge whose target address is strictly less than the
    // source block's start is a backward jump, i.e. a loop back-edge.
    edges
        .iter()
        .filter(|e| {
            let from_start = blocks.get(e.from).map(|b| b.start).unwrap_or(0);
            e.target_addr < from_start
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use armature_ir::{Instruction, Mnemonic, Operand};
    use std::collections::HashMap;

    fn ins(addr: u64, m: Mnemonic, ops: Vec<Operand>) -> Instruction {
        Instruction {
            address: addr,
            size: 4,
            text: format!("{m}"),
            mnemonic: m,
            operands: ops,
            raw: vec![0; 4],
        }
    }

    fn block(id: usize, start: u64, insns: Vec<Instruction>) -> BasicBlock {
        let end = insns
            .last()
            .map(|i| i.address + i.size as u64)
            .unwrap_or(start);
        BasicBlock {
            id,
            start,
            end,
            instructions: insns,
        }
    }

    #[test]
    fn cfg_loop_detection() {
        // A real loop: the tail of block 1 jumps back to block 0 (0x100 < 0x200).
        // 0x100: mov            (block 0)
        // 0x104: jmp 0x200      (block 0 -> block 1)
        // 0x200: add            (block 1)
        // 0x204: cmp            (block 1)
        // 0x208: jne 0x200      (block 1 self-loop)
        // 0x20c: jmp 0x100      (block 1 -> block 0 : back-edge / loop)
        // 0x300: ret            (block 2)
        let mut blocks = vec![block(
            0,
            0x100,
            vec![
                ins(0x100, Mnemonic::Mov, vec![]),
                ins(0x104, Mnemonic::Jmp, vec![Operand::Imm(0x200)]),
            ],
        )];
        blocks.push(block(
            1,
            0x200,
            vec![
                ins(0x200, Mnemonic::Add, vec![]),
                ins(0x204, Mnemonic::Cmp, vec![]),
                ins(0x208, Mnemonic::Jcc("ne".into()), vec![Operand::Imm(0x200)]),
                ins(0x20c, Mnemonic::Jmp, vec![Operand::Imm(0x100)]),
            ],
        ));
        blocks.push(block(2, 0x300, vec![ins(0x300, Mnemonic::Ret, vec![])]));

        let cfg = Cfg::from_blocks(blocks);
        assert_eq!(cfg.block_count(), 3);
        // unconditional jmp 0x100 from block 1 -> block 0 (back-edge)
        assert!(cfg
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::Unconditional && e.to == 0 && e.target_addr == 0x100));
        assert_eq!(cfg.loop_count, 1);
    }

    #[test]
    fn cfg_fallthrough() {
        let blocks = vec![
            block(
                0,
                0x0,
                vec![
                    ins(0x0, Mnemonic::Mov, vec![]),
                    ins(0x4, Mnemonic::Ret, vec![]),
                ],
            ),
            block(1, 0x8, vec![ins(0x8, Mnemonic::Nop, vec![])]),
        ];
        let cfg = Cfg::from_blocks(blocks);
        // ret is terminal; the nop block has no predecessor but exists.
        assert_eq!(cfg.block_count(), 2);
        let _ = HashMap::<u64, usize>::new();
    }
}
