//! Error type for the formats layer.

use thiserror::Error;

/// Errors produced while ingesting a raw binary.
#[derive(Debug, Error)]
pub enum FormatError {
    #[error("unsupported or unrecognized binary container (magic mismatch)")]
    Unrecognized,

    #[error("failed to parse {format} container: {source}")]
    Parse {
        format: &'static str,
        #[source]
        source: goblin::error::Error,
    },

    #[error("section '{0}' has an out-of-range file offset/size")]
    SectionRange(String),

    #[error("the binary contains no executable (.text) section to analyze")]
    NoCodeSection,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result alias for the formats layer.
pub type Result<T> = std::result::Result<T, FormatError>;
