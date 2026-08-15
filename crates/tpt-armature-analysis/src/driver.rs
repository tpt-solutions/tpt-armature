//! Windows kernel-driver profile recovery (feature `driver-pe`).
//!
//! Builds a [`DriverProfile`] from a fully analyzed PE: extracts `CTL_CODE`
//! IOCTL constants from the instruction stream and recovers the IRP
//! `MajorFunction` dispatch table from the entry function. The dispatch
//! recovery uses a base-pointer-provenance matcher in the spirit of A.1's MMIO
//! matcher: a handler address loaded into a register and then stored into
//! `DriverObject->MajorFunction[index]` is recorded as that IRP's dispatcher.

use std::collections::{BTreeMap, HashMap};

use tpt_armature_formats::driver::{CtlCode, DriverFramework, DriverInfo};
use tpt_armature_ir::{Function, Instruction, Mnemonic, Operand};

use crate::Analysis;

/// IRP `MajorFunction` indices (Windows `IRP_MJ_*`, in declaration order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IrpMajorFunction {
    Create,
    CreateNamedPipe,
    Close,
    Read,
    Write,
    QueryInformation,
    SetInformation,
    QueryEa,
    SetEa,
    FlushBuffers,
    QueryVolumeInformation,
    SetVolumeInformation,
    DirectoryControl,
    FileSystemControl,
    DeviceIoControl,
    InternalDeviceControl,
    Shutdown,
    LockControl,
    Cleanup,
    CreateMailslot,
    QuerySecurity,
    SetSecurity,
    Power,
    SystemControl,
    DeviceChange,
    QueryQuota,
    SetQuota,
    Pnp,
}

impl IrpMajorFunction {
    /// Map a `MajorFunction` array index to the variant.
    pub fn from_index(i: u8) -> Option<IrpMajorFunction> {
        IRP_MJ.get(i as usize).copied()
    }

    /// The `MajorFunction` array index for this variant.
    pub fn index(self) -> u8 {
        IRP_MJ.iter().position(|m| *m == self).unwrap() as u8
    }
}

/// `IRP_MJ_*` in declaration order (index 0..27).
const IRP_MJ: &[IrpMajorFunction] = &[
    IrpMajorFunction::Create,
    IrpMajorFunction::CreateNamedPipe,
    IrpMajorFunction::Close,
    IrpMajorFunction::Read,
    IrpMajorFunction::Write,
    IrpMajorFunction::QueryInformation,
    IrpMajorFunction::SetInformation,
    IrpMajorFunction::QueryEa,
    IrpMajorFunction::SetEa,
    IrpMajorFunction::FlushBuffers,
    IrpMajorFunction::QueryVolumeInformation,
    IrpMajorFunction::SetVolumeInformation,
    IrpMajorFunction::DirectoryControl,
    IrpMajorFunction::FileSystemControl,
    IrpMajorFunction::DeviceIoControl,
    IrpMajorFunction::InternalDeviceControl,
    IrpMajorFunction::Shutdown,
    IrpMajorFunction::LockControl,
    IrpMajorFunction::Cleanup,
    IrpMajorFunction::CreateMailslot,
    IrpMajorFunction::QuerySecurity,
    IrpMajorFunction::SetSecurity,
    IrpMajorFunction::Power,
    IrpMajorFunction::SystemControl,
    IrpMajorFunction::DeviceChange,
    IrpMajorFunction::QueryQuota,
    IrpMajorFunction::SetQuota,
    IrpMajorFunction::Pnp,
];

/// A recovered IOCTL (`CTL_CODE` constant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ioctl {
    /// The packed `CTL_CODE` constant.
    pub code: u32,
    /// The decoded fields.
    pub ctl: CtlCode,
}

