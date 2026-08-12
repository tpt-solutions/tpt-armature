//! Error type for the extension layer.

use thiserror::Error;

/// Errors produced by scripting / plugin hosts.
#[derive(Debug, Error)]
pub enum ExtError {
    #[cfg(feature = "rhai")]
    #[error("rhai script error: {0}")]
    Rhai(String),

    #[cfg(feature = "wasm")]
    #[error("wasm plugin error: {0}")]
    Wasm(String),

    #[error("the requested extension backend is not compiled in (enable a feature)")]
    BackendUnavailable,
}

/// Convenience result alias for the extension layer.
pub type Result<T> = std::result::Result<T, ExtError>;
