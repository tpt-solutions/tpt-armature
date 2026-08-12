//! Instructions, basic blocks, and functions in the IR.

use crate::operand::Operand;
use std::fmt;

/// A normalized instruction mnemonic.
///
/// Common control-flow and arithmetic mnemonics get first-class variants so the
/// analysis passes can reason about them cheaply; everything else falls back to
/// [`Mnemonic::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mnemonic {
    Mov,
    Movzx,
    Lea,
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Cmp,
    Test,
    Push,
    Pop,
    Call,
    Jmp,
    /// Conditional branch; the string is the condition (e.g. `z`, `nz`, `c`).
    Jcc(String),
    Ret,
    Nop,
    /// Any other mnemonic, kept verbatim (lower-cased).
    Other(String),
}

impl Mnemonic {
    /// Map a raw assembler mnemonic string to the IR [`Mnemonic`].
    pub fn from_str(s: &str) -> Mnemonic {
        let lower = s.to_ascii_lowercase();
        match lower.as_str() {
            "mov" => Mnemonic::Mov,
            "movzx" => Mnemonic::Movzx,
            "lea" => Mnemonic::Lea,
            "add" => Mnemonic::Add,
            "sub" => Mnemonic::Sub,
            "mul" => Mnemonic::Mul,
            "div" => Mnemonic::Div,
            "and" => Mnemonic::And,
            "or" => Mnemonic::Or,
            "xor" => Mnemonic::Xor,
            "shl" => Mnemonic::Shl,
            "shr" => Mnemonic::Shr,
            "cmp" => Mnemonic::Cmp,
            "test" => Mnemonic::Test,
            "push" => Mnemonic::Push,
            "pop" => Mnemonic::Pop,
            "call" => Mnemonic::Call,
            "jmp" => Mnemonic::Jmp,
            "ret" | "retn" => Mnemonic::Ret,
            "nop" => Mnemonic::Nop,
            _ if lower.starts_with("j") && lower.len() > 1 && lower != "jmp" => {
                Mnemonic::Jcc(lower[1..].to_string())
            }
            _ => Mnemonic::Other(lower),
        }
    }

    /// Whether this instruction terminates a basic block (control flow leaves).
    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Mnemonic::Jmp | Mnemonic::Jcc(_) | Mnemonic::Ret | Mnemonic::Call
        )
    }

    /// Whether this instruction unconditionally transfers control (no fallthrough).
    pub fn is_unconditional_branch(&self) -> bool {
        matches!(self, Mnemonic::Jmp | Mnemonic::Ret)
    }
}

impl fmt::Display for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Mnemonic::Mov => "mov",
            Mnemonic::Movzx => "movzx",
            Mnemonic::Lea => "lea",
            Mnemonic::Add => "add",
            Mnemonic::Sub => "sub",
            Mnemonic::Mul => "mul",
            Mnemonic::Div => "div",
            Mnemonic::And => "and",
            Mnemonic::Or => "or",
            Mnemonic::Xor => "xor",
            Mnemonic::Shl => "shl",
            Mnemonic::Shr => "shr",
            Mnemonic::Cmp => "cmp",
            Mnemonic::Test => "test",
            Mnemonic::Push => "push",
            Mnemonic::Pop => "pop",
            Mnemonic::Call => "call",
            Mnemonic::Jmp => "jmp",
            Mnemonic::Ret => "ret",
            Mnemonic::Nop => "nop",
            Mnemonic::Jcc(c) => return write!(f, "j{c}"),
            Mnemonic::Other(s) => return f.write_str(s),
        };
        f.write_str(s)
    }
}

/// A single decoded instruction in the IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// Virtual address of the instruction.
    pub address: u64,
    /// Encoded length in bytes.
    pub size: u32,
    /// Normalized mnemonic.
    pub mnemonic: Mnemonic,
    /// Operands, in program order.
    pub operands: Vec<Operand>,
    /// Raw machine-code bytes.
    pub raw: Vec<u8>,
    /// Human-readable assembly text (backend formatted).
    pub text: String,
}

impl Instruction {
    /// Registers this instruction defines (written).
    pub fn defs(&self) -> Vec<String> {
        match &self.mnemonic {
            Mnemonic::Mov
            | Mnemonic::Movzx
            | Mnemonic::Lea
            | Mnemonic::Add
            | Mnemonic::Sub
            | Mnemonic::Mul
            | Mnemonic::Div
            | Mnemonic::And
            | Mnemonic::Or
            | Mnemonic::Xor
            | Mnemonic::Shl
            | Mnemonic::Shr => self
                .operands
                .first()
                .and_then(|o| match o {
                    Operand::Reg(r) => Some(r.clone()),
                    _ => None,
                })
                .into_iter()
                .collect(),
            Mnemonic::Pop => self
                .operands
                .first()
                .and_then(|o| match o {
                    Operand::Reg(r) => Some(r.clone()),
                    _ => None,
                })
                .into_iter()
                .collect(),
            Mnemonic::Push | Mnemonic::Cmp | Mnemonic::Test | Mnemonic::Call | Mnemonic::Jmp => {
                vec![]
            }
            _ => vec![],
        }
    }

    /// Registers this instruction uses (reads).
    pub fn uses(&self) -> Vec<String> {
        self.operands
            .iter()
            .flat_map(|o| o.reads_regs().into_iter().map(|s| s.to_string()))
            .collect()
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// A maximal basic block: a straight-line sequence of instructions with a single
/// entry and a single exit.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// Unique block id within its function.
    pub id: usize,
    /// Address of the first instruction.
    pub start: u64,
    /// Address one past the last instruction.
    pub end: u64,
    /// Constituent instructions, in order.
    pub instructions: Vec<Instruction>,
}

impl BasicBlock {
    /// The block's terminator instruction, if present.
    pub fn terminator(&self) -> Option<&Instruction> {
        self.instructions.last()
    }
}

/// A function: a named/identified collection of basic blocks.
#[derive(Debug, Clone)]
pub struct Function {
    /// Unique function id within the module.
    pub id: usize,
    /// Entry address of the function.
    pub start: u64,
    /// Optional symbol name (from exports / scripts).
    pub name: Option<String>,
    /// Basic blocks belonging to the function.
    pub blocks: Vec<BasicBlock>,
}

impl Function {
    /// Total number of instructions across all blocks.
    pub fn instruction_count(&self) -> usize {
        self.blocks.iter().map(|b| b.instructions.len()).sum()
    }
}
