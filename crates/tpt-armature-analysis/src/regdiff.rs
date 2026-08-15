//! Register-table / function similarity diffing across firmware / driver revisions.
//!
//! This is *not* a general BSim clone. It is a purpose-built, pairwise diff that
//! answers two practical questions when you have two builds of the same blob:
//!
//! 1. **Register-table similarity** — how alike are the MMIO register layouts?
//!    Computed as the Jaccard similarity over `(offset, width, rw-kind)` tuples
//!    (the base address is ignored, so a revision that kept the same register
//!    map at a moved base still scores high). Requires the [`RegisterTable`]
//!    produced by the `mmio` pass (the `regdiff` feature implies `mmio`).
//!
//! 2. **Function similarity** — which functions correspond across the two builds?
//!    Each function is reduced to a feature signature (a wildcarded-mnemonic
//!    histogram plus a few CFG-shape counts), and a Diaphora/BSim-style greedy
//!    bipartite matcher pairs the most similar unmatched functions above a
//!    similarity threshold. This half is standalone and needs no MMIO data.

use std::collections::HashMap;

#[cfg(feature = "mmio")]
use std::collections::HashSet;

use tpt_armature_ir::{Function, Mnemonic};

#[cfg(feature = "mmio")]
use crate::mmio::RegisterTable;

