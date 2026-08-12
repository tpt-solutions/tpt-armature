//! Layer 4 — Extension ("The Custom Gears").
//!
//! Community extensibility without touching the core Rust code: Rhai scripting
//! for quick automation and sandboxed WebAssembly plugins for heavy duty. Both
//! backends are feature-gated so the default workspace build stays light.

pub mod error;

#[cfg(feature = "rhai")]
pub mod rhai_host;

#[cfg(feature = "wasm")]
pub mod wasm_host;

pub use error::{ExtError, Result};

#[cfg(feature = "rhai")]
pub use rhai_host::{default_rename_script, ScriptHost};

#[cfg(feature = "wasm")]
pub use wasm_host::{PluginApi, PluginHost};
