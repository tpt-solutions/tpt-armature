//! MMIO / register-access mining pass.
//!
//! This module performs block-local base-pointer provenance tracking over raw
//! `Instruction` / `Operand` values (deliberately *not* `defs()` / `uses()`,
//! which drop memory-operand writes and flag `push`/`pop` register activity).
//! It identifies memory-mapped I/O (MMIO) accesses and clusters constant-offset
//! accesses into a [`RegisterTable`], which is useful when reverse-engineering
//! device drivers where hardware registers live in MMIO regions.
//!
//! The analysis tracks:
//! - Base-pointer provenance: which register currently holds an MMIO base
//!   address. Tracking is **block-local** — every basic block starts with an
//!   empty provenance set, so a register reused as a stack/frame pointer in one
//!   block can never leak an MMIO base into another.
//! - Constant offsets from that base.
//! - Read / write / read-write classification for each access.
//! - Per-base clustering of nearby constant offsets into register entries.
//!
//! ## Limitations
//!
//! - Indexed addressing (`[base + index * scale + disp]`) is *excluded*: the
//!   offset is not a fixed register offset, so it cannot be a statically known
//!   MMIO register and is skipped.
//! - Base provenance is reset whenever a tracked register is redefined by a
//!   non-base-loading instruction (e.g. `mov rbx, rsp`). This also means a
//!   base that is *incremented* (`add rbx, 4`) is dropped after the arithmetic,
//!   so the accesses after the increment are not attributed. That is an accepted
//!   imprecision of the linear provenance model.

use std::collections::{HashMap, HashSet};

use tpt_armature_ir::{Function, Instruction, Mnemonic, Module, Operand};

/// Read / write kind for a register access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RwKind {
    /// Register is read (load from MMIO).
    Read,
    /// Register is written (store to MMIO).
    Write,
    /// Register is both read and written (read-modify-write).
    ReadWrite,
}

impl RwKind {
    /// Merge two [`RwKind`]s, returning the more permissive one.
    pub fn merge(self, other: RwKind) -> RwKind {
        match (self, other) {
            (RwKind::Read, RwKind::Write) | (RwKind::Write, RwKind::Read) => RwKind::ReadWrite,
            (RwKind::ReadWrite, _) | (_, RwKind::ReadWrite) => RwKind::ReadWrite,
            (a, _) => a,
        }
    }

    /// Render the kind as a lower-cased token for export formats.
    pub fn as_str(self) -> &'static str {
        match self {
            RwKind::Read => "read",
            RwKind::Write => "write",
            RwKind::ReadWrite => "readwrite",
        }
    }
}

/// A single register entry in the register table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterEntry {
    /// Offset from the MMIO base address (in bytes).
    pub offset: u64,
    /// Width of the access in bytes (1, 2, 4, 8).
    pub width: u8,
    /// Read / write kind.
    pub rw_kind: RwKind,
    /// Addresses where this register is accessed (for cross-referencing).
    pub access_addresses: Vec<u64>,
    /// Optional name hint (e.g. from debug info or naming heuristics).
    pub name: Option<String>,
}

impl RegisterEntry {
    /// Create a new register entry.
    pub fn new(offset: u64, width: u8, rw_kind: RwKind, addr: u64) -> Self {
        Self {
            offset,
            width,
            rw_kind,
            access_addresses: vec![addr],
            name: None,
        }
    }

    /// Add another access to this entry.
    pub fn add_access(&mut self, rw_kind: RwKind, addr: u64) {
        self.rw_kind = self.rw_kind.merge(rw_kind);
        self.access_addresses.push(addr);
    }
}

/// A table of MMIO registers clustered by base address and offset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterTable {
    /// Base address -> vector of register entries at that base.
    pub registers: HashMap<u64, Vec<RegisterEntry>>,
}

impl RegisterTable {
    /// Create a new empty register table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all register entries for a given base address.
    pub fn get(&self, base: u64) -> Option<&Vec<RegisterEntry>> {
        self.registers.get(&base)
    }

    /// Get a mutable reference to register entries for a base address.
    pub fn get_mut(&mut self, base: u64) -> Option<&mut Vec<RegisterEntry>> {
        self.registers.get_mut(&base)
    }

