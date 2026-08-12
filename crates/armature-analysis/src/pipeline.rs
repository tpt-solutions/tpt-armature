//! End-to-end pipeline: bytes -> MemoryMap -> IR -> analysis structures.

use crate::cfg::{build_cfg, Cfg};
use crate::dataflow::{analyze as df_analyze, DataFlow};
use crate::error::{AnalysisError, Result};
use crate::xref::{build_xrefs_for_module, XrefIndex};
use armature_disasm::for_architecture;
use armature_formats::{parse, MemoryMap};
use armature_ir::{Instruction, IrBuilder, Module};

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

    let code = map
        .code_section()
        .ok_or(AnalysisError::NoCodeSection)?;

    let base = map.base_address + code.virt_addr;
    let disassembler = for_architecture(map.arch)
        .map_err(|e| AnalysisError::Ingestion(e.to_string()))?;

    let instructions = disassembler
        .disassemble(&code.data, base)
        .map_err(|e| AnalysisError::Ingestion(e.to_string()))?;

    let mut builder = IrBuilder::new();
    builder.extend(instructions.clone());
    let module = builder.build_module();

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
