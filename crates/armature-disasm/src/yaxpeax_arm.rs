//! yaxpeax backend for ARM / AArch64 disassembly (feature `arm`).

use crate::disassembler::Disassembler;
use crate::error::Result;
use armature_ir::{Instruction, Mnemonic};
use yaxpeax_arch::{Arch, Decoder, U8Reader};

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

macro_rules! disasm_arch {
    ($arch:ty, $bytes:expr, $base:expr) => {{
        let mut reader = U8Reader::new($bytes);
        let decoder = <$arch as Arch>::Decoder::default();
        let mut out = Vec::new();
        let mut addr: u64 = 0;
        loop {
            match decoder.decode(&mut reader) {
                Ok(insn) => {
                    let text = format!("{insn}");
                    let raw = $bytes
                        .get(addr as usize..)
                        .map(|s| s.iter().take(4).copied().collect())
                        .unwrap_or_default();
                    let mnem = format!("{insn:?}")
                        .split(' ')
                        .next()
                        .unwrap_or("?")
                        .to_ascii_lowercase();
                    out.push(Instruction {
                        address: $base + addr,
                        size: 4,
                        mnemonic: Mnemonic::Other(mnem),
                        operands: Vec::new(),
                        raw,
                        text,
                    });
                    addr += 4;
                }
                Err(_) => break,
            }
        }
        out
    }};
}

impl Disassembler for YaxpeaxArm {
    fn disassemble(&self, code: &[u8], base: u64) -> Result<Vec<Instruction>> {
        if code.is_empty() {
            return Ok(Vec::new());
        }
        if self.is64 {
            Ok(disasm_arch!(yaxpeax_arm::armv8::a64::ARMv8, code, base))
        } else {
            Ok(disasm_arch!(yaxpeax_arm::armv7::ARMv7, code, base))
        }
    }
}
