//! Error type for the disassembly layer.

use thiserror::Error;

/// Errors arising from disassembly backends.
#[derive(Debug, Error)]
pub enum DisasmError {
    #[error("unsupported architecture for disassembly: {0}")]
    UnsupportedArchitecture(String),

    #[error("no code bytes supplied to disassembler")]
    EmptyInput,

    #[error("decoder error: {0}")]
    Backend(String),
}

/// Convenience result alias for the disassembly layer.
pub type Result<T> = std::result::Result<T, DisasmError>;
