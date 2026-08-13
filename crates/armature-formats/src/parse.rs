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

    let mut map = MemoryMap {
        format: BinaryFormat::Unknown,
        arch: Architecture::Unknown,
        base_address: 0,
        entry_point: 0,
        sections: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
    };

    match object {
        goblin::Object::Elf(elf) => parse_elf(elf, bytes, &mut map)?,
        goblin::Object::PE(pe) => parse_pe(pe, bytes, &mut map)?,
        goblin::Object::Mach(mach) => parse_macho(mach, bytes, &mut map)?,
        _ => return Err(FormatError::Unrecognized),
    }

    Ok(map)
}

#[allow(clippy::too_many_arguments)]
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

fn parse_elf(elf: goblin::elf::Elf<'_>, bytes: &[u8], map: &mut MemoryMap) -> Result<()> {
    map.format = BinaryFormat::ELF;
    map.arch = arch::from_elf_machine(elf.header.e_machine);

    for sh in &elf.section_headers {
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("").to_string();
        let range = match sh.file_range() {
            Some(r) if r.end <= bytes.len() => r,
            _ => continue,
        };
        let data = bytes[range.start..range.end].to_vec();
        let exec = sh.sh_flags & goblin::elf::section_header::SHF_EXECINSTR as u64 != 0;
        let writable = sh.sh_flags & goblin::elf::section_header::SHF_WRITE as u64 != 0;
        let readable = sh.sh_flags & goblin::elf::section_header::SHF_ALLOC as u64 != 0;
        push_section(
            &mut map.sections,
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

    const STT_FUNC: u8 = 2;
    const STT_OBJECT: u8 = 1;
    for sym in elf.dynsyms.iter() {
        let name = elf.dynstrtab.get_at(sym.st_name).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        if sym.st_shndx == 0 {
            map.imports.push(Import {
                library: String::new(),
                name: Some(name),
                ordinal: None,
                addr: 0,
            });
        } else if sym.st_type() == STT_FUNC || sym.st_type() == STT_OBJECT {
            map.exports.push(Export {
                name,
                addr: sym.st_value,
            });
        }
    }

    map.entry_point = elf.header.e_entry;
    Ok(())
}

fn parse_pe(pe: goblin::pe::PE<'_>, bytes: &[u8], map: &mut MemoryMap) -> Result<()> {
    map.format = BinaryFormat::PE;
    map.arch = arch::from_pe_machine(pe.header.coff_header.machine);

    map.base_address = pe
        .header
        .optional_header
        .as_ref()
        .map(|oh| oh.windows_fields.image_base)
        .unwrap_or(0);

    const EXEC: u32 = 0x2000_0000;
    const WRITE: u32 = 0x8000_0000;
    const READ: u32 = 0x4000_0000;
    for section in &pe.sections {
        let name = section.name().unwrap_or("").to_string();
        let data = section
            .data(bytes)
            .unwrap_or_default()
            .unwrap_or_default()
            .to_vec();
        let virt = section.virtual_address as u64;
        let vsize = section.virtual_size as u64;
        let char = section.characteristics;
        push_section(
            &mut map.sections,
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

    for import in &pe.imports {
        let addr = if import.offset != 0 {
            file_offset_to_va(&map.sections, import.offset as u64)
        } else {
            0
        };
        map.imports.push(Import {
            library: import.dll.to_string(),
            name: Some(import.name.to_string()),
            ordinal: Some(import.ordinal),
            addr,
        });
    }

    for export in &pe.exports {
        let name = export.name.map(|s| s.to_string()).unwrap_or_default();
        let offset = export.offset.unwrap_or(0) as u64;
        let addr = if offset != 0 {
            file_offset_to_va(&map.sections, offset)
        } else {
            0
        };
        map.exports.push(Export { name, addr });
    }

    map.entry_point = map.base_address + pe.entry as u64;
    Ok(())
}

fn parse_macho(mach: goblin::mach::Mach<'_>, bytes: &[u8], map: &mut MemoryMap) -> Result<()> {
    use goblin::mach::constants::{S_ATTR_PURE_INSTRUCTIONS, S_ATTR_SOME_INSTRUCTIONS};

    let binary = match mach {
        goblin::mach::Mach::Binary(b) => b,
        goblin::mach::Mach::Fat(multi) => {
            let mut found = None;
            for arch in multi.iter_arches().flatten() {
                let slice = arch.slice(bytes);
                if let Ok(macho) = goblin::mach::MachO::parse(slice, 0) {
                    found = Some(macho);
                    break;
                }
            }
            match found {
                Some(b) => b,
                None => return Err(FormatError::Unrecognized),
            }
        }
    };

    map.format = BinaryFormat::MachO;
    map.arch = arch::from_macho_cputype(binary.header.cputype);

    let mut text_vmaddr = 0u64;
    for segment in &binary.segments {
        if segment.segname.as_slice().starts_with(b"__TEXT") {
            text_vmaddr = segment.vmaddr;
        }
        let vmaddr = segment.vmaddr;
        let sections = segment.sections().map_err(|e| FormatError::Parse {
            format: "Mach-O",
            source: e,
        })?;
        for (sect, section_data) in sections {
            let name = sect.name().unwrap_or("").to_string();
            let exec = sect.flags & S_ATTR_PURE_INSTRUCTIONS != 0
                || sect.flags & S_ATTR_SOME_INSTRUCTIONS != 0;
            push_section(
                &mut map.sections,
                name,
                vmaddr + sect.addr,
                sect.size,
                sect.offset as u64,
                section_data.to_vec(),
                exec,
                true,
                true,
            );
        }
    }

    if let Some(symbols) = &binary.symbols {
        for (name, nlist) in symbols.into_iter().filter_map(std::result::Result::ok) {
            if !name.is_empty() {
                map.exports.push(Export {
                    name: name.to_string(),
                    addr: nlist.n_value,
                });
            }
        }
    }

    map.entry_point = text_vmaddr + mach_entry(&binary).unwrap_or(0);
    Ok(())
}

/// Best-effort Mach-O entry point extraction from `LC_MAIN`.
fn mach_entry(binary: &goblin::mach::MachO<'_>) -> Option<u64> {
    use goblin::mach::load_command::CommandVariant;
    for lc in &binary.load_commands {
        if let CommandVariant::Main(main) = &lc.command {
            return Some(main.entryoff);
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
        assert_ne!(
            map.format,
            BinaryFormat::Unknown,
            "container not recognized"
        );
        assert!(
            !map.sections.is_empty(),
            "expected at least one section in the test binary"
        );
        assert!(
            map.code_section().is_some(),
            "expected an executable section"
        );
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
