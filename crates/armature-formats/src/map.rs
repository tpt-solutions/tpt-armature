//! The standardized output of the ingestion layer.

/// The container format of the original binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryFormat {
    PE,
    ELF,
    MachO,
    Unknown,
}

impl std::fmt::Display for BinaryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinaryFormat::PE => "PE",
            BinaryFormat::ELF => "ELF",
            BinaryFormat::MachO => "Mach-O",
            BinaryFormat::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// A contiguous region of the binary mapped into virtual address space.
#[derive(Debug, Clone)]
pub struct Section {
    /// Section name (`.text`, `.data`, ...). May be empty for unnamed regions.
    pub name: String,
    /// Virtual address of the section's start.
    pub virt_addr: u64,
    /// Size in bytes.
    pub size: u64,
    /// File offset of the raw bytes (for debugging / hex views).
    pub offset: u64,
    /// Raw section bytes.
    pub data: Vec<u8>,
    /// Executable (contains code).
    pub is_executable: bool,
    /// Writable.
    pub is_writable: bool,
    /// Readable.
    pub is_readable: bool,
}

/// An imported symbol (dependency the binary expects at load time).
#[derive(Debug, Clone)]
pub struct Import {
    /// Originating library (DLL / shared object), if known.
    pub library: String,
    /// Symbol name, when imported by name.
    pub name: Option<String>,
    /// Ordinal, when imported by ordinal.
    pub ordinal: Option<u16>,
    /// Resolved (or hinted) virtual address.
    pub addr: u64,
}

/// An exported symbol (entry the binary publishes to others).
#[derive(Debug, Clone)]
pub struct Export {
    /// Exported symbol name.
    pub name: String,
    /// Virtual address of the export.
    pub addr: u64,
}

/// A container-format-independent view of a loaded binary.
///
/// This is the contract between Layer 1 (ingestion) and Layer 2 (analysis).
#[derive(Debug, Clone)]
pub struct MemoryMap {
    /// Detected container format.
    pub format: BinaryFormat,
    /// Detected architecture.
    pub arch: crate::Architecture,
    /// Preferred load base address (image base / `__TEXT` vmaddr).
    pub base_address: u64,
    /// Program entry point (virtual address).
    pub entry_point: u64,
    /// All sections, in container order.
    pub sections: Vec<Section>,
    /// Imported symbols.
    pub imports: Vec<Import>,
    /// Exported symbols.
    pub exports: Vec<Export>,
}

impl MemoryMap {
    /// Return the first executable section, if any.
    pub fn code_section(&self) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| s.is_executable && !s.data.is_empty())
            .or_else(|| self.sections.iter().find(|s| s.is_executable))
    }

    /// Return the first writable data section, if any.
    pub fn data_section(&self) -> Option<&Section> {
        self.sections.iter().find(|s| s.is_writable && !s.data.is_empty())
    }

    /// Total bytes across all sections.
    pub fn total_bytes(&self) -> usize {
        self.sections.iter().map(|s| s.data.len()).sum()
    }
}