/// Jaccard similarity (0.0..=1.0) between two [`RegisterTable`]s over their
/// `(offset, width, rw-kind)` register tuples. The base address is ignored, so a
/// revision that keeps the same register layout at a different base still
/// matches. Two empty tables compare as `1.0`.
#[cfg(feature = "mmio")]
pub fn register_table_jaccard(a: &RegisterTable, b: &RegisterTable) -> f64 {
    let set_a: HashSet<(u64, u8, &'static str)> = a
        .registers
        .values()
        .flatten()
        .map(|e| (e.offset, e.width, e.rw_kind.as_str()))
        .collect();
    let set_b: HashSet<(u64, u8, &'static str)> = b
        .registers
        .values()
        .flatten()
        .map(|e| (e.offset, e.width, e.rw_kind.as_str()))
        .collect();
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    let inter = set_a.intersection(&set_b).count() as f64;
    let uni = set_a.union(&set_b).count() as f64;
    if uni == 0.0 {
        1.0
    } else {
        inter / uni
    }
}

/// A lightweight per-function feature signature used for similarity matching.
#[derive(Debug, Clone, Default)]
pub struct FunctionFeatures {
    /// Wildcarded-mnemonic histogram (operands stripped).
    pub mnemonics: HashMap<String, usize>,
    /// Number of basic blocks (CFG-shape proxy).
    pub blocks: usize,
    /// Total number of instructions.
    pub instructions: usize,
    /// Number of terminator (branch / call / ret) instructions.
    pub terminators: usize,
}

/// Build the feature signature for a recovered [`Function`].
pub fn function_features(func: &Function) -> FunctionFeatures {
    let mut mnemonics = HashMap::new();
    let mut instructions = 0usize;
    let mut terminators = 0usize;
    for block in &func.blocks {
        for ins in &block.instructions {
            *mnemonics.entry(mnemonic_key(&ins.mnemonic)).or_default() += 1;
            instructions += 1;
            if ins.mnemonic.is_terminator() {
                terminators += 1;
            }
        }
    }
    FunctionFeatures {
        mnemonics,
        blocks: func.blocks.len(),
        instructions,
        terminators,
    }
}

/// Wildcarded mnemonic key (operands are intentionally discarded).
fn mnemonic_key(m: &Mnemonic) -> String {
    match m {
        Mnemonic::Jcc(c) => format!("jcc:{c}"),
        other => other.to_string(),
    }
}

/// Cosine similarity of the mnemonic histograms in `[0, 1]`. Returns `0.0` if
/// either side is empty.
pub fn mnemonic_cosine(a: &FunctionFeatures, b: &FunctionFeatures) -> f64 {
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    let mut keys: Vec<&String> = a.mnemonics.keys().collect();
    for k in b.mnemonics.keys() {
        if !a.mnemonics.contains_key(k) {
            keys.push(k);
        }
    }
    for k in keys {
        let av = *a.mnemonics.get(k).unwrap_or(&0) as f64;
        let bv = *b.mnemonics.get(k).unwrap_or(&0) as f64;
        dot += av * bv;
        na += av * av;
        nb += bv * bv;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// Structural (CFG-shape) similarity in `[0, 1]`: the mean of pairwise
/// normalized distances over block / instruction / terminator counts.
pub fn structural_similarity(a: &FunctionFeatures, b: &FunctionFeatures) -> f64 {
    let score = |x: f64, y: f64| -> f64 {
        if x + y == 0.0 {
            1.0
        } else {
            1.0 - (x - y).abs() / (x + y)
        }
    };
    let w_blocks = score(a.blocks as f64, b.blocks as f64);
    let w_instr = score(a.instructions as f64, b.instructions as f64);
    let w_term = score(a.terminators as f64, b.terminators as f64);
    (w_blocks + w_instr + w_term) / 3.0
}

/// Combined function similarity: a weighted blend of mnemonic cosine (`0.7`) and
/// structural shape (`0.3`).
pub fn function_similarity(a: &FunctionFeatures, b: &FunctionFeatures) -> f64 {
    const W_MONEMONIC: f64 = 0.7;
    const W_STRUCTURAL: f64 = 0.3;
    W_MONEMONIC * mnemonic_cosine(a, b) + W_STRUCTURAL * structural_similarity(a, b)
}

/// A greedily-matched pair of functions across two binaries.
#[derive(Debug, Clone)]
pub struct FunctionMatch {
    /// Entry address of the function in binary A.
    pub a_addr: u64,
    /// Entry address of the function in binary B.
    pub b_addr: u64,
    /// Combined similarity score of the pair.
    pub similarity: f64,
}

/// Greedy bipartite matching (Diaphora / BSim-style).
///
/// All candidate `(a, b)` pairs with similarity at or above `threshold` are
/// sorted by descending similarity; each is then taken only if neither side has
/// already been matched. The result is a one-to-one (or one-to-zero) mapping
/// favouring the strongest correspondences first.
pub fn match_functions(a: &[Function], b: &[Function], threshold: f64) -> Vec<FunctionMatch> {
    let fa: Vec<(u64, FunctionFeatures)> =
        a.iter().map(|f| (f.start, function_features(f))).collect();
    let fb: Vec<(u64, FunctionFeatures)> =
        b.iter().map(|f| (f.start, function_features(f))).collect();

    let mut candidates: Vec<(usize, usize, f64)> = Vec::new();
    for (i, (_, fa_i)) in fa.iter().enumerate() {
        for (j, (_, fb_j)) in fb.iter().enumerate() {
            let s = function_similarity(fa_i, fb_j);
            if s >= threshold {
                candidates.push((i, j, s));
            }
        }
    }
    candidates.sort_by(|x, y| y.2.partial_cmp(&x.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut used_a = vec![false; fa.len()];
    let mut used_b = vec![false; fb.len()];
    let mut out = Vec::new();
    for (i, j, s) in candidates {
        if !used_a[i] && !used_b[j] {
            used_a[i] = true;
            used_b[j] = true;
            out.push(FunctionMatch {
                a_addr: fa[i].0,
                b_addr: fb[j].0,
                similarity: s,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_armature_ir::{BasicBlock, Instruction, Operand};

    fn ins(addr: u64, mnemonic: Mnemonic, operands: Vec<Operand>) -> Instruction {
        Instruction {
            address: addr,
            size: 4,
            mnemonic,
            operands,
            raw: Vec::new(),
            text: String::new(),
        }
    }

    fn func(start: u64, insts: Vec<Instruction>) -> Function {
        let end = insts
            .last()
            .map(|i| i.address + i.size as u64)
            .unwrap_or(start);
        Function {
            id: 0,
            start,
            name: None,
            blocks: vec![BasicBlock {
                id: 0,
                start,
                end,
                instructions: insts,
            }],
        }
    }

    #[test]
    fn identical_functions_match() {
        let f = func(
            0x1000,
            vec![
                ins(
                    0x1000,
                    Mnemonic::Mov,
                    vec![Operand::Reg("rax".into()), Operand::Imm(1)],
                ),
                ins(
                    0x1004,
                    Mnemonic::Add,
                    vec![Operand::Reg("rax".into()), Operand::Imm(2)],
                ),
                ins(0x1008, Mnemonic::Ret, vec![]),
            ],
        );
        let matches = match_functions(std::slice::from_ref(&f), std::slice::from_ref(&f), 0.0);
        assert_eq!(matches.len(), 1);
        assert!((matches[0].similarity - 1.0).abs() < 1e-9);
    }

    #[test]
    fn dissimilar_functions_do_not_match() {
        let a = func(
            0x1000,
            vec![
                ins(
                    0x1000,
                    Mnemonic::Mov,
                    vec![Operand::Reg("rax".into()), Operand::Imm(1)],
                ),
                ins(0x1004, Mnemonic::Ret, vec![]),
            ],
        );
        let b = func(
            0x2000,
            vec![
                ins(0x2000, Mnemonic::Push, vec![Operand::Reg("rbx".into())]),
                ins(0x2004, Mnemonic::Pop, vec![Operand::Reg("rbx".into())]),
                ins(0x2008, Mnemonic::Ret, vec![]),
            ],
        );
        let matches = match_functions(&[a], &[b], 0.9);
        assert!(
            matches.is_empty(),
            "unrelated functions must not match at high threshold"
        );
    }

    #[test]
    fn greedy_matching_is_one_to_one() {
        let base = func(
            0x1000,
            vec![
                ins(
                    0x1000,
                    Mnemonic::Mov,
                    vec![Operand::Reg("rax".into()), Operand::Imm(1)],
                ),
                ins(
                    0x1004,
                    Mnemonic::Add,
                    vec![Operand::Reg("rax".into()), Operand::Imm(2)],
                ),
                ins(0x1008, Mnemonic::Ret, vec![]),
            ],
        );
        let near = func(
            0x2000,
            vec![
                ins(
                    0x2000,
                    Mnemonic::Mov,
                    vec![Operand::Reg("rax".into()), Operand::Imm(1)],
                ),
                ins(
                    0x2004,
                    Mnemonic::Add,
                    vec![Operand::Reg("rax".into()), Operand::Imm(2)],
                ),
                ins(0x2008, Mnemonic::Ret, vec![]),
            ],
        );
        let far = func(
            0x3000,
            vec![
                ins(
                    0x3000,
                    Mnemonic::Mov,
                    vec![Operand::Reg("rbx".into()), Operand::Imm(9)],
                ),
                ins(0x3004, Mnemonic::Ret, vec![]),
            ],
        );
        let matches = match_functions(&[base], &[near, far], 0.0);
        assert_eq!(matches.len(), 1, "base must match exactly one function");
        assert_eq!(matches[0].b_addr, 0x2000);
    }

    #[cfg(feature = "mmio")]
    #[test]
    fn jaccard_identical_and_disjoint() {
        use crate::mmio::{RegisterEntry, RwKind};
        let mut t1 = RegisterTable::new();
        t1.insert(
            0xFEE0_0000,
            RegisterEntry::new(0x10, 4, RwKind::Write, 0x1000),
        );
        t1.insert(
            0xFEE0_0000,
            RegisterEntry::new(0x14, 4, RwKind::Read, 0x1004),
        );
        let mut t2 = RegisterTable::new();
        t2.insert(
            0x4000_0000,
            RegisterEntry::new(0x10, 4, RwKind::Write, 0x2000),
        );
        t2.insert(
            0x4000_0000,
            RegisterEntry::new(0x14, 4, RwKind::Read, 0x2004),
        );
        assert!((register_table_jaccard(&t1, &t2) - 1.0).abs() < 1e-9);

        let mut t3 = RegisterTable::new();
        t3.insert(
            0xFEE0_0000,
            RegisterEntry::new(0x20, 4, RwKind::Write, 0x3000),
        );
        let j = register_table_jaccard(&t1, &t3);
        assert!((j - 0.0).abs() < 1e-9, "disjoint tuples score 0, got {j}");

        let j_self = register_table_jaccard(&t1, &t1);
        assert!((j_self - 1.0).abs() < 1e-9);
    }

    #[cfg(feature = "mmio")]
    #[test]
    fn jaccard_partial_overlap() {
        use crate::mmio::{RegisterEntry, RwKind};
        let mut t1 = RegisterTable::new();
        t1.insert(
            0xFEE0_0000,
            RegisterEntry::new(0x10, 4, RwKind::Write, 0x1000),
        );
        t1.insert(
            0xFEE0_0000,
            RegisterEntry::new(0x14, 4, RwKind::Read, 0x1004),
        );
        let mut t2 = RegisterTable::new();
        t2.insert(
            0x4000_0000,
            RegisterEntry::new(0x10, 4, RwKind::Write, 0x2000),
        );
        t2.insert(
            0x4000_0000,
            RegisterEntry::new(0x18, 4, RwKind::Read, 0x2004),
        );
        let j = register_table_jaccard(&t1, &t2);
        assert!(
            (j - 1.0 / 3.0).abs() < 1e-9,
            "intersection 1 / union 3, got {j}"
        );
    }
}
