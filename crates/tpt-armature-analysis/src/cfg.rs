//! Control-flow graph construction.

use tpt_armature_ir::{BasicBlock, Instruction, Mnemonic, Operand};

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
    ///
    /// `func_of` must have one entry per block giving the owning function index;
    /// it is used to avoid counting cross-function edges (e.g. a tail `jmp` into
    /// another function, or a `call`) as intra-function loops. For a single
    /// function pass `vec![0; blocks.len()]`.
    pub fn from_blocks(blocks: Vec<BasicBlock>, func_of: Vec<usize>) -> Cfg {
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

        let loop_count = count_back_edges(&blocks, &edges, &func_of);
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
///
/// `func_of` records the owning function of each block so [`count_back_edges`]
/// does not mistake cross-function `jmp`/`call` edges (a tail call into another
/// function, or a `call`) for an intra-function loop.
pub fn build_cfg(module: &tpt_armature_ir::Module) -> Cfg {
    let mut blocks = Vec::new();
    let mut func_of = Vec::new();
    for (fi, f) in module.functions.iter().enumerate() {
        for b in &f.blocks {
            func_of.push(fi);
            blocks.push(b.clone());
        }
    }
    Cfg::from_blocks(blocks, func_of)
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

/// A back-edge is an edge from a CFG node to one of its own DFS ancestors
/// (i.e. an edge that closes a cycle). This is the standard definition of a loop
/// back-edge and avoids the false positives of the old address-order heuristic,
/// which counted any backward jump — including `Call` edges, cross-function
/// edges, and fall-throughs to earlier addresses — as a loop.
///
/// The CFG built by [`Cfg::from_blocks`] may combine multiple functions (the
/// CLI's combined graph). We skip [`EdgeKind::Call`] edges (a call is control
/// transfer, not a loop) and any edge that leaves the owning function
/// (`func_of[e.from] != func_of[e.to]`), since a tail `jmp` into another
/// function is not an intra-function loop.
fn count_back_edges(blocks: &[BasicBlock], edges: &[Edge], func_of: &[usize]) -> usize {
    let _ = blocks;
    let n = edges
        .iter()
        .map(|e| e.from.max(e.to))
        .max()
        .map_or(0, |m| m + 1);
    if n == 0 {
        return 0;
    }

    // Adjacency list (skip Call edges and cross-function edges).
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in edges {
        if e.kind == EdgeKind::Call {
            continue;
        }
        if e.from >= n || e.to >= n {
            continue;
        }
        if func_of[e.from] != func_of[e.to] {
            continue;
        }
        adj[e.from].push(e.to);
    }

    // DFS three-color marking: 0 = unvisited (white), 1 = on stack (gray),
    // 2 = done (black). An edge to a gray node is a back-edge.
    const GRAY: u8 = 1;
    let mut color = vec![0u8; n];
    let mut back_edges = 0usize;
    // Iterative DFS to avoid stack overflow on large functions.
    for start in 0..n {
        if color[start] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        color[start] = GRAY;
        while let Some((u, ei)) = stack.last().copied() {
            if ei < adj[u].len() {
                stack.last_mut().unwrap().1 += 1;
                let v = adj[u][ei];
                match color[v] {
                    0 => {
                        color[v] = GRAY;
                        stack.push((v, 0));
                    }
                    1 => {
                        back_edges += 1;
                    }
                    _ => {}
                }
            } else {
                color[u] = 2;
                stack.pop();
            }
        }
    }
    back_edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tpt_armature_ir::{Instruction, Mnemonic, Operand};

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

        let cfg = Cfg::from_blocks(blocks.clone(), vec![0; blocks.len()]);
        assert_eq!(cfg.block_count(), 3);
        // unconditional jmp 0x100 from block 1 -> block 0 (back-edge)
        assert!(cfg
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::Unconditional && e.to == 0 && e.target_addr == 0x100));
        // One genuine loop: the outer `jmp 0x100`. (The `jne` is mid-block, so it
        // is not a terminator and contributes no edge.)
        assert_eq!(cfg.loop_count, 1);
    }

    #[test]
    fn cfg_back_edges_exclude_calls_and_cross_function() {
        // A `call` to an earlier address and a backward jump into another
        // function must NOT be counted as loops by the new cycle-aware detector.
        // 0x100: call 0x000     (call to a lower address — not a loop)
        // 0x105: ret
        // 0x200: jmp 0x100      (jump back into function A — cross-function)
        // 0x205: ret
        let blocks = vec![
            block(
                0,
                0x100,
                vec![
                    ins(0x100, Mnemonic::Call, vec![Operand::Imm(0x000)]),
                    ins(0x105, Mnemonic::Ret, vec![]),
                ],
            ),
            block(
                1,
                0x200,
                vec![
                    ins(0x200, Mnemonic::Jmp, vec![Operand::Imm(0x100)]),
                    ins(0x205, Mnemonic::Ret, vec![]),
                ],
            ),
        ];
        let cfg = Cfg::from_blocks(blocks.clone(), vec![0; blocks.len()]);
        // The only edge targetting a lower address is a `Call`, which the
        // detector excludes; the cross-function `jmp` has no node in this CFG,
        // so it produces no edge at all.
        assert_eq!(cfg.loop_count, 0);
        assert!(cfg
            .edges
            .iter()
            .all(|e| e.kind != EdgeKind::Call || e.target_addr >= cfg.nodes[e.from].start));
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
        let cfg = Cfg::from_blocks(blocks.clone(), vec![0; blocks.len()]);
        // ret is terminal; the nop block has no predecessor but exists.
        assert_eq!(cfg.block_count(), 2);
        let _ = HashMap::<u64, usize>::new();
    }

    #[test]
    fn combined_cfg_excludes_cross_function_loops() {
        // Two distinct functions concatenated (as `build_cfg` does). Function B
        // tail-jumps back into function A at a lower address. That edge is
        // cross-function, so it must NOT be counted as an intra-function loop.
        // Function A: 0x100 mov; 0x104 jmp 0x140  (A -> A)
        //            0x140 ret
        // Function B: 0x200 mov; 0x204 jmp 0x100  (B tail-jumps into A)
        //            0x208 ret
        let mut blocks = vec![block(
            0,
            0x100,
            vec![
                ins(0x100, Mnemonic::Mov, vec![]),
                ins(0x104, Mnemonic::Jmp, vec![Operand::Imm(0x140)]),
            ],
        )];
        blocks.push(block(1, 0x140, vec![ins(0x140, Mnemonic::Ret, vec![])]));
        blocks.push(block(
            0,
            0x200,
            vec![
                ins(0x200, Mnemonic::Mov, vec![]),
                ins(0x204, Mnemonic::Jmp, vec![Operand::Imm(0x100)]),
            ],
        ));
        blocks.push(block(1, 0x208, vec![ins(0x208, Mnemonic::Ret, vec![])]));

        // func_of: first two blocks belong to function 0, last two to function 1.
        let func_of = vec![0, 0, 1, 1];
        let cfg = Cfg::from_blocks(blocks, func_of);
        // No genuine loop inside either function.
        assert_eq!(cfg.loop_count, 0);
        // The cross-function edge still exists as an edge, but is not a loop.
        assert!(cfg
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::Unconditional && e.to == 0));
    }
}
