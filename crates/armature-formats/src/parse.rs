//! Binary container parsing -> [`MemoryMap`].

use crate::arch::{self, Architecture};
use crate::error::{FormatError, Result};
use crate::map::{BinaryFormat, Export, Import, MemoryMap, Section};

/// Parse a raw binary blob into a standardized [`MemoryMap`].
///
/// The function auto-detects the container format (PE / ELF / Mach-O) and maps
/// the architecture, sections, entry point, and imported/exported symbols.
pub fn parse(bytes: &[u8]) -> Result<MemoryMap> {
    let object = goblin::Object::parse(bytes).map_err(|_| FormatError::Unrecognized)?;

    match object {
        goblin::Object::Elf(elf) => parse_elf(elf, bytes),
        goblin::Object::PE(pe) => parse_pe(pe, bytes),
        goblin::Object::Mach(mach) => parse_macho(mach, bytes),
        _ => Err(FormatError::Unrecognized),
    }
}

fn push_section(
    sections: &mut Vec<Section>,
    name: String,
    virt_addr: u64,
    size: u64,
    offset: u64,
    data: Vec<u8>,
    exec: bool,
    writable: bool,
    readable: bool,
) {
    if name.is_empty() && data.is_empty() {
        return;
    }
    sections.push(Section {
        name,
        virt_addr,
        size,
        offset,
        data,
        is_executable: exec,
        is_writable: writable,
        is_readable: readable,
    });
}

/// Translate a file offset into a virtual address using section layout.
fn file_offset_to_va(sections: &[Section], offset: u64) -> u64 {
    for s in sections {
        let len = s.data.len() as u64;
        if len > 0 && offset >= s.offset && offset < s.offset + len {
            return s.virt_addr + (offset - s.offset);
        }
    }
    offset
}

fn parse_elf(elf: goblin::elf::Elf<'_>, bytes: &[u8]) -> Result<MemoryMap> {
    let arch = arch::from_elf_machine(elf.header.e_machine);
    let mut sections = Vec::new();

    for sh in &elf.section_headers {
        let name = elf
            .shdr_strtab
            .get(sh.sh_name as usize)
            .unwrap_or("")
            .to_string();
        let data = match sh.file_range() {
            Some((start, end)) if end <= bytes.len() => bytes[start..end].to_vec(),
            _ => continue,
        };
        let exec = sh.sh_flags & goblin::elf::section_header::SHF_EXECINSTR as u64 != 0;
        let writable = sh.sh_flags & goblin::elf::section_header::SHF_WRITE as u64 != 0;
        let readable = sh.sh_flags & goblin::elf::section_header::SHF_ALLOC as u64 != 0;
        push_section(
            &mut sections,
            name,
            sh.sh_addr,
            sh.sh_size,
            sh.sh_offset,
            data,
            exec,
            writable,
            readable,
        );
    }

    let mut imports = Vec::new();
    let mut exports = Vec::new();
    const STT_FUNC: u8 = 2;
    const STT_OBJECT: u8 = 1;
    for sym in elf.dynsyms.iter() {
        let name = elf.dynstrtab.get(sym.st_name).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        if sym.st_shndx == goblin::elf::sym::STN_UNDEF {
            imports.push(Import {
                library: String::new(),
                name: Some(name),
                ordinal: None,
                addr: 0,
            });
        } else if sym.st_type() == STT_FUNC || sym.st_type() == STT_OBJECT {
            exports.push(Export { name, addr: sym.st_value });
        }
    }

    Ok(MemoryMap {
        format: BinaryFormat::ELF,
        arch,
        base_address: 0,
        entry_point: elf.header.e_entry,
        sections,
        imports,
        exports,
    })
}

