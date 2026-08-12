//! The [`Disassembler`] trait and backend selection.

use crate::error::{DisasmError, Result};
use armature_formats::Architecture;
use armature_ir::Instruction;

/// A disassembly backend that lowers machine code into IR instructions.
pub trait Disassembler {
    /// Disassemble `code` (located at virtual address `base`) into IR.
    fn disassemble(&self, code: &[u8], base: u64) -> Result<Vec<Instruction>>;
}

/// Select the appropriate backend for an architecture.
///
/// x86/x64 use the iced backend. ARM/AArch64 use the yaxpeax backend, which is
/// only available when the `arm` feature is enabled.
pub fn for_architecture(arch: Architecture) -> Result<Box<dyn Disassembler>> {
    match arch {
        Architecture::X86 => Ok(Box::new(crate::iced::IcedDisassembler::new(32))),
        Architecture::X86_64 => Ok(Box::new(crate::iced::IcedDisassembler::new(64))),
        #[cfg(feature = "arm")]
        Architecture::Arm => Ok(Box::new(crate::yaxpeax_arm::YaxpeaxArm::new(false))),
        #[cfg(feature = "arm")]
        Architecture::Aarch64 => Ok(Box::new(crate::yaxpeax_arm::YaxpeaxArm::new(true))),
        other => Err(DisasmError::UnsupportedArchitecture(other.to_string())),
    }
}
