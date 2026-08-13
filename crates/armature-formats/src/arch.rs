//! Target architecture detection.

/// The instruction-set architecture of a parsed binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    X86,
    X86_64,
    Arm,
    Aarch64,
    Unknown,
}

impl Architecture {
    /// Number of bits per virtual address, if known.
    pub fn bits(self) -> Option<u32> {
        match self {
            Architecture::X86 => Some(32),
            Architecture::X86_64 => Some(64),
            Architecture::Arm => Some(32),
            Architecture::Aarch64 => Some(64),
            Architecture::Unknown => None,
        }
    }

    /// Whether this architecture can be disassembled by the available backends.
    ///
    /// x86/x64 are always supported (iced). ARM/AArch64 are supported when the
    /// `arm` feature is enabled on the disassembly layer (yaxpeax); the feature
    /// propagates here so callers can query capability without depending on the
    /// disassembly crate directly.
    pub fn is_disassemblable(self) -> bool {
        match self {
            Architecture::X86 | Architecture::X86_64 => true,
            #[cfg(feature = "arm")]
            Architecture::Arm | Architecture::Aarch64 => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Architecture::X86 => "x86",
            Architecture::X86_64 => "x86_64",
            Architecture::Arm => "arm",
            Architecture::Aarch64 => "aarch64",
            Architecture::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

// ELF `e_machine` values.
const EM_386: u16 = 3;
const EM_X86_64: u16 = 62;
const EM_ARM: u16 = 40;
const EM_AARCH64: u16 = 183;

/// Map an ELF `e_machine` field to an [`Architecture`].
pub fn from_elf_machine(machine: u16) -> Architecture {
    match machine {
        EM_386 => Architecture::X86,
        EM_X86_64 => Architecture::X86_64,
        EM_ARM => Architecture::Arm,
        EM_AARCH64 => Architecture::Aarch64,
        _ => Architecture::Unknown,
    }
}

// PE `Machine` COFF header values.
const IMAGE_FILE_MACHINE_I386: u16 = 0x14c;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_FILE_MACHINE_ARM: u16 = 0x1c0;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xaa64;

/// Map a PE COFF `Machine` field to an [`Architecture`].
pub fn from_pe_machine(machine: u16) -> Architecture {
    match machine {
        IMAGE_FILE_MACHINE_I386 => Architecture::X86,
        IMAGE_FILE_MACHINE_AMD64 => Architecture::X86_64,
        IMAGE_FILE_MACHINE_ARM => Architecture::Arm,
        IMAGE_FILE_MACHINE_ARM64 => Architecture::Aarch64,
        _ => Architecture::Unknown,
    }
}

// Mach-O `cputype` values (high bit marks the 64-bit variant).
const CPU_TYPE_X86: u32 = 7;
const CPU_TYPE_X86_64: u32 = 0x01000007;
const CPU_TYPE_ARM: u32 = 12;
const CPU_TYPE_ARM64: u32 = 0x0100000c;

/// Map a Mach-O `cputype` field to an [`Architecture`].
pub fn from_macho_cputype(cputype: u32) -> Architecture {
    match cputype {
        CPU_TYPE_X86 => Architecture::X86,
        CPU_TYPE_X86_64 => Architecture::X86_64,
        CPU_TYPE_ARM => Architecture::Arm,
        CPU_TYPE_ARM64 => Architecture::Aarch64,
        _ => Architecture::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elf_machine_mapping() {
        assert_eq!(from_elf_machine(EM_386), Architecture::X86);
        assert_eq!(from_elf_machine(EM_X86_64), Architecture::X86_64);
        assert_eq!(from_elf_machine(EM_ARM), Architecture::Arm);
        assert_eq!(from_elf_machine(EM_AARCH64), Architecture::Aarch64);
        assert_eq!(from_elf_machine(0x9999), Architecture::Unknown);
    }

    #[test]
    fn pe_machine_mapping() {
        assert_eq!(from_pe_machine(IMAGE_FILE_MACHINE_I386), Architecture::X86);
        assert_eq!(
            from_pe_machine(IMAGE_FILE_MACHINE_AMD64),
            Architecture::X86_64
        );
        assert_eq!(
            from_pe_machine(IMAGE_FILE_MACHINE_ARM64),
            Architecture::Aarch64
        );
    }

    #[test]
    fn macho_cputype_mapping() {
        assert_eq!(from_macho_cputype(CPU_TYPE_X86_64), Architecture::X86_64);
        assert_eq!(from_macho_cputype(CPU_TYPE_ARM64), Architecture::Aarch64);
    }

    #[test]
    fn bits_and_disassemblable() {
        assert_eq!(Architecture::X86_64.bits(), Some(64));
        assert!(Architecture::X86.is_disassemblable());
        #[cfg(not(feature = "arm"))]
        assert!(!Architecture::Arm.is_disassemblable());
        #[cfg(feature = "arm")]
        assert!(Architecture::Arm.is_disassemblable());
    }
}
