//! String and constant extraction pass.
//!
//! Scans every section for printable ASCII / UTF-16 runs (strings) and harvests
//! the immediate constants referenced by the decoded instructions. Both are
//! surfaced in the CLI (`armature strings`) and the GUI strings panel, and are
//! handy triage leads (format strings, URLs, magic numbers, jump-table offsets).

use armature_formats::MemoryMap;

/// The encoding of an extracted string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringKind {
    /// 8-bit printable ASCII / UTF-8.
    Ascii,
    /// 16-bit (little-endian) UTF-16 run.
    Utf16,
}

/// A string discovered in the binary image.
#[derive(Debug, Clone)]
pub struct ExtractedString {
    /// Virtual address of the string's start.
    pub addr: u64,
    /// Encoding.
    pub kind: StringKind,
    /// Raw bytes of the run.
    pub bytes: Vec<u8>,
    /// Lossy UTF-8 rendering of the run (what analysts read).
    pub text: String,
}

impl ExtractedString {
    /// Whether the run is printable enough to be a useful lead.
    fn is_printable(byte: u8) -> bool {
        (0x20..0x7f).contains(&byte)
    }
}

/// Extract printable strings (ASCII and little-endian UTF-16) from a memory map.
///
/// `min_len` is the minimum run length (in characters) to keep.
pub fn extract_strings(map: &MemoryMap, min_len: usize) -> Vec<ExtractedString> {
    let mut out = Vec::new();
    for section in &map.sections {
        let base = map.base_address + section.virt_addr;
        out.extend(extract_ascii(&section.data, base, min_len));
        out.extend(extract_utf16(&section.data, base, min_len));
    }
    out.sort_by_key(|s| s.addr);
    out
}

fn extract_ascii(data: &[u8], base: u64, min_len: usize) -> Vec<ExtractedString> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = data.len();
    while i < n {
        if ExtractedString::is_printable(data[i]) {
            let mut j = i + 1;
            while j < n && ExtractedString::is_printable(data[j]) {
                j += 1;
            }
            if j - i >= min_len {
                let bytes = data[i..j].to_vec();
                let text = String::from_utf8_lossy(&bytes).into_owned();
                out.push(ExtractedString {
                    addr: base + i as u64,
                    kind: StringKind::Ascii,
                    bytes,
                    text,
                });
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn extract_utf16(data: &[u8], base: u64, min_len: usize) -> Vec<ExtractedString> {
    let mut out = Vec::new();
    let n = data.len();
    let mut i = 0;
    while i + 1 < n {
        // Little-endian UTF-16 code unit: low byte printable, high byte zero.
        let lo = data[i];
        let hi = data[i + 1];
        if ExtractedString::is_printable(lo) && hi == 0 {
            let mut j = i + 2;
            while j + 1 < n && ExtractedString::is_printable(data[j]) && data[j + 1] == 0 {
                j += 2;
            }
            let units = (j - i) / 2;
            if units >= min_len {
                let mut wide: Vec<u16> = Vec::with_capacity(units);
                let mut k = i;
                while k + 1 < j {
                    wide.push(u16::from_le_bytes([data[k], data[k + 1]]));
                    k += 2;
                }
                let text = String::from_utf16_lossy(&wide);
                out.push(ExtractedString {
                    addr: base + i as u64,
                    kind: StringKind::Utf16,
                    bytes: data[i..j].to_vec(),
                    text,
                });
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Collect the distinct immediate constants referenced by the decoded
/// instructions, sorted ascending. Useful for spotting magic numbers, bitmasks,
/// and jump-table offsets. `max` caps the returned count for UI sanity.
pub fn extract_constants(instructions: &[armature_ir::Instruction], max: usize) -> Vec<u64> {
    use std::collections::BTreeSet;
    let mut set = BTreeSet::new();
    for ins in instructions {
        for op in &ins.operands {
            if let armature_ir::Operand::Imm(v) = op {
                // Skip tiny values (likely registers-as-index noise) and keep the
                // interesting magnitudes.
                if *v >= 2 {
                    set.insert(*v);
                }
            }
        }
    }
    set.into_iter().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use armature_formats::{MemoryMap, Section};

    fn map_with(bytes: &[u8]) -> MemoryMap {
        MemoryMap {
            format: armature_formats::BinaryFormat::Unknown,
            arch: armature_formats::Architecture::X86_64,
            base_address: 0x1000,
            entry_point: 0,
            sections: vec![Section {
                name: ".rdata".into(),
                virt_addr: 0,
                size: bytes.len() as u64,
                offset: 0,
                data: bytes.to_vec(),
                is_executable: false,
                is_writable: false,
                is_readable: true,
            }],
            imports: vec![],
            exports: vec![],
            debug_symbols: vec![],
        }
    }

    #[test]
    fn extracts_ascii_run() {
        let data = b"\x00\x00Hello, world!\x00\x01\x02";
        let map = map_with(data);
        let strings = extract_strings(&map, 4);
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].text, "Hello, world!");
        assert_eq!(strings[0].addr, 0x1000 + 2);
        assert_eq!(strings[0].kind, StringKind::Ascii);
    }

    #[test]
    fn extracts_utf16_run() {
        let mut data = vec![0u8; 0];
        // "Hi" little-endian UTF-16: H=0x48, i=0x69
        data.extend_from_slice(&[0x48, 0x00, 0x69, 0x00, 0x00, 0x00]);
        let map = map_with(&data);
        let strings = extract_strings(&map, 2);
        assert!(strings.iter().any(|s| s.text == "Hi" && s.kind == StringKind::Utf16));
    }

    #[test]
    fn extracts_constants() {
        use armature_ir::{Instruction, Mnemonic, Operand};
        let insns = vec![
            Instruction {
                address: 0,
                size: 5,
                mnemonic: Mnemonic::Mov,
                operands: vec![Operand::Reg("rax".into()), Operand::Imm(0xdeadbeef)],
                raw: vec![0; 5],
                text: "mov rax, 0xdeadbeef".into(),
            },
            Instruction {
                address: 5,
                size: 5,
                mnemonic: Mnemonic::Add,
                operands: vec![Operand::Reg("rbx".into()), Operand::Imm(1)],
                raw: vec![0; 5],
                text: "add rbx, 1".into(),
            },
        ];
        let constants = extract_constants(&insns, 100);
        assert!(constants.contains(&0xdeadbeef));
        assert!(!constants.contains(&1));
    }
}
