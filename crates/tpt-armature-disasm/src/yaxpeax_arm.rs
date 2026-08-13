//! yaxpeax backend for ARM / AArch64 disassembly (feature `arm`).
//!
//! Lowers yaxpeax-decoded instructions into the `tpt-armature` IR. Unlike the
//! previous implementation (which emitted every instruction as `Mnemonic::Other`
//! with no operands, leaving CFG / function recovery / decompiler / data-flow
//! inert for ARM), this module classifies real mnemonics and extracts branch
//! targets so the analysis passes operate on ARM code.

use crate::disassembler::Disassembler;
use crate::error::Result;
use tpt_armature_ir::{Instruction, Mnemonic, Operand};
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
                    // A32/A64 instructions are fixed 4 bytes; yaxpeax does not
                    // decode T32/Thumb here, so the step is always 4.
                    let size = 4u32;
                    let text = format!("{insn}");
                    let mnem_raw = format!("{insn:?}")
                        .split(' ')
                        .next()
                        .unwrap_or("?")
                        .to_string();
                    let mnemonic = classify(&mnem_raw);
                    let insn_addr = $base + addr;
                    let operands = extract_operands(&text, &mnemonic, insn_addr);
                    let raw = $bytes
                        .get(addr as usize..)
                        .map(|s| s.iter().take(size as usize).copied().collect())
                        .unwrap_or_default();
                    out.push(Instruction {
                        address: insn_addr,
                        size,
                        mnemonic,
                        operands,
                        raw,
                        text,
                    });
                    addr += size as u64;
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

/// Classify a yaxpeax debug-mnemonic string into an IR [`Mnemonic`].
///
/// yaxpeax renders the variant as the first whitespace-delimited token of its
/// `Debug` output (e.g. `Mov`, `B`, `Bl`, `Ret`, `B.Eq`), so we match on that.
fn classify(raw: &str) -> Mnemonic {
    let lower = raw.trim().to_ascii_lowercase();

    // Conditional branches: `b.eq`, `b.ne`, ... (A64) or `b<Eq>`-style variants.
    if let Some(cond) = lower.strip_prefix("b.") {
        return Mnemonic::Jcc(cond.to_string());
    }

    match lower.as_str() {
        "ret" | "eret" | "eret aa64" => Mnemonic::Ret,
        "br" => Mnemonic::Jmp,
        "b" => Mnemonic::Jmp,
        "bl" | "blr" => Mnemonic::Call,
        "bx" | "blx" => Mnemonic::Call,
        "cbz" | "cbnz" | "tbz" | "tbnz" => Mnemonic::Jcc(String::new()),
        "mov" | "movz" | "movn" | "movk" | "movt" => Mnemonic::Mov,
        "add" | "adds" => Mnemonic::Add,
        "sub" | "subs" => Mnemonic::Sub,
        "mul" | "madd" | "msub" | "mneg" => Mnemonic::Mul,
        "udiv" | "sdiv" => Mnemonic::Div,
        "and" | "ands" => Mnemonic::And,
        "orr" | "orn" | "or" => Mnemonic::Or,
        "eor" | "eon" | "eor3" => Mnemonic::Xor,
        "lsl" | "lslv" => Mnemonic::Shl,
        "lsr" | "lsrv" | "asr" | "asrv" => Mnemonic::Shr,
        "cmp" | "cmn" => Mnemonic::Cmp,
        "tst" => Mnemonic::Test,
        "push" => Mnemonic::Push,
        "pop" => Mnemonic::Pop,
        "nop" => Mnemonic::Nop,
        // Everything else (ldr/str/ldp/stp/brk/svc/...) is kept verbatim.
        _ => Mnemonic::Other(lower),
    }
}