    /// Insert or update a register entry.
    pub fn insert(&mut self, base: u64, entry: RegisterEntry) {
        let entries = self.registers.entry(base).or_default();
        if let Some(e) = entries
            .iter_mut()
            .find(|e| e.offset == entry.offset && e.width == entry.width)
        {
            e.add_access(entry.rw_kind, entry.access_addresses[0]);
        } else {
            entries.push(entry);
        }
    }

    /// Merge another register table into this one.
    pub fn merge(&mut self, other: RegisterTable) {
        for (base, entries) in other.registers {
            for entry in entries {
                self.insert(base, entry);
            }
        }
    }

    /// Total number of register entries across all bases.
    pub fn len(&self) -> usize {
        self.registers.values().map(|v| v.len()).sum()
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.registers.is_empty()
    }

    /// Get all base addresses in the table.
    pub fn bases(&self) -> Vec<u64> {
        let mut bases: Vec<u64> = self.registers.keys().cloned().collect();
        bases.sort();
        bases
    }
}

/// Configuration for MMIO analysis.
#[derive(Debug, Clone)]
pub struct MmioConfig {
    /// Known MMIO base address ranges `(start, end)`. If empty, heuristic
    /// detection is used.
    pub known_ranges: Vec<(u64, u64)>,
    /// Maximum offset from base to consider as a register (default: `0x10000`).
    pub max_offset: u64,
    /// Minimum number of accesses for a base to be reported as an MMIO region
    /// (default: `2`).
    pub min_accesses: usize,
}

impl Default for MmioConfig {
    fn default() -> Self {
        Self {
            known_ranges: Vec::new(),
            max_offset: 0x10000,
            min_accesses: 2,
        }
    }
}

/// Analyze a single basic block's instructions for MMIO / register accesses.
///
/// Base-pointer provenance is local to this block: the returned map is keyed by
/// MMIO base address and is *not* merged with any other block. Use
/// [`analyze_mmio_function`] / [`analyze_mmio_module`] to analyze a whole
/// function or module while keeping per-block provenance.
fn analyze_block(
    instructions: &[Instruction],
    config: &MmioConfig,
) -> HashMap<u64, Vec<RegisterEntry>> {
    let mut base_pointers: HashMap<String, HashSet<u64>> = HashMap::new();
    let mut local: HashMap<u64, Vec<RegisterEntry>> = HashMap::new();

    for ins in instructions {
        let mut loaded: Option<String> = None;

        // Base-pointer loads: `mov reg, imm` and `lea reg, [imm]` (absolute).
        if matches!(ins.mnemonic, Mnemonic::Mov | Mnemonic::Lea) && ins.operands.len() >= 2 {
            if let (Operand::Reg(dst), Operand::Imm(imm)) = (&ins.operands[0], &ins.operands[1]) {
                if is_potential_mmio_base(*imm, config) {
                    base_pointers.entry(dst.clone()).or_default().insert(*imm);
                    loaded = Some(dst.clone());
                }
            } else if let (
                Operand::Reg(dst),
                Operand::Mem {
                    base: None,
                    index: None,
                    disp,
                    ..
                },
            ) = (&ins.operands[0], &ins.operands[1])
            {
                if *disp >= 0 && is_potential_mmio_base(*disp as u64, config) {
                    base_pointers
                        .entry(dst.clone())
                        .or_default()
                        .insert(*disp as u64);
                    loaded = Some(dst.clone());
                }
            }
        }

        // Drop provenance for any register this instruction redefines, unless it
        // was just loaded as an MMIO base above. This is what breaks the
        // stack / MMIO-reuse hard case: `mov rbx, rsp` clobbers a previously
        // tracked MMIO base.
        for d in ins.defs() {
            if loaded.as_deref() != Some(d.as_str()) {
                base_pointers.remove(&d);
            }
        }

        // Memory accesses using a tracked base register.
        let rw_kind = access_rw_kind(ins);
        for operand in &ins.operands {
            if let Operand::Mem {
                base, index, disp, ..
            } = operand
            {
                // Indexed addressing: offset is not a fixed register offset.
                if index.is_some() {
                    continue;
                }
                if *disp < 0 {
                    continue;
                }
                let offset = *disp as u64;
                if offset > config.max_offset {
                    continue;
                }
                let Some(base_reg) = base else { continue };
                let Some(bases) = base_pointers.get(base_reg) else {
                    continue;
                };
                let width = access_width(ins, operand);
                let kind = rw_kind;
                for &mmio_base in bases {
                    let entry = RegisterEntry::new(offset, width, kind, ins.address);
                    match local.get_mut(&mmio_base) {
                        Some(entries) => {
                            if let Some(e) = entries
                                .iter_mut()
                                .find(|e| e.offset == offset && e.width == width)
                            {
                                e.add_access(kind, ins.address);
                            } else {
                                entries.push(entry);
                            }
                        }
                        None => {
                            local.insert(mmio_base, vec![entry]);
                        }
                    }
                }
            }
        }
    }

    local
}

