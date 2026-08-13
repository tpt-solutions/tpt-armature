//! Operands: the things instructions read from and write to.

use std::fmt;

/// A single operand of an instruction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Operand {
    /// A named register, e.g. `rax`, `x0`, `esp`.
    Reg(String),
    /// An immediate (constant) value.
    Imm(u64),
    /// A memory reference: `[base + index * scale + disp]`.
    Mem {
        base: Option<String>,
        index: Option<String>,
        scale: u8,
        disp: i64,
    },
}

impl Operand {
    /// Registers this operand reads (for data-flow use/def computation).
    pub fn reads_regs(&self) -> Vec<&str> {
        match self {
            Operand::Reg(r) => vec![r.as_str()],
            Operand::Imm(_) => vec![],
            Operand::Mem { base, index, .. } => {
                let mut v = Vec::new();
                if let Some(b) = base {
                    v.push(b.as_str());
                }
                if let Some(i) = index {
                    v.push(i.as_str());
                }
                v
            }
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Reg(r) => f.write_str(r),
            Operand::Imm(v) => write!(f, "0x{v:x}"),
            Operand::Mem {
                base,
                index,
                scale,
                disp,
            } => {
                f.write_str("[")?;
                let mut parts = Vec::new();
                if let Some(b) = base {
                    parts.push(b.clone());
                }
                if let Some(i) = index {
                    parts.push(format!("{i}*{scale}"));
                }
                let inner = parts.join(" + ");
                if *disp != 0 || inner.is_empty() {
                    if inner.is_empty() {
                        write!(f, "{disp}")?;
                    } else if *disp >= 0 {
                        write!(f, "{inner} + {disp}")?;
                    } else {
                        write!(f, "{inner} - {}", -disp)?;
                    }
                } else {
                    f.write_str(&inner)?;
                }
                f.write_str("]")
            }
        }
    }
}
