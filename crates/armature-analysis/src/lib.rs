//! Layer 2 — Analysis ("Exposing the Framework").
//!
//! Consumes the IR produced by [`armature_disasm`] and builds the mathematical
//! structures analysts use: a control-flow graph ([`cfg`]), a cross-reference
//! index ([`xref`]), and a lightweight data-flow summary ([`dataflow`]).

pub mod cfg;
pub mod dataflow;
pub mod error;
pub mod pipeline;
pub mod xref;

pub use cfg::{build_cfg, Cfg, Edge, EdgeKind};
pub use dataflow::{analyze, DataFlow};
pub use error::{AnalysisError, Result};
pub use pipeline::{analyze_binary, Analysis};
pub use xref::{build_xrefs, Xref, XrefIndex, XrefKind};