/// Finalize a per-base map: drop bases with too few accesses and sort entries.
fn finalize(table: &mut RegisterTable, config: &MmioConfig) {
    table.registers.retain(|_, entries| {
        let total: usize = entries.iter().map(|e| e.access_addresses.len()).sum();
        total >= config.min_accesses
    });
    for entries in table.registers.values_mut() {
        entries.sort_by_key(|e| e.offset);
    }
}

/// Analyze a flat instruction slice as a single block.
pub fn analyze_mmio(instructions: &[Instruction], config: &MmioConfig) -> RegisterTable {
    let mut table = RegisterTable::new();
    let local = analyze_block(instructions, config);
    for (base, entries) in local {
        for entry in entries {
            table.insert(base, entry);
        }
    }
    finalize(&mut table, config);
    table
}

/// Analyze a recovered function, keeping base-pointer provenance block-local.
pub fn analyze_mmio_function(function: &Function, config: &MmioConfig) -> RegisterTable {
    let mut table = RegisterTable::new();
    for block in &function.blocks {
        let local = analyze_block(&block.instructions, config);
        for (base, entries) in local {
            for entry in entries {
                table.insert(base, entry);
            }
        }
    }
    finalize(&mut table, config);
    table
}

/// Analyze an entire module (all recovered functions).
pub fn analyze_mmio_module(module: &Module, config: &MmioConfig) -> RegisterTable {
    let mut table = RegisterTable::new();
    for function in &module.functions {
        let local = analyze_mmio_function(function, config);
        table.merge(local);
    }
    finalize(&mut table, config);
    table
}

/// Check if an immediate value looks like a potential MMIO base address.
fn is_potential_mmio_base(val: u64, config: &MmioConfig) -> bool {
    if !config.known_ranges.is_empty() {
        return config
            .known_ranges
            .iter()
            .any(|(start, end)| val >= *start && val < *end);
    }

    // Heuristics for common MMIO regions:
    // - x86 local APIC / MSI: 0xFEE00000 (page-aligned, high 32-bit).
    // - ARM / PCIe ECAM peripherals: high, large-aligned addresses.
    (val >= 0x8000_0000 && (val & 0xFFF) == 0)
        || (val >= 0xFFFF_0000_0000_0000 && (val & 0xFFFF) == 0)
}

/// Determine read / write kind from an instruction's operands.
fn access_rw_kind(ins: &Instruction) -> RwKind {
    match ins.mnemonic {
        Mnemonic::Mov => {
            if ins.operands.len() >= 2 {
                if matches!(ins.operands[0], Operand::Mem { .. }) {
                    RwKind::Write
                } else if matches!(ins.operands[1], Operand::Mem { .. }) {
                    RwKind::Read
                } else {
                    RwKind::ReadWrite
                }
            } else {
                RwKind::ReadWrite
            }
        }
        Mnemonic::Movzx | Mnemonic::Lea | Mnemonic::Push => RwKind::Read,
        Mnemonic::Pop => RwKind::Write,
        Mnemonic::Add | Mnemonic::Sub | Mnemonic::And | Mnemonic::Or | Mnemonic::Xor => {
            if ins
                .operands
                .iter()
                .any(|o| matches!(o, Operand::Mem { .. }))
            {
                RwKind::ReadWrite
            } else {
                RwKind::Read
            }
        }
        _ => RwKind::Read,
    }
}

