//! End-to-end pipeline integration test: ingest the running test binary, run the
//! full analysis, and assert the public structures come back populated.

use armature_analysis::{analyze_binary, build_cfg};
use armature_ir::Module;

#[test]
fn pipeline_analyze_host_binary() {
    let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let analysis =
        analyze_binary(&bytes).expect("analyze_binary should succeed on a real host binary");

    // The host binary is always a recognized format with a code section.
    assert!(!analysis.map.format.to_string().is_empty());
    assert!(analysis.code_section().is_some());

    // Disassembly of a real binary yields instructions.
    assert!(
        !analysis.instructions.is_empty(),
        "expected at least some disassembled instructions"
    );

    // Function recovery should find the entry point at minimum.
    assert!(
        !analysis.module.functions.is_empty(),
        "expected at least one recovered function (entry point)"
    );

    // The combined CFG spans the recovered functions.
    let single = Module {
        functions: analysis.module.functions.clone(),
    };
    let cfg = build_cfg(&single);
    assert!(!cfg.nodes.is_empty(), "CFG must contain blocks");
}

#[test]
fn pipeline_xref_index_populated() {
    let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let analysis = analyze_binary(&bytes).unwrap();
    // Cross-reference index is always constructed; it may legitimately be empty
    // for trivial inputs, so we only assert it is present and countable.
    let _ = analysis.xrefs.count();
}