/// Build operands for a classified ARM instruction.
///
/// * Branch/call instructions get their (relative) target as an `Imm` so the CFG
///   pass can wire up edges.
/// * Arithmetic/move/compare instructions get the first register as the
///   destination (so `defs()` is populated) plus any immediate; memory operands
///   are captured as `Mem` when present.
fn extract_operands(text: &str, mnemonic: &Mnemonic, insn_addr: u64) -> Vec<Operand> {
    let mut ops = Vec::new();

    if matches!(mnemonic, Mnemonic::Jmp | Mnemonic::Jcc(_) | Mnemonic::Call) {
        if let Some(target) = parse_branch_target(text, insn_addr) {
            ops.push(Operand::Imm(target));
        }
        return ops;
    }

    if matches!(
        mnemonic,
        Mnemonic::Mov
            | Mnemonic::Add
            | Mnemonic::Sub
            | Mnemonic::Mul
            | Mnemonic::Div
            | Mnemonic::And
            | Mnemonic::Or
            | Mnemonic::Xor
            | Mnemonic::Shl
            | Mnemonic::Shr
            | Mnemonic::Cmp
            | Mnemonic::Test
            | Mnemonic::Push
            | Mnemonic::Pop
    ) {
        if let Some(mem) = mem_operand(text) {
            ops.push(mem);
        }
        if let Some(reg) = first_reg(text) {
            ops.push(Operand::Reg(reg));
        }
        if let Some(imm) = first_imm(text) {
            ops.push(Operand::Imm(imm));
        }
    }

    ops
}

/// Parse a branch target from the disassembly text.
///
/// yaxpeax renders branch immediates as a relative offset from the instruction
/// (e.g. `b #0x14` or `b #-0x8`), so the absolute target is `insn_addr + offset`.
fn parse_branch_target(text: &str, insn_addr: u64) -> Option<u64> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for `0x` (optionally preceded by `#` and/or `-`).
        if (bytes[i] == b'0' && i + 1 < bytes.len() && bytes[i + 1] == b'x')
            || (i + 1 < bytes.len()
                && (bytes[i] == b'#' || bytes[i] == b'-')
                && bytes[i + 1] == b'0'
                && i + 2 < bytes.len()
                && bytes[i + 2] == b'x')
        {
            let neg = bytes[i] == b'-';
            let start = if bytes[i] == b'#' { i + 1 } else { i };
            let hex_start = if bytes[start] == b'-' { start + 1 } else { start };
            // Skip `0x`.
            let mut j = hex_start + 2;
            let digits_start = j;
            while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
                j += 1;
            }
            if j > digits_start {
                let hex = &text[digits_start..j];
                if let Ok(val) = i64::from_str_radix(hex, 16) {
                    let signed = if neg { -val } else { val };
                    let abs = (insn_addr as i64 + signed).max(0) as u64;
                    return Some(abs);
                }
            }
        }
        i += 1;
    }
    None
}

/// Find the first register-like token in a disassembly string.
fn first_reg(text: &str) -> Option<String> {
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if !c.is_alphabetic() {
            continue;
        }
        // Collect the whole word starting at this alphabetic character.
        let mut word = String::new();
        word.push(c);
        while let Some(&(_, nc)) = chars.peek() {
            if nc.is_alphanumeric() {
                word.push(nc);
                chars.next();
            } else {
                break;
            }
        }
        if is_reg(word.as_str()) {
            return Some(word);
        }
    }
    None
}

/// Parse the first immediate (`#0x...` / `#123`) from a disassembly string.
fn first_imm(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' && i + 1 < bytes.len() {
            let mut j = i + 1;
            let neg = bytes[j] == b'-';
            if neg {
                j += 1;
            }
            if j + 1 < bytes.len() && bytes[j] == b'0' && bytes[j + 1] == b'x' {
                let hex_start = j + 2;
                let mut k = hex_start;
                while k < bytes.len() && bytes[k].is_ascii_hexdigit() {
                    k += 1;
                }
                if k > hex_start {
                    if let Ok(v) = u64::from_str_radix(&text[hex_start..k], 16) {
                        return Some(v);
                    }
                }
            } else if bytes[j].is_ascii_digit() {
                let mut k = j;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if let Ok(v) = text[j..k].parse::<u64>() {
                    return Some(v);
                }
            }
        }
        i += 1;
    }
    None
}