/// Estimate access width in bytes from the register operand (best effort).
fn access_width(ins: &Instruction, mem: &Operand) -> u8 {
    for o in &ins.operands {
        if let Operand::Reg(r) = o {
            if o != mem {
                return register_width(r);
            }
        }
    }
    4
}

/// Best-effort register width in bytes from its name (x86 + ARM64 suffixes).
fn register_width(name: &str) -> u8 {
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if name.starts_with('x') && name.len() > 1 && digits(&name[1..]) {
        return 8;
    }
    if name.starts_with('w') && name.len() > 1 && digits(&name[1..]) {
        return 4;
    }
    if name.starts_with('e') {
        return 4;
    }
    if name.starts_with('r') {
        if name.ends_with('d') {
            return 4;
        }
        if name.ends_with('w') {
            return 2;
        }
        if name.ends_with('b') {
            return 1;
        }
        return 8;
    }
    if name.ends_with('w') {
        return 2;
    }
    if name.ends_with('l') || name.ends_with('b') || name.ends_with('h') {
        return 1;
    }
    if ["ax", "bx", "cx", "dx", "si", "di", "bp", "sp"].contains(&name) {
        return 2;
    }
    if [
        "al", "ah", "bl", "bh", "cl", "ch", "dl", "dh", "sil", "dil", "bpl", "spl",
    ]
    .contains(&name)
    {
        return 1;
    }
    4
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_armature_ir::{BasicBlock, Function, Instruction};

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

    fn block(id: usize, insts: Vec<Instruction>) -> BasicBlock {
        let start = insts.first().map(|i| i.address).unwrap_or(0);
        let end = insts.last().map(|i| i.address + i.size as u64).unwrap_or(0);
        BasicBlock {
            id,
            start,
            end,
            instructions: insts,
        }
    }

    fn func(blocks: Vec<BasicBlock>) -> Function {
        let start = blocks.first().map(|b| b.start).unwrap_or(0);
        Function {
            id: 0,
            start,
            name: None,
            blocks,
        }
    }

    #[test]
    fn basic_base_and_offsets() {
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0xFEE0_0000)],
            ),
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x10,
                    },
                    Operand::Reg("ebx".into()),
                ],
            ),
            ins(
                0x1008,
                Mnemonic::Mov,
                vec![
                    Operand::Reg("ecx".into()),
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x14,
                    },
                ],
            ),
        ];
        let table = analyze_mmio(&insts, &MmioConfig::default());
        assert_eq!(table.bases(), vec![0xFEE0_0000]);
        let regs = table.get(0xFEE0_0000).unwrap();
        assert_eq!(regs.len(), 2);
        let w = regs.iter().find(|e| e.offset == 0x10).unwrap();
        assert_eq!(w.rw_kind, RwKind::Write);
        assert_eq!(w.width, 4);
        let r = regs.iter().find(|e| e.offset == 0x14).unwrap();
        assert_eq!(r.rw_kind, RwKind::Read);
        assert_eq!(r.width, 4);
    }

    #[test]
    fn indexed_addressing_excluded() {
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0xFEE0_0000)],
            ),
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: Some("rbx".into()),
                        scale: 4,
                        disp: 0x10,
                    },
                    Operand::Reg("ecx".into()),
                ],
            ),
            ins(
                0x1008,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x20,
                    },
                    Operand::Reg("edx".into()),
                ],
            ),
            ins(
                0x100C,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x24,
                    },
                    Operand::Reg("edx".into()),
                ],
            ),
        ];
        let table = analyze_mmio(&insts, &MmioConfig::default());
        let regs = table.get(0xFEE0_0000).unwrap();
        assert_eq!(regs.len(), 2, "indexed access must be excluded");
        assert!(regs.iter().all(|e| e.offset >= 0x20));
    }

    #[test]
    fn stack_mmio_reuse_is_block_local() {
        let mmio_block = block(
            0,
            vec![
                ins(
                    0x1000,
                    Mnemonic::Mov,
                    vec![Operand::Reg("rbx".into()), Operand::Imm(0xFEE0_0000)],
                ),
                ins(
                    0x1004,
                    Mnemonic::Mov,
                    vec![
                        Operand::Mem {
                            base: Some("rbx".into()),
                            index: None,
                            scale: 1,
                            disp: 0x00,
                        },
                        Operand::Reg("eax".into()),
                    ],
                ),
                ins(
                    0x1008,
                    Mnemonic::Mov,
                    vec![
                        Operand::Mem {
                            base: Some("rbx".into()),
                            index: None,
                            scale: 1,
                            disp: 0x04,
                        },
                        Operand::Reg("eax".into()),
                    ],
                ),
            ],
        );
        let stack_block = block(
            1,
            vec![
                ins(
                    0x2000,
                    Mnemonic::Mov,
                    vec![Operand::Reg("rbx".into()), Operand::Reg("rsp".into())],
                ),
                ins(
                    0x2004,
                    Mnemonic::Mov,
                    vec![
                        Operand::Mem {
                            base: Some("rbx".into()),
                            index: None,
                            scale: 1,
                            disp: 0x00,
                        },
                        Operand::Reg("rcx".into()),
                    ],
                ),
            ],
        );
        let table =
            analyze_mmio_function(&func(vec![mmio_block, stack_block]), &MmioConfig::default());
        assert_eq!(table.bases(), vec![0xFEE0_0000]);
        let regs = table.get(0xFEE0_0000).unwrap();
        assert_eq!(
            regs.len(),
            2,
            "stack access must not be attributed to MMIO base"
        );
    }

    #[test]
    fn rw_kind_merges_on_repeat() {
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0xFEE0_0000)],
            ),
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x10,
                    },
                    Operand::Reg("ebx".into()),
                ],
            ),
            ins(
                0x1008,
                Mnemonic::Mov,
                vec![
                    Operand::Reg("ecx".into()),
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x10,
                    },
                ],
            ),
        ];
        let table = analyze_mmio(&insts, &MmioConfig::default());
        let regs = table.get(0xFEE0_0000).unwrap();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].offset, 0x10);
        assert_eq!(regs[0].rw_kind, RwKind::ReadWrite);
        assert_eq!(regs[0].access_addresses.len(), 2);
    }

    #[test]
    fn non_mmio_base_is_ignored() {
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0x1234)],
            ),
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x10,
                    },
                    Operand::Reg("ebx".into()),
                ],
            ),
        ];
        let table = analyze_mmio(&insts, &MmioConfig::default());
        assert!(table.is_empty(), "small immediate is not an MMIO base");
    }

    #[test]
    fn known_range_overrides_heuristic() {
        let cfg = MmioConfig {
            known_ranges: vec![(0x4000_0000, 0x4001_0000)],
            ..Default::default()
        };
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0x4000_1000)],
            ),
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x08,
                    },
                    Operand::Reg("ebx".into()),
                ],
            ),
            ins(
                0x1008,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x0C,
                    },
                    Operand::Reg("ebx".into()),
                ],
            ),
        ];
        let table = analyze_mmio(&insts, &cfg);
        assert_eq!(table.bases(), vec![0x4000_1000]);
    }

    #[test]
    fn width_detection_from_register() {
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0xFEE0_0000)],
            ),
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x00,
                    },
                    Operand::Reg("rax".into()),
                ],
            ),
            ins(
                0x1008,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x04,
                    },
                    Operand::Reg("ebx".into()),
                ],
            ),
            ins(
                0x100C,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rax".into()),
                        index: None,
                        scale: 1,
                        disp: 0x08,
                    },
                    Operand::Reg("ax".into()),
                ],
            ),
        ];
        let table = analyze_mmio(&insts, &MmioConfig::default());
        let regs = table.get(0xFEE0_0000).unwrap();
        let by_off = |o: u64| regs.iter().find(|e| e.offset == o).unwrap().width;
        assert_eq!(by_off(0x00), 8);
        assert_eq!(by_off(0x04), 4);
        assert_eq!(by_off(0x08), 2);
    }
}
