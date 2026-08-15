//! Layer 2 — Analysis ("Exposing the Framework").
//!
//! Consumes the IR produced by [`tpt_armature_disasm`] and builds the mathematical
//! structures analysts use: a control-flow graph ([`crate::cfg`]), a
//! cross-reference index ([`crate::xref`]), and a lightweight data-flow summary
//! ([`crate::dataflow`]).

pub mod cfg;
pub mod dataflow;
pub mod decompile;
pub mod error;
pub mod export;
pub mod pipeline;
pub mod strings;
pub mod xref;

#[cfg(feature = "clean-room")]
pub mod clean_room;
#[cfg(feature = "driver-pe")]
pub mod driver;
#[cfg(feature = "regdiff")]
pub mod regdiff;
#[cfg(feature = "mmio")]
pub mod mmio;

pub use cfg::{build_cfg, Cfg, Edge, EdgeKind};
pub use dataflow::{analyze, DataFlow};
pub use decompile::{decompile_function, decompile_module};
#[cfg(feature = "driver-pe")]
pub use driver::{
    extract_ioctls, recover_dispatch, recover_driver_profile, DriverProfile, Ioctl,
    IrpMajorFunction,
};
#[cfg(feature = "clean-room")]
pub use clean_room::{export, Artifact, CleanRoomExport, CleanRoomSource, Manifest};
pub use error::{AnalysisError, Result};
pub use export::{analysis_to_json, export_renames, parse_renames, RenameFormat};
#[cfg(feature = "mmio")]
pub use export::{export_register_table, RegisterTableFormat};
#[cfg(feature = "mmio")]
pub use mmio::{
    analyze_mmio, analyze_mmio_function, analyze_mmio_module, MmioConfig, RegisterEntry,
    RegisterTable, RwKind,
};
pub use pipeline::{analyze_binary, analyze_map, Analysis};
#[cfg(feature = "regdiff")]
pub use regdiff::register_table_jaccard;
#[cfg(feature = "regdiff")]
pub use regdiff::{
    function_features, function_similarity, match_functions, mnemonic_cosine,
    structural_similarity, FunctionFeatures, FunctionMatch,
};
pub use strings::{extract_constants, extract_strings, ExtractedString, StringKind};
pub use xref::{build_xrefs, Xref, XrefIndex, XrefKind};
