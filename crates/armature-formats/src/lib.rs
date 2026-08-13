//! Layer 1 — Ingestion ("Stripping the Casing").
//!
//! Raw bytes in, a standardized [`MemoryMap`] out. This crate abstracts over the
//! three mainstream executable container formats (PE, ELF, Mach-O) and exposes a
//! single architecture-agnostic view consumed by the analysis layer.

pub mod arch;
pub mod error;
pub mod map;
pub mod parse;

#[cfg(feature = "debuginfo")]
pub mod debuginfo;

pub use arch::Architecture;
pub use error::{FormatError, Result};
pub use map::{BinaryFormat, DebugSymbol, DebugSymbolKind, Export, Import, MemoryMap, Section};
pub use parse::parse;
