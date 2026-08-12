//! iced-x86 backend for x86 / x86_64 disassembly.

use crate::disassembler::Disassembler;
use crate::error::{DisasmError, Result};
use armature_ir::{Instruction, Mnemonic, Operand};

use iced_x86::Formatter;

/// Disassembler backed by [`iced_x86`].
pub struct IcedDisassembler {
    /// Bitness: 32 (x86) or 64 (x86_64).
    pub bits: u32,
}

impl IcedDisassembler {
    /// Create a backend for the given bitness (32 or 64).
    pub fn new(bits: u32) -> Self {
        IcedDisassembler { bits }
    }
}

impl Disassembler for IcedDisassembler {
    fn disassemble(&self, code: &[u8], base: u64) -> Result<Vec<Instruction>> {
        if code.is_empty() {
            return Err(DisasmError::EmptyInput);
        }
        if self.bits != 32 && self.bits != 64 {
            return Err(DisasmError::UnsupportedArchitecture(format!(
                "iced only supports 32/64 bits, got {}",
                self.bits
            )));
        }

        let mut decoder = iced_x86::Decoder::new(self.bits, code, iced_x86::DecoderOptions::NONE);
        decoder.set_ip(base);

        let mut formatter = iced_x86::GasFormatter::new();
        let mut out_text = String::new();
        let mut ins = iced_x86::Instruction::default();
        let mut result = Vec::new();

        while decoder.can_decode() {
            decoder.decode_out(&mut ins);
            let ip = ins.ip();
            let len = ins.len() as u32;
            if len == 0 {
                break;
            }

            out_text.clear();
            formatter.format(&ins, &mut out_text);

            let mnemonic = Mnemonic::from_str(&format!("{:?}", ins.mnemonic()).to_ascii_lowercase());
            let operands = extract_operands(&ins);

            let offset = (ip - base) as usize;
            let raw = code
                .get(offset..offset + len as usize)
                .unwrap_or(&[])
                .to_vec();

            result.push(Instruction {
                address: ip,
                size: len,
                mnemonic,
                operands,
                raw,
                text: out_text.trim().to_string(),
            });
        }

        Ok(result)
    }
}

fn reg_name(reg: iced_x86::Register) -> Option<String> {
    if reg == iced_x86::Register::None {
        None
    } else {
        Some(iced_x86::register_to_string(reg).to_string())
    }
}

fn extract_operands(ins: &iced_x86::Instruction) -> Vec<Operand> {
    let mut operands = Vec::new();
    for i in 0..ins.op_count() {
        match ins.op_kind(i) {
            iced_x86::OpKind::Register => {
                if let Some(name) = reg_name(ins.op_register(i)) {
                    operands.push(Operand::Reg(name));
                }
            }
            iced_x86::OpKind::Memory => {
                operands.push(Operand::Mem {
                    base: reg_name(ins.memory_base()),
                    index: reg_name(ins.memory_index()),
                    scale: ins.memory_index_scale() as u8,
                    disp: ins.memory_displacement() as i64,
                });
            }
            iced_x86::OpKind::NearBranch16
            | iced_x86::OpKind::NearBranch32
            | iced_x86::OpKind::NearBranch64 => {
                operands.push(Operand::Imm(ins.near_branch_target()));
            }
            iced_x86::OpKind::FarBranch16 | iced_x86::OpKind::FarBranch32 => {
                operands.push(Operand::Imm(ins.far_branch_target()));
            }
            _ => {
                if ins.is_immediate(i) {
                    operands.push(Operand::Imm(ins.immediate(i)));
                }
            }
        }
    }
    operands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disassemble_x86_64_nop_add() {
        // nop (0x90); mov rax, 1 (48 c7 c0 01 00 00 00); ret (c3)
        let code = [0x90u8, 0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, 0xc3];
        let d = IcedDisassembler::new(64);
        let ins = d.disassemble(&code, 0x400000).unwrap();
        assert_eq!(ins.len(), 3);
        assert_eq!(ins[0].mnemonic, Mnemonic::Nop);
        assert_eq!(ins[1].mnemonic, Mnemonic::Mov);
        assert_eq!(ins[1].operands[1], Operand::Imm(1));
        assert_eq!(ins[1].address, 0x400001);
        assert_eq!(ins[2].mnemonic, Mnemonic::Ret);
    }

    #[test]
    fn disassemble_branch_target() {
        // jmp 0x400005 relative: eb 03 ; nop ; nop ; nop  (relative +3 to ip after instr)
        let code = [0xebu8, 0x03, 0x90, 0x90, 0x90];
        let d = IcedDisassembler::new(64);
        let ins = d.disassemble(&code, 0x400000).unwrap();
        assert_eq!(ins[0].mnemonic, Mnemonic::Jmp);
        assert_eq!(ins[0].operands[0], Operand::Imm(0x400005));
    }
}
