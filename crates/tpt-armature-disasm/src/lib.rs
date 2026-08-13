//! Layer 2 — Disassembly ("Exposing the Framework").
//!
//! This crate wraps the native Rust disassembler backends (iced for x86/x64,
//! yaxpeax for ARM behind the `arm` feature) and lowers their output into the
//! shared [`tpt_armature_ir::Instruction`] representation.

pub mod disassembler;
pub mod error;
pub mod iced;

#[cfg(feature = "arm")]
pub mod yaxpeax_arm;

pub use disassembler::{for_architecture, Disassembler};
pub use error::{DisasmError, Result};
