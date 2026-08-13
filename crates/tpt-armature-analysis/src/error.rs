//! Error type for the analysis layer.

use thiserror::Error;

/// Errors produced by analysis passes.
#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("the IR module contains no instructions to analyze")]
    EmptyModule,
    #[error("the binary contains no executable (.text) section to analyze")]
    NoCodeSection,
    #[error("ingestion / disassembly failure: {0}")]
    Ingestion(String),
}

/// Convenience result alias for the analysis layer.
pub type Result<T> = std::result::Result<T, AnalysisError>;