/// Recovered driver profile.
#[derive(Debug, Clone, Default)]
pub struct DriverProfile {
    /// Whether the binary was classified as a kernel driver.
    pub is_driver: bool,
    /// Driver framework classification.
    pub framework: DriverFramework,
    /// DriverEntry virtual address.
    pub driver_entry: u64,
    /// IRP `MajorFunction` -> dispatch handler address.
    pub dispatch: BTreeMap<IrpMajorFunction, u64>,
    /// Distinct IOCTLs found in the instruction stream.
    pub ioctls: Vec<Ioctl>,
}

/// Extract distinct `CTL_CODE` IOCTLs from an instruction stream.
pub fn extract_ioctls(instructions: &[Instruction]) -> Vec<Ioctl> {
    let mut seen: BTreeMap<u32, Ioctl> = BTreeMap::new();
    for ins in instructions {
        for op in &ins.operands {
            if let Operand::Imm(v) = op {
                if let Some(ctl) = CtlCode::decode(*v as u32) {
                    seen.insert(
                        *v as u32,
                        Ioctl {
                            code: *v as u32,
                            ctl,
                        },
                    );
                }
            }
        }
    }
    seen.into_values().collect()
}

/// Recover the IRP `MajorFunction` dispatch table from an entry function.
///
/// Heuristic: a handler address is loaded into a register (`mov reg, imm`), then
/// stored into `DriverObject->MajorFunction[index]` (`mov [base + disp], reg`)
/// where `disp` lands on the `MajorFunction` array (offset `MAJOR_OFFSET`,
/// stride `ptr_size`). The loaded immediate is recorded for that index.
pub fn recover_dispatch(entry: &Function, ptr_size: u8) -> BTreeMap<IrpMajorFunction, u64> {
    // DRIVER_OBJECT.MajorFunction field offset (x64 0x70, x86 0x38).
    let base_off: i64 = if ptr_size == 8 { 0x70 } else { 0x38 };
    let upper = base_off + (IRP_MJ.len() as i64) * ptr_size as i64;
    let mut last_imm: HashMap<String, u64> = HashMap::new();
    let mut result: BTreeMap<IrpMajorFunction, u64> = BTreeMap::new();

    for block in &entry.blocks {
        for ins in &block.instructions {
            if matches!(ins.mnemonic, Mnemonic::Mov | Mnemonic::Lea) && ins.operands.len() >= 2 {
                if let (Operand::Reg(dst), Operand::Imm(v)) = (&ins.operands[0], &ins.operands[1]) {
                    last_imm.insert(dst.clone(), *v);
                }
            }
            if matches!(ins.mnemonic, Mnemonic::Mov) && ins.operands.len() >= 2 {
                if let (
                    Operand::Mem {
                        base: Some(_),
                        index: None,
                        scale: _,
                        disp,
                    },
                    Operand::Reg(src),
                ) = (&ins.operands[0], &ins.operands[1])
                {
                    if *disp >= base_off
                        && *disp < upper
                        && (*disp - base_off) % ptr_size as i64 == 0
                    {
                        if let Some(&handler) = last_imm.get(src) {
                            let idx = ((*disp - base_off) / ptr_size as i64) as u8;
                            if let Some(mj) = IrpMajorFunction::from_index(idx) {
                                result.insert(mj, handler);
                            }
                        }
                    }
                }
            }
        }
    }
    result
}

