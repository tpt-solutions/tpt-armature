//! Debug-information symbol import (feature `debuginfo`).
//!
//! Container import/export tables are frequently stripped. ELF `.symtab` symbols
//! are recovered inline in [`crate::parse`]; here we parse PDB public symbols so
//! PE binaries can also benefit from debug-derived names.

use crate::error::{FormatError, Result};
use crate::map::{DebugSymbol, DebugSymbolKind};
use std::io::Cursor;

/// A single symbol recovered from a PDB file, in PDB-internal (section, offset)
/// coordinates. Translate to a virtual address with the matching PE image via
/// [`PdbSymbol::to_va`].
pub struct PdbSymbol {
    /// Symbol name.
    pub name: String,
    /// PDB-internal section index (1-based, matching the PE section order).
    pub section: u16,
    /// Offset within the section.
    pub offset: u32,
    /// Symbol kind.
    pub kind: DebugSymbolKind,
}

impl PdbSymbol {
    /// Resolve this symbol to a virtual address given the PE image base and the
    /// virtual address of each PE section (in section order).
    pub fn to_va(&self, image_base: u64, section_vas: &[u64]) -> Option<u64> {
        let idx = (self.section as usize).checked_sub(1)?;
        let va = *section_vas.get(idx)?;
        Some(image_base + va + self.offset as u64)
    }
}

/// Parse public (and data) symbols from a PDB file's raw bytes (Windows PE
/// debug info). Returns symbols in PDB-internal coordinates; resolve them to
/// virtual addresses with the matching PE via [`PdbSymbol::to_va`].
pub fn parse_pdb(bytes: &[u8]) -> Result<Vec<PdbSymbol>> {
    use pdb::FallibleIterator;

    let mut pdb = pdb::PDB::open(Cursor::new(bytes)).map_err(|e| FormatError::DebugInfo {
        format: "PDB",
        message: e.to_string(),
    })?;
    let symbols = pdb.global_symbols().map_err(|e| FormatError::DebugInfo {
        format: "PDB",
        message: e.to_string(),
    })?;
    let mut iter = symbols.iter();
    let mut out = Vec::new();
    while let Some(sym) = iter.next().map_err(|e| FormatError::DebugInfo {
        format: "PDB",
        message: e.to_string(),
    })? {
        let (offset, name, kind) = match sym.parse() {
            Ok(pdb::SymbolData::Public(p)) => (
                p.offset,
                p.name.to_string().into_owned(),
                DebugSymbolKind::Function,
            ),
            Ok(pdb::SymbolData::Data(d)) => (
                d.offset,
                d.name.to_string().into_owned(),
                DebugSymbolKind::Data,
            ),
            _ => continue,
        };
        if name.is_empty() {
            continue;
        }
        out.push(PdbSymbol {
            name,
            section: offset.section,
            offset: offset.offset,
            kind,
        });
    }
    Ok(out)
}

/// Parse a PE and merge its PDB's public symbols (resolved to virtual addresses)
/// into the resulting [`MemoryMap`].
pub fn parse_with_pdb(pe_bytes: &[u8], pdb_bytes: &[u8]) -> Result<crate::map::MemoryMap> {
    let mut map = crate::parse(pe_bytes)?;
    let symbols = parse_pdb(pdb_bytes)?;
    if symbols.is_empty() {
        return Ok(map);
    }

    // Virtual address of each PE section, in section-header order, so a PDB
    // section index maps directly to `section_vas[index - 1]`.
    let pe = goblin::pe::PE::parse(pe_bytes).map_err(|e| FormatError::DebugInfo {
        format: "PE",
        message: e.to_string(),
    })?;
    let section_vas: Vec<u64> = pe
        .sections
        .iter()
        .map(|s| s.virtual_address as u64)
        .collect();

    for sym in symbols {
        if let Some(addr) = sym.to_va(map.base_address, &section_vas) {
            map.debug_symbols.push(DebugSymbol {
                name: sym.name,
                addr,
                kind: sym.kind,
            });
        }
    }
    Ok(map)
}

/// Extract function names and addresses from DWARF debug info in an ELF image
/// (feature `debuginfo`). Each `DW_TAG_subprogram` with a `DW_AT_name` and a
/// `DW_AT_low_pc` becomes a [`DebugSymbol`], enriching the function names that
/// flow into `function.name` during analysis. Any parsing failure degrades
/// gracefully to "no DWARF" so a missing or malformed debug section never breaks
/// binary ingestion.
#[cfg(feature = "debuginfo")]
pub fn parse_dwarf_elf(elf: &goblin::elf::Elf<'_>, bytes: &[u8]) -> Vec<DebugSymbol> {
    use gimli::{EndianSlice, LittleEndian, SectionId};

    // Borrow each named ELF section's bytes from `bytes` so the slices outlive
    // the `Dwarf` we build (the closure returns `EndianSlice`s tied to `bytes`).
    let section_bytes = |name: &str| -> Option<&[u8]> {
        for sh in &elf.section_headers {
            if elf
                .shdr_strtab
                .get_at(sh.sh_name)
                .map(|s| s == name)
                .unwrap_or(false)
            {
                if let Some(r) = sh.file_range() {
                    if r.end <= bytes.len() {
                        return Some(&bytes[r.start..r.end]);
                    }
                }
            }
        }
        None
    };

    let dwarf = gimli::Dwarf::load(
        |id: SectionId| -> std::result::Result<EndianSlice<LittleEndian>, gimli::Error> {
            Ok(EndianSlice::new(
                section_bytes(id.name()).unwrap_or(&[]),
                LittleEndian,
            ))
        },
    );
    let dwarf = match dwarf {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    fn visit(
        node: gimli::EntriesTreeNode<EndianSlice<LittleEndian>>,
        dwarf: &gimli::Dwarf<EndianSlice<LittleEndian>>,
        out: &mut Vec<DebugSymbol>,
    ) {
        {
            let entry = node.entry();
            // DW_TAG_subprogram == 0x2e
            if entry.tag().0 == 0x2e {
                let mut name = None;
                let mut low_pc = None;
                let mut attrs = entry.attrs();
                while let Some(attr) = attrs.next().unwrap_or(None) {
                    match attr.name().0 {
                        // DW_AT_name == 0x03
                        0x03 => {
                            if let Some(slice) = attr.string_value(&dwarf.debug_str) {
                                if let Ok(s) = slice.to_string() {
                                    name = Some(s.to_string());
                                }
                            }
                        }
                        // DW_AT_low_pc == 0x11
                        0x11 => {
                            low_pc = attr.udata_value();
                        }
                        _ => {}
                    }
                }
                if let (Some(name), Some(low_pc)) = (name, low_pc) {
                    out.push(DebugSymbol {
                        name,
                        addr: low_pc,
                        kind: DebugSymbolKind::Function,
                    });
                }
            }
        }
        let mut children = node.children();
        while let Some(child) = children.next().unwrap_or(None) {
            visit(child, dwarf, out);
        }
    }

    let mut out = Vec::new();
    let mut units = dwarf.units();
    while let Ok(Some(header)) = units.next() {
        let unit = match dwarf.unit(header) {
            Ok(u) => u,
            Err(_) => continue,
        };
        let mut tree = match unit.entries_tree(None) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let root = match tree.root() {
            Ok(r) => r,
            Err(_) => continue,
        };
        visit(root, &dwarf, &mut out);
    }
    out
}
