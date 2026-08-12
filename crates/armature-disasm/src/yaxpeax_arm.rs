//! yaxpeax backend for ARM / AArch64 disassembly (feature `arm`).

use crate::disassembler::Disassembler;
use crate::error::Result;
use armature_ir::Instruction;

use yaxpeax_arch::*;

/// Disassembler backed by `yaxpeax` for 32-bit ARM or 64-bit AArch64.
pub struct YaxpeaxArm {
    /// `false` = 32-bit ARM (ArmArch), `true` = 64-bit AArch64.
    pub is64: bool,
}

impl YaxpeaxArm {
    /// Create an ARM backend. `is64` selects AArch64 vs 32-bit ARM.
    pub fn new(is64: bool) -> Self {
        YaxpeaxArm { is64 }
    }
}

impl Disassembler for YaxpeaxArm {
    fn disassemble(&self, code: &[u8], base: u64) -> Result<Vec<Instruction>> {
        if code.is_empty() {
            return Ok(Vec::new());
        }
        if self.is64 {
            Ok(disasm::<yaxpeax_arm::armv8::AArch64>(code, base))
        } else {
            Ok(disasm::<yaxpeax_arm::arm::ArmArch>(code, base))
        }
    }
}

fn disasm<A>(bytes: &[u8], base: u64) -> Vec<Instruction>
where
    A: Arch,
    A::Address: Into<u64> + Copy,
    A::Instruction: std::fmt::Display + std::fmt::Debug,
{
    let mut reader = U8Reader::new(bytes);
    let mut decoder = <A as Arch>::Decoder::default();
    let mut insn = <A as Arch>::Instruction::default();
    let mut out = Vec::new();

    loop {
        match decoder.decode(&mut reader, &mut insn) {
            Ok(()) => {
                let addr: u64 = insn.address.into();
                let text = format!("{insn}");
                let raw = bytes
                    .get((addr as usize)..)
                    .map(|s| s[..s.len().min(4)].to_vec())
                    .unwrap_or_default();
                out.push(Instruction {
                    address: base + addr,
                    size: 4,
                    mnemonic: armature_ir::Mnemonic::Other(format!("{:?}", insn).split(' ').next().unwrap_or("?").to_string().to_ascii_lowercase()),
                    operands: Vec::new(),
                    raw,
                    text,
                });
            }
            Err(_) => break,
        }
    }
    out
}
