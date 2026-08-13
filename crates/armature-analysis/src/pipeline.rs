//! End-to-end pipeline: bytes -> MemoryMap -> IR -> analysis structures.

use crate::cfg::{build_cfg, Cfg};
use crate::dataflow::{analyze as df_analyze, DataFlow};
use crate::error::{AnalysisError, Result};
use crate::xref::{build_xrefs_for_module, XrefIndex};
use armature_disasm::for_architecture;
use armature_formats::{parse, MemoryMap};
use crate::strings::{extract_constants, extract_strings, ExtractedString};
use armature_ir::{recover_functions, Instruction, Module};

/// The fully analyzed binary: every layer's output in one bundle.
#[derive(Debug)]
pub struct Analysis {
    /// Layer 1 output.
    pub map: MemoryMap,
    /// Decoded instructions (address order).
    pub instructions: Vec<Instruction>,
    /// IR module (single linear-sweep function).
    pub module: Module,
    /// Control-flow graph.
    pub cfg: Cfg,
    /// Cross-reference index.
    pub xrefs: XrefIndex,
    /// Data-flow summary.
    pub dataflow: DataFlow,
    /// Printable strings discovered in the image.
    pub strings: Vec<ExtractedString>,
    /// Distinct immediate constants referenced by the instructions.
    pub constants: Vec<u64>,
}

impl Analysis {
    /// Convenience: the executable section that was disassembled.
    pub fn code_section(&self) -> Option<&armature_formats::Section> {
        self.map.code_section()
    }
}

/// Run the full analysis pipeline over a pre-parsed [`MemoryMap`] (e.g. one
/// produced with debug-info symbols already merged in).
pub fn analyze_map(map: MemoryMap) -> Result<Analysis> {
    // Require at least one executable section before disassembling.
    map.code_section().ok_or(AnalysisError::NoCodeSection)?;

    let disassembler =
        for_architecture(map.arch).map_err(|e| AnalysisError::Ingestion(e.to_string()))?;

    // Disassemble *every* executable section so multi-section, packed, and
    // Mach-O-stub binaries are fully covered (not just the first `.text`).
    let mut instructions: Vec<Instruction> = Vec::new();
    for section in map.executable_sections() {
        let base = map.base_address + section.virt_addr;
        let insns = disassembler
            .disassemble(&section.data, base)
            .map_err(|e| AnalysisError::Ingestion(e.to_string()))?;
        instructions.extend(insns);
    }
    instructions.sort_by_key(|i| i.address);

    let entries = function_entries(&map);
    let mut functions = recover_functions(&instructions, &entries);

    // Name functions from their exported symbol, or from recovered debug-info
    // symbols, if either provides a name for the function's entry address.
    let mut name_by_addr: std::collections::HashMap<u64, String> = map
        .exports
        .iter()
        .filter(|e| e.addr != 0)
        .map(|e| (e.addr, e.name.clone()))
        .collect();
    for sym in &map.debug_symbols {
        if sym.addr != 0 && !name_by_addr.contains_key(&sym.addr) {
            name_by_addr.insert(sym.addr, sym.name.clone());
        }
    }
    for f in &mut functions {
        if let Some(name) = name_by_addr.get(&f.start) {
            f.name = Some(name.clone());
        }
    }

    let module = Module { functions };

    let cfg = build_cfg(&module);
    let xrefs = build_xrefs_for_module(&module, &map.exports);
    let dataflow = df_analyze(&module);
    let strings = extract_strings(&map, 4);
    let constants = extract_constants(&instructions, 500);

    Ok(Analysis {
        map,
        instructions,
        module,
        cfg,
        xrefs,
        dataflow,
        strings,
        constants,
    })
}

/// Run the full analysis pipeline over raw binary bytes.
pub fn analyze_binary(bytes: &[u8]) -> Result<Analysis> {
    let map = parse(bytes).map_err(|e| AnalysisError::Ingestion(e.to_string()))?;
    analyze_map(map)
}

/// Candidate function entry points: the binary entry point, every in-range
/// exported symbol, and (as a fallback for stripped binaries) the first
/// instruction of the primary code section.
fn function_entries(map: &MemoryMap) -> Vec<u64> {
    let code = match map.code_section() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let base = map.base_address + code.virt_addr;
    let end = base + code.data.len() as u64;
    let mut entries: Vec<u64> = Vec::new();
    if map.entry_point != 0 {
        entries.push(map.entry_point);
    }
    for e in &map.exports {
        if e.addr != 0 {
            entries.push(e.addr);
        }
    }
    entries.retain(|&a| a >= base && a < end);
    if entries.is_empty() {
        entries.push(base);
    }
    entries
}
