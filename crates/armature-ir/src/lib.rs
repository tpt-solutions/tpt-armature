//! Layer 2 — custom Intermediate Representation ("Exposing the Framework").
//!
//! The IR is the lingua franca between the disassembly backends and the
//! analysis passes. Every backend (iced, yaxpeax, ...) lowers its native
//! instruction representation into the [`Instruction`] / [`Operand`] / [`Mnemonic`]
//! types here, and every analysis (CFG, data-flow, X-refs) consumes them.

pub mod builder;
pub mod operand;
pub mod instr;

pub use builder::{IrBuilder, Module};
pub use instr::{BasicBlock, Function, Instruction, Mnemonic};
pub use operand::Operand;
