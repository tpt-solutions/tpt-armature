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

    let mut pdb = pdb::PDB::open(Cursor::new(bytes))
        .map_err(|e| FormatError::DebugInfo { format: "PDB", message: e.to_string() })?;
    let symbols = pdb
        .global_symbols()
        .map_err(|e| FormatError::DebugInfo { format: "PDB", message: e.to_string() })?;
    let mut iter = symbols.iter();
    let mut out = Vec::new();
    while let Some(sym) = iter
        .next()
        .map_err(|e| FormatError::DebugInfo { format: "PDB", message: e.to_string() })?
    {
        let (offset, name, kind) = match sym.parse() {
            Ok(pdb::SymbolData::Public(p)) => {
                (p.offset, p.name.to_string().into_owned(), DebugSymbolKind::Function)
            }
            Ok(pdb::SymbolData::Data(d)) => {
                (d.offset, d.name.to_string().into_owned(), DebugSymbolKind::Data)
            }
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
    let pe = goblin::pe::PE::parse(pe_bytes)
        .map_err(|e| FormatError::DebugInfo { format: "PE", message: e.to_string() })?;
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
