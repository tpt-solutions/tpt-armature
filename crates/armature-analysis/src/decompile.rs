//! Lightweight pseudocode / decompiler view.
//!
//! Translates the recovered IR ([`armature_ir::Function`]) into a rough
//! C-like listing: register assignments, calls, branches, and returns. This is
//! a linear (block-by-block) rendering rather than full control-flow
//! structuring, but it gives analysts a readable, IR-derived overview of a
//! function without a full decompiler.

use armature_ir::{Function, Instruction, Mnemonic, Module, Operand};
use std::collections::HashMap;

/// Render every recovered function in the module as pseudocode.
pub fn decompile_module(module: &Module) -> String {
    let names = function_names(module);
    let mut out = String::new();
    for func in &module.functions {
        out.push_str(&decompile_function(func, &names));
        out.push('\n');
    }
    out
}

/// Render a single function as pseudocode.
pub fn decompile_function(func: &Function, call_names: &HashMap<u64, String>) -> String {
    let mut out = String::new();
    let label = func
        .name
        .clone()
        .unwrap_or_else(|| format!("fn_{:x}", func.start));
    out.push_str(&format!("// {} @ 0x{:x}\n", label, func.start));
    out.push_str(&format!("{}() {{\n", sanitize(&label)));

    for block in &func.blocks {
        if func.blocks.len() > 1 {
            out.push_str(&format!("  {}:\n", block_label(block.start)));
        }
        for ins in &block.instructions {
            let line = decompile_instruction(ins, call_names);
            if !line.is_empty() {
                out.push_str(&format!("  {line}\n"));
            }
        }
    }

    out.push_str("}\n");
    out
}

/// Translate one instruction into a pseudocode statement (empty string = skip).
fn decompile_instruction(ins: &Instruction, call_names: &HashMap<u64, String>) -> String {
    let ops = &ins.operands;
    let a = ops.first();
    let b = ops.get(1);

    match &ins.mnemonic {
        Mnemonic::Nop => String::new(),
        Mnemonic::Mov => assign(ins, "="),
        Mnemonic::Movzx => assign(ins, "="),
        Mnemonic::Lea => {
            // lea reg, [mem]  ~  reg = &mem
            let d = a.map(operand_str).unwrap_or_default();
            let s = b.map(operand_str).unwrap_or_default();
            format!("{d} = &{s};")
        }
        Mnemonic::Add => binop(a, b, "+"),
        Mnemonic::Sub => binop(a, b, "-"),
        Mnemonic::Mul => binop(a, b, "*"),
        Mnemonic::Div => binop(a, b, "/"),
        Mnemonic::And => binop(a, b, "&"),
        Mnemonic::Or => binop(a, b, "|"),
        Mnemonic::Xor => binop(a, b, "^"),
        Mnemonic::Shl => binop(a, b, "<<"),
        Mnemonic::Shr => binop(a, b, ">>"),
        Mnemonic::Cmp => comment(&format!("cmp {}, {}", a_str(ops), b_str(ops))),
        Mnemonic::Test => comment(&format!("test {}, {}", a_str(ops), b_str(ops))),
        Mnemonic::Push => comment(&format!("push {}", a_str(ops))),
        Mnemonic::Pop => comment(&format!("pop {}", a_str(ops))),
        Mnemonic::Call => {
            let target = ops.iter().find_map(|o| match o {
                Operand::Imm(v) => Some(*v),
                _ => None,
            });
            match target {
                Some(t) => {
                    let name = call_names
                        .get(&t)
                        .cloned()
                        .unwrap_or_else(|| format!("fn_{t:x}"));
                    format!("{name}();")
                }
                None => comment("call (indirect)"),
            }
        }
        Mnemonic::Jmp => {
            let target = branch_target(ops);
            match target {
                Some(t) => format!("goto {};", block_label(t)),
                None => comment("jmp (indirect)"),
            }
        }
        Mnemonic::Jcc(cond) => {
            let target = branch_target(ops);
            match target {
                Some(t) => format!("if ({cond}) goto {};", block_label(t)),
                None => comment(&format!("j{cond} (indirect)")),
            }
        }
        Mnemonic::Ret => "return;".to_string(),
        Mnemonic::Other(text) => comment(text),
    }
}

