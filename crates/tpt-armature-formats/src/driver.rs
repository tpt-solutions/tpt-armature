//! Windows kernel-driver (`.sys`) detection and IOCTL decoding.
//!
//! Feature `driver-pe`. Provides:
//! - [`DriverFramework`] / [`DriverInfo`] classification of a PE (WDM vs
//!   KMDF/WDF; native subsystem ⇒ kernel driver).
//! - [`CtlCode`] decode/encode for Windows `CTL_CODE` IOCTL constants.

use goblin::pe::PE;

/// PE subsystem value for native/kernel drivers (`IMAGE_SUBSYSTEM_NATIVE`).
const SUBSYSTEM_NATIVE: u16 = 1;

/// Driver model recovered from the import set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DriverFramework {
    /// Classic WDM driver (ntoskrnl `IoCreateDevice` / `IoRegisterDriver`).
    Wdm,
    /// KMDF/WDF driver (imports `WdfDriverCreate` / `WDFLDR`).
    Kmdf,
    /// Detected as a kernel driver but the model could not be narrowed.
    #[default]
    Unknown,
}

/// Classification of a PE as a Windows kernel driver.
#[derive(Debug, Clone)]
pub struct DriverInfo {
    /// True when the PE's subsystem is NATIVE (kernel driver).
    pub is_kernel_driver: bool,
    /// Best-effort driver framework classification.
    pub framework: DriverFramework,
    /// DriverEntry virtual address (image base + entry RVA).
    pub driver_entry: u64,
    /// Import DLL names (lowercased), for inspection.
    pub import_libs: Vec<String>,
}

impl DriverInfo {
    /// Classify `bytes` as a Windows kernel driver, if it parses as a PE at all.
    pub fn detect(bytes: &[u8]) -> Option<DriverInfo> {
        let pe = PE::parse(bytes).ok()?;

        let subsystem = pe
            .header
            .optional_header
            .as_ref()
            .map(|oh| oh.windows_fields.subsystem)
            .unwrap_or(0);
        let is_kernel_driver = subsystem == SUBSYSTEM_NATIVE;

        let import_libs: Vec<String> = pe
            .imports
            .iter()
            .map(|i| i.dll.to_ascii_lowercase())
            .collect();
        let import_names: Vec<String> = pe
            .imports
            .iter()
            .map(|i| i.name.to_string().to_ascii_lowercase())
            .collect();

        let framework = if import_names.iter().any(|n| n == "wdfdrivercreate")
            || import_libs.iter().any(|l| l.contains("wdf"))
        {
            DriverFramework::Kmdf
        } else if import_libs.iter().any(|l| l.contains("ntoskrnl"))
            && import_names.iter().any(|n| {
                n.starts_with("iocreate") || n == "ioregisterdriver" || n == "iocreatedriver"
            })
        {
            DriverFramework::Wdm
        } else {
            DriverFramework::Unknown
        };

        let image_base = pe
            .header
            .optional_header
            .as_ref()
            .map(|oh| oh.windows_fields.image_base)
            .unwrap_or(0);
        let driver_entry = image_base + pe.entry as u64;

        Some(DriverInfo {
            is_kernel_driver,
            framework,
            driver_entry,
            import_libs,
        })
    }
}

/// Recognized system `FILE_DEVICE_*` values. Custom/third-party device types
/// use `>= 0x8000`, which is also accepted.
const KNOWN_DEVICE_TYPES: &[u16] = &[
    0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x000A, 0x000B, 0x000C,
    0x000D, 0x000E, 0x000F, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016, 0x0017, 0x0018,
    0x0019, 0x001A, 0x001B, 0x001C, 0x001D, 0x001E, 0x001F, 0x0020, 0x0021, 0x0022, 0x0023, 0x0024,
    0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002A, 0x002B, 0x002C, 0x002D, 0x002E, 0x002F, 0x0030,
    0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037,
];

/// Whether `device_type` is a plausible `FILE_DEVICE_*` value (recognized
/// system value or custom `>= 0x8000`).
fn known_device_type(device_type: u16) -> bool {
    KNOWN_DEVICE_TYPES.contains(&device_type) || device_type >= 0x8000
}

/// A decoded Windows `CTL_CODE` IOCTL value.
///
/// `CTL_CODE(DeviceType, Function, Method, Access)` packs as:
/// `((DeviceType) << 16) | ((Access) << 14) | ((Function) << 2) | (Method)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CtlCode {
    /// `DeviceType` field (high 16 bits).
    pub device_type: u16,
    /// `Function` code (bits 2..14).
    pub function: u16,
    /// `Method` (low 2 bits): 0 = buffered, 1 = in-direct, 2 = out-direct,
    /// 3 = neither.
    pub method: u8,
    /// `Access` (bits 14..16): 0, 1 = any, 2 = read, 3 = write.
    pub access: u8,
}

impl CtlCode {
    /// Decode a `CTL_CODE` constant. Returns `None` when the value cannot be a
    /// plausible IOCTL: zero or unknown device type, or out-of-range
    /// method/access. "Unknown device type" means it is neither a recognized
    /// system `FILE_DEVICE_*` value nor a custom (`>= 0x8000`) one — this filter
    /// rejects the bulk of coincidental constants that happen to decode.
    pub fn decode(code: u32) -> Option<CtlCode> {
        let device_type = ((code >> 16) & 0xFFFF) as u16;
        let access = ((code >> 14) & 0x3) as u8;
        let function = ((code >> 2) & 0xFFF) as u16;
        let method = (code & 0x3) as u8;
        if method > 3 || access > 3 || !known_device_type(device_type) {
            return None;
        }
        Some(CtlCode {
            device_type,
            function,
            method,
            access,
        })
    }

    /// Re-encode to the packed `CTL_CODE` constant.
    pub fn encode(&self) -> u32 {
        ((self.device_type as u32) << 16)
            | ((self.access as u32) << 14)
            | ((self.function as u32) << 2)
            | (self.method as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctl_code_round_trip() {
        // FILE_DEVICE_UNKNOWN=0x22, function 0x800, METHOD_BUFFERED, FILE_ANY_ACCESS
        let code = ((0x22u32) << 16) | ((1u32) << 14) | ((0x800u32) << 2) | 0;
        let decoded = CtlCode::decode(code).expect("valid ctl code");
        assert_eq!(decoded.device_type, 0x22);
        assert_eq!(decoded.function, 0x800);
        assert_eq!(decoded.method, 0);
        assert_eq!(decoded.access, 1);
        assert_eq!(decoded.encode(), code);
    }

    #[test]
    fn ctl_code_rejects_zero_device() {
        assert!(CtlCode::decode(0).is_none());
    }

    #[test]
    fn detect_driver_on_user_exe_is_none() {
        // The host test binary is a user-mode EXE (not the NATIVE subsystem).
        let bytes = std::fs::read(std::env::current_exe().expect("exe")).expect("read");
        let info = DriverInfo::detect(&bytes);
        assert!(info.map(|i| i.is_kernel_driver).unwrap_or(false) == false);
    }
}
