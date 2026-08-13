//! End-to-end pipeline: bytes -> MemoryMap -> IR -> analysis structures.

use crate::cfg::{build_cfg, Cfg};
use crate::dataflow::{analyze as df_analyze, DataFlow};
use crate::error::{AnalysisError, Result};
use crate::xref::{build_xrefs_for_module, XrefIndex};
use armature_disasm::for_architecture;
use armature_formats::{parse, MemoryMap, Section};
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
}

impl Analysis {
    /// Convenience: the executable section that was disassembled.
    pub fn code_section(&self) -> Option<&armature_formats::Section> {
        self.map.code_section()
    }
}

/// Run the full analysis pipeline over raw binary bytes.
pub fn analyze_binary(bytes: &[u8]) -> Result<Analysis> {
    let map = parse(bytes).map_err(|e| AnalysisError::Ingestion(e.to_string()))?;

    let code = map.code_section().ok_or(AnalysisError::NoCodeSection)?;

    let base = map.base_address + code.virt_addr;
    let disassembler =
        for_architecture(map.arch).map_err(|e| AnalysisError::Ingestion(e.to_string()))?;

    let instructions = disassembler
        .disassemble(&code.data, base)
        .map_err(|e| AnalysisError::Ingestion(e.to_string()))?;

    let entries = function_entries(&map, code, base);
    let mut functions = recover_functions(&instructions, &entries);

    // Name functions from their exported symbol, if any.
    let name_by_addr: std::collections::HashMap<u64, String> = map
        .exports
        .iter()
        .filter(|e| e.addr != 0)
        .map(|e| (e.addr, e.name.clone()))
        .collect();
    for f in &mut functions {
        if let Some(name) = name_by_addr.get(&f.start) {
            f.name = Some(name.clone());
        }
    }

    let module = Module { functions };

    let cfg = build_cfg(&module);
    let xrefs = build_xrefs_for_module(&module, &map.exports);
    let dataflow = df_analyze(&module);

    Ok(Analysis {
        map,
        instructions,
        module,
        cfg,
        xrefs,
        dataflow,
    })
}

/// Candidate function entry points: the binary entry point, every in-range
/// exported symbol, and (as a fallback for stripped binaries) the first
/// instruction of the code section.
fn function_entries(map: &MemoryMap, code: &Section, base: u64) -> Vec<u64> {
    let mut entries: Vec<u64> = Vec::new();
    if map.entry_point != 0 {
        entries.push(map.entry_point);
    }
    for e in &map.exports {
        if e.addr != 0 {
            entries.push(e.addr);
        }
    }
    let end = base + code.data.len() as u64;
    entries.retain(|&a| a >= base && a < end);
    if entries.is_empty() {
        entries.push(base);
    }
    entries
}