/// Build a `Mem` operand from the first `[ ... ]` memory expression in `text`.
fn mem_operand(text: &str) -> Option<Operand> {
    let open = text.find('[')?;
    let close = text[open..].find(']')? + open;
    let inner = &text[open + 1..close];
    let mut base = None;
    let mut index = None;
    let mut scale: u8 = 1;
    for part in inner.split(|c| c == ',' || c == '+' || c == '-' || c == '*' || c == ' ') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Ok(_imm) = p.parse::<i64>() {
            // displacement; ignored for the simple IR Mem model
            continue;
        }
        if is_reg(p) {
            if base.is_none() {
                base = Some(p.to_string());
            } else {
                index = Some(p.to_string());
            }
        }
        if let Some(stripped) = p.strip_prefix('*') {
            if let Ok(s) = stripped.parse::<u8>() {
                scale = s;
            }
        }
    }
    if base.is_none() && index.is_none() {
        return None;
    }
    Some(Operand::Mem {
        base,
        index,
        scale,
        disp: 0,
    })
}

/// Heuristic register-name check covering the common A32/A64 registers.
fn is_reg(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "sp" | "pc" | "lr" | "fp" | "ip" | "sl" | "sb" | "xzr" | "wzr"
    ) {
        return true;
    }
    let first = lower.chars().next();
    match first {
        Some('x') | Some('w') | Some('v') | Some('q') | Some('b') | Some('h')
        | Some('s') | Some('d') | Some('r') => lower[1..].chars().all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_mnemonics() {
        assert_eq!(classify("Mov"), Mnemonic::Mov);
        assert_eq!(classify("ADD"), Mnemonic::Add);
        assert_eq!(classify("SUB"), Mnemonic::Sub);
        assert_eq!(classify("RET"), Mnemonic::Ret);
        assert_eq!(classify("B"), Mnemonic::Jmp);
        assert_eq!(classify("BL"), Mnemonic::Call);
        assert_eq!(classify("BLR"), Mnemonic::Call);
        assert_eq!(classify("BR"), Mnemonic::Jmp);
        assert_eq!(classify("B.EQ"), Mnemonic::Jcc("eq".into()));
        assert_eq!(classify("B.NE"), Mnemonic::Jcc("ne".into()));
        assert_eq!(classify("CBZ"), Mnemonic::Jcc(String::new()));
        assert_eq!(classify("NOP"), Mnemonic::Nop);
        assert_eq!(classify("LDR"), Mnemonic::Other("ldr".into()));
    }

    #[test]
    fn branch_target_is_relative_to_instruction() {
        // `b #0x14` at address 0x1000 -> 0x1014.
        assert_eq!(parse_branch_target("b #0x14", 0x1000), Some(0x1014));
        // Backward branch `b #-0x8` at 0x1000 -> 0x0ff8.
        assert_eq!(parse_branch_target("b #-0x8", 0x1000), Some(0x0ff8));
    }

    #[test]
    fn extracts_destination_register_and_imm() {
        let ops = extract_operands("mov x0, #1", &Mnemonic::Mov, 0x1000);
        assert!(ops.contains(&Operand::Reg("x0".into())));
        assert!(ops.contains(&Operand::Imm(1)));
    }

    #[test]
    fn extracts_memory_operand() {
        let ops = extract_operands("str x0, [x1, #8]", &Mnemonic::Mov, 0x1000);
        assert!(ops
            .iter()
            .any(|o| matches!(o, Operand::Mem { base, .. } if base.as_deref() == Some("x1"))));
    }

    #[test]
    fn branch_operand_is_imm_target() {
        let ops = extract_operands("b #0x14", &Mnemonic::Jmp, 0x1000);
        assert_eq!(ops, vec![Operand::Imm(0x1014)]);
    }
}