/// Recover a full [`DriverProfile`] from an analyzed binary and its
/// [`DriverInfo`] classification.
pub fn recover_driver_profile(analysis: &Analysis, info: &DriverInfo) -> DriverProfile {
    let ptr_size: u8 = match analysis.map.arch {
        tpt_armature_formats::Architecture::X86 => 4,
        _ => 8,
    };

    let mut dispatch = BTreeMap::new();
    if info.is_kernel_driver {
        if let Some(f) = analysis
            .module
            .functions
            .iter()
            .find(|f| f.start == info.driver_entry)
        {
            dispatch = recover_dispatch(f, ptr_size);
        }
    }

    // IOCTLs are only decoded inside the IRP_MJ_DEVICE_CONTROL /
    // IRP_MJ_INTERNAL_DEVICE_CONTROL dispatch handlers, so scope extraction to
    // those recovered functions to avoid scanning the whole image (which would
    // surface thousands of unrelated constants).
    let handler_addrs: Vec<u64> = [
        IrpMajorFunction::DeviceIoControl,
        IrpMajorFunction::InternalDeviceControl,
    ]
    .iter()
    .filter_map(|mj| dispatch.get(mj).copied())
    .collect();

    let mut ioctls = Vec::new();
    if !handler_addrs.is_empty() {
        let mut instrs: Vec<Instruction> = Vec::new();
        for f in &analysis.module.functions {
            if handler_addrs.contains(&f.start) {
                for b in &f.blocks {
                    instrs.extend(b.instructions.iter().cloned());
                }
            }
        }
        ioctls = extract_ioctls(&instrs);
    }

    DriverProfile {
        is_driver: info.is_kernel_driver,
        framework: info.framework,
        driver_entry: info.driver_entry,
        dispatch,
        ioctls,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_armature_ir::{BasicBlock, Operand};

    fn ins(addr: u64, mnemonic: Mnemonic, operands: Vec<Operand>) -> Instruction {
        Instruction {
            address: addr,
            size: 4,
            mnemonic,
            operands,
            raw: Vec::new(),
            text: String::new(),
        }
    }

    fn entry(start: u64, insts: Vec<Instruction>) -> Function {
        let end = insts
            .last()
            .map(|i| i.address + i.size as u64)
            .unwrap_or(start);
        Function {
            id: 0,
            start,
            name: None,
            blocks: vec![BasicBlock {
                id: 0,
                start,
                end,
                instructions: insts,
            }],
        }
    }

    #[test]
    fn extract_ioctls_finds_ctl_codes() {
        // FILE_DEVICE_UNKNOWN=0x22, fn 0x800, METHOD_BUFFERED, ANY_ACCESS
        let code = ((0x22u32) << 16) | ((1u32) << 14) | ((0x800u32) << 2);
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(code as u64)],
            ),
            // A non-IOCTL immediate that must be ignored.
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![Operand::Reg("rbx".into()), Operand::Imm(0x1234)],
            ),
        ];
        let ioctls = extract_ioctls(&insts);
        assert_eq!(ioctls.len(), 1);
        assert_eq!(ioctls[0].code, code);
        assert_eq!(ioctls[0].ctl.device_type, 0x22);
    }

    #[test]
    fn recover_dispatch_reads_majorfunction_stores() {
        // DriverEntry at 0x1000 (x64): load handlers, store into MajorFunction[].
        let insts = vec![
            ins(
                0x1000,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0x2000)],
            ),
            ins(
                0x1004,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rcx".into()),
                        index: None,
                        scale: 1,
                        disp: 0x70,
                    },
                    Operand::Reg("rax".into()),
                ],
            ),
            ins(
                0x1008,
                Mnemonic::Mov,
                vec![Operand::Reg("rax".into()), Operand::Imm(0x3000)],
            ),
            ins(
                0x100C,
                Mnemonic::Mov,
                vec![
                    Operand::Mem {
                        base: Some("rcx".into()),
                        index: None,
                        scale: 1,
                        disp: 0x70 + 14 * 8,
                    },
                    Operand::Reg("rax".into()),
                ],
            ),
        ];
        let dispatch = recover_dispatch(&entry(0x1000, insts), 8);
        assert_eq!(dispatch.get(&IrpMajorFunction::Create), Some(&0x2000));
        assert_eq!(
            dispatch.get(&IrpMajorFunction::DeviceIoControl),
            Some(&0x3000)
        );
        assert_eq!(dispatch.len(), 2);
    }

    #[test]
    fn irp_major_function_index_round_trip() {
        for (i, mj) in IRP_MJ.iter().enumerate() {
            assert_eq!(IrpMajorFunction::from_index(i as u8), Some(*mj));
            assert_eq!(mj.index(), i as u8);
        }
    }
}