fn parse_pe(pe: goblin::pe::PE<'_>, bytes: &[u8]) -> Result<MemoryMap> {
    let arch = arch::from_pe_machine(pe.header.coff_header.machine);

    let base = match &pe.header.optional_header {
        Some(goblin::pe::optional_header::OptionalHeader::PE32(oh)) => {
            oh.windows_fields.image_base as u64
        }
        Some(goblin::pe::optional_header::OptionalHeader::PE32Plus(oh)) => {
            oh.windows_fields.image_base
        }
        None => 0,
    };

    let mut sections = Vec::new();
    const EXEC: u32 = 0x2000_0000;
    const WRITE: u32 = 0x8000_0000;
    const READ: u32 = 0x4000_0000;
    for section in &pe.sections {
        let name = section.name().to_string();
        let data = section.raw_data(bytes).to_vec();
        let virt = section.virtual_address as u64;
        let vsize = section.virtual_size as u64;
        let char = section.characteristics;
        push_section(
            &mut sections,
            name,
            virt,
            if vsize > 0 { vsize } else { data.len() as u64 },
            section.pointer_to_raw_data as u64,
            data,
            char & EXEC != 0,
            char & WRITE != 0,
            char & READ != 0,
        );
    }

    let mut imports = Vec::new();
    for import in &pe.imports {
        let addr = if import.offset != 0 {
            file_offset_to_va(&sections, import.offset as u64)
        } else {
            0
        };
        imports.push(Import {
            library: import.dll.clone(),
            name: import.name.clone(),
            ordinal: import.ordinal,
            addr,
        });
    }

    let mut exports = Vec::new();
    for export in &pe.exports {
        let name = export.name.clone().unwrap_or_default();
        let addr = if export.offset != 0 {
            file_offset_to_va(&sections, export.offset as u64)
        } else {
            0
        };
        exports.push(Export { name, addr });
    }

    Ok(MemoryMap {
        format: BinaryFormat::PE,
        arch,
        base_address: base,
        entry_point: base + pe.entry as u64,
        sections,
        imports,
        exports,
    })
}

fn parse_macho(mach: goblin::mach::Mach<'_>, bytes: &[u8]) -> Result<MemoryMap> {
    let binary = match mach {
        goblin::mach::Mach::Binary(b) => b,
        goblin::mach::Mach::Fat(fat) => {
            let arches = fat.iter_arches(bytes).map_err(|e| FormatError::Parse {
                format: "Mach-O (fat)",
                source: e,
            })?;
            let mut first = None;
            for arch in arches {
                if let Ok(arch) = arch {
                    if let Ok(b) = fat.get(bytes, &arch) {
                        first = Some(b);
                        break;
                    }
                }
            }
            match first {
                Some(b) => b,
                None => return Err(FormatError::Unrecognized),
            }
        }
    };

    let arch = arch::from_macho_cputype(binary.header.cputype);
    let mut sections = Vec::new();

    for seg in &binary.segments {
        let (vmaddr, sections_in_seg) = match &seg {
            goblin::mach::segment::SegmentCommand::Segment32(s) => (s.vmaddr as u64, &s.sections),
            goblin::mach::segment::SegmentCommand::Segment64(s) => (s.vmaddr as u64, &s.sections),
        };
        for sect in sections_in_seg {
            let name = sect.sectname.to_string();
            let data = match sect.data(&seg, bytes) {
                Ok(d) => d.to_vec(),
                Err(_) => continue,
            };
            let exec = sect.flags & goblin::mach::section::S_ATTR_PURE_INSTRUCTIONS != 0
                || sect.flags & goblin::mach::section::S_ATTR_SOME_INSTRUCTIONS != 0;
            push_section(
                &mut sections,
                name,
                vmaddr + sect.addr as u64,
                sect.size as u64,
                0,
                data,
                exec,
                true,
                true,
            );
        }
    }

    let mut exports = Vec::new();
    for (name, _nlist, _section) in binary.symbols() {
        if let Ok(name) = name {
            if !name.is_empty() {
                exports.push(Export {
                    name: name.to_string(),
                    addr: 0,
                });
            }
        }
    }

    let entry = mach_entry(&binary).unwrap_or(0);

    Ok(MemoryMap {
        format: BinaryFormat::MachO,
        arch,
        base_address: 0,
        entry_point: entry,
        sections,
        imports: Vec::new(),
        exports,
    })
}

/// Best-effort Mach-O entry point extraction from `LC_MAIN` / `LC_UNIXTHREAD`.
fn mach_entry(binary: &goblin::mach::MachO<'_>) -> Option<u64> {
    use goblin::mach::load_command::CommandVariant;
    for lc in &binary.loads {
        if let CommandVariant::Main(main) = &lc.command {
            return Some(main.entryoff as u64);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_self_is_recognized() {
        let exe = std::env::current_exe().expect("current exe");
        let bytes = std::fs::read(&exe).expect("read exe");
        let map = parse(&bytes).expect("parse self");
        assert_ne!(map.format, BinaryFormat::Unknown, "container not recognized");
        assert!(
            !map.sections.is_empty(),
            "expected at least one section in the test binary"
        );
        assert!(map.code_section().is_some(), "expected an executable section");
        assert!(
            map.arch != Architecture::Unknown || map.format == BinaryFormat::MachO,
            "architecture should be detected for PE/ELF"
        );
    }

    #[test]
    fn unrecognized_bytes_fail() {
        let bytes = vec![0u8; 16];
        assert!(parse(&bytes).is_err());
    }
}