fn assign(ins: &Instruction, op: &str) -> String {
    let ops = &ins.operands;
    match (ops.first(), ops.get(1)) {
        (Some(d), Some(s)) => format!("{} {} {};", operand_str(d), op, operand_str(s)),
        _ => comment(&ins.text),
    }
}

fn binop(a: Option<&Operand>, b: Option<&Operand>, op: &str) -> String {
    match (a, b) {
        (Some(d), Some(s)) => format!(
            "{} = {} {} {};",
            operand_str(d),
            operand_str(d),
            op,
            operand_str(s)
        ),
        _ => String::new(),
    }
}

fn comment(text: &str) -> String {
    format!("// {text}")
}

fn branch_target(ops: &[Operand]) -> Option<u64> {
    ops.iter().find_map(|o| match o {
        Operand::Imm(v) => Some(*v),
        _ => None,
    })
}

fn a_str(ops: &[Operand]) -> String {
    ops.first().map(operand_str).unwrap_or_default()
}

fn b_str(ops: &[Operand]) -> String {
    ops.get(1).map(operand_str).unwrap_or_default()
}

fn operand_str(op: &Operand) -> String {
    op.to_string()
}

fn block_label(addr: u64) -> String {
    format!("L_{addr:x}")
}

/// Build addr -> function-label map for resolving call targets to names.
fn function_names(module: &Module) -> HashMap<u64, String> {
    module
        .functions
        .iter()
        .map(|f| {
            let name = f
                .name
                .clone()
                .unwrap_or_else(|| format!("fn_{:x}", f.start));
            (f.start, sanitize(&name))
        })
        .collect()
}

/// Make a C-friendly identifier from an arbitrary label.
fn sanitize(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("sym");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use armature_ir::{BasicBlock, Instruction, Operand};

    fn ins(addr: u64, m: Mnemonic, ops: Vec<Operand>) -> Instruction {
        Instruction {
            address: addr,
            size: 4,
            mnemonic: m.clone(),
            operands: ops,
            raw: vec![0; 4],
            text: format!("{m}"),
        }
    }

    #[test]
    fn decompiles_assignments_and_calls() {
        let mut module = Module::default();
        module.functions.push(armature_ir::Function {
            id: 0,
            start: 0x1000,
            name: Some("compute".into()),
            blocks: vec![BasicBlock {
                id: 0,
                start: 0x1000,
                end: 0x1010,
                instructions: vec![
                    ins(
                        0x1000,
                        Mnemonic::Mov,
                        vec![Operand::Reg("rax".into()), Operand::Imm(1)],
                    ),
                    ins(
                        0x1004,
                        Mnemonic::Add,
                        vec![Operand::Reg("rax".into()), Operand::Imm(2)],
                    ),
                    ins(0x1008, Mnemonic::Call, vec![Operand::Imm(0x2000)]),
                    ins(0x100c, Mnemonic::Ret, vec![]),
                ],
            }],
        });
        let out = decompile_module(&module);
        assert!(out.contains("compute() {"));
        assert!(out.contains("rax = 1;"));
        assert!(out.contains("rax = rax + 2;"));
        assert!(out.contains("fn_2000();"));
        assert!(out.contains("return;"));
    }

    #[test]
    fn decompiles_branch_to_label() {
        let mut module = Module::default();
        module.functions.push(armature_ir::Function {
            id: 0,
            start: 0x1000,
            name: None,
            blocks: vec![
                BasicBlock {
                    id: 0,
                    start: 0x1000,
                    end: 0x1004,
                    instructions: vec![ins(
                        0x1000,
                        Mnemonic::Jcc("z".into()),
                        vec![Operand::Imm(0x2000)],
                    )],
                },
                BasicBlock {
                    id: 1,
                    start: 0x2000,
                    end: 0x2004,
                    instructions: vec![ins(0x2000, Mnemonic::Ret, vec![])],
                },
            ],
        });
        let out = decompile_module(&module);
        assert!(out.contains("L_1000:"));
        assert!(out.contains("if (z) goto L_2000;"));
    }
}
