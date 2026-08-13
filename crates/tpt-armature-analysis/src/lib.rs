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

pub use cfg::{build_cfg, Cfg, Edge, EdgeKind};
pub use dataflow::{analyze, DataFlow};
pub use decompile::{decompile_function, decompile_module};
pub use error::{AnalysisError, Result};
pub use export::{analysis_to_json, export_renames, RenameFormat};
pub use pipeline::{analyze_binary, analyze_map, Analysis};
pub use strings::{extract_constants, extract_strings, ExtractedString, StringKind};
pub use xref::{build_xrefs, Xref, XrefIndex, XrefKind};
