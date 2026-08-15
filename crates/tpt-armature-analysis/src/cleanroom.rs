//! Clean-room analyst mode (feature `clean-room`).
//!
//! Enforces a *structural* boundary between recovered binary knowledge and the
//! artifacts an analyst is allowed to publish. Only sanitized, review-safe
//! outputs — a [`RegisterTable`] (from the MMIO pass) or a [`DriverProfile`]
//! (from the driver pass) — may cross the boundary. Raw IR, functions, and
//! decompiled text never do: there is no constructor or function in this module
//! that accepts them.
//!
//! The guarantee is mechanical. [`CleanRoomSource`] is a sealed trait
//! implemented *only* for the sanctioned types (and the seal is not exported, so
//! no downstream crate can implement it). [`export`] is generic over
//! `CleanRoomSource`, so attempting to export a raw instruction or any other
//! type fails to compile. A `trybuild` compile-fail test (`tests/ui`) pins this
//! boundary.

use crate::{DriverProfile, RegisterTable, RegisterTableFormat, export_register_table};
use std::time::{SystemTime, UNIX_EPOCH};

/// Seals [`CleanRoomSource`] so it can only be implemented inside this crate.
pub trait Sealed {}

/// A review-safe artifact that may be exported from a clean-room session.
///
/// Only [`RegisterTable`] and [`DriverProfile`] implement this (see
/// [`Sealed`]). There is deliberately no implementation for raw IR types.
pub trait CleanRoomSource: Sealed {
    /// A stable kind label for the artifact (e.g. `register-table`).
    fn kind(&self) -> &'static str;
    /// Render the review-safe export text for this artifact.
    fn render(&self) -> String;
}

impl Sealed for RegisterTable {}
impl Sealed for DriverProfile {}

impl CleanRoomSource for RegisterTable {
    fn kind(&self) -> &'static str {
        "register-table"
    }

    fn render(&self) -> String {
        export_register_table(self, "MMIO", RegisterTableFormat::Rnndb)
    }
}

impl CleanRoomSource for DriverProfile {
    fn kind(&self) -> &'static str {
        "driver-profile"
    }

    fn render(&self) -> String {
        render_driver_profile(self)
    }
}

/// Metadata recorded for a single exported artifact in the manifest.
#[derive(Debug, Clone)]
pub struct ArtifactMeta {
    /// Artifact kind (`register-table` / `driver-profile`).
    pub kind: &'static str,
    /// SHA-256 (hex) of the rendered artifact content.
    pub sha256: String,
}

/// Audit-trail manifest accompanying a clean-room export.
#[derive(Debug, Clone)]
pub struct Manifest {
    /// Producing tool identifier.
    pub tool: &'static str,
    /// Generation time (Unix epoch seconds).
    pub generated_at: String,
    /// One entry per exported artifact.
    pub artifacts: Vec<ArtifactMeta>,
}

/// A single exported, review-safe artifact.
#[derive(Debug, Clone)]
pub struct Artifact {
    /// Artifact kind.
    pub kind: &'static str,
    /// Rendered, review-safe content.
    pub content: String,
    /// SHA-256 (hex) of `content`.
    pub sha256: String,
}

/// A clean-room export: the audit manifest plus its single artifact.
#[derive(Debug, Clone)]
pub struct CleanRoomExport {
    /// Audit manifest.
    pub manifest: Manifest,
    /// The exported artifact.
    pub artifact: Artifact,
}

impl CleanRoomExport {
    /// Render the manifest as JSON (audit trail).
    pub fn manifest_json(&self) -> String {
        let mut entries = String::new();
        for (i, m) in self.manifest.artifacts.iter().enumerate() {
            let comma = if i + 1 < self.manifest.artifacts.len() {
                ","
            } else {
                ""
            };
            entries.push_str(&format!(
                "\n    {{ \"kind\": \"{}\", \"sha256\": \"{}\" }}{}",
                m.kind, m.sha256, comma
            ));
        }
        format!(
            "{{\n  \"tool\": \"{}\",\n  \"generated_at\": \"{}\",\n  \"artifacts\": [{}]\n}}\n",
            self.manifest.tool, self.manifest.generated_at, entries
        )
    }
}

/// Export a single review-safe source as a clean-room artifact with an
/// accompanying SHA-256 audit manifest.
pub fn export(source: &impl CleanRoomSource) -> CleanRoomExport {
    let kind = source.kind();
    let content = source.render();
    let sha = sha256_hex(&content);
    let artifact = Artifact {
        kind,
        content,
        sha256: sha.clone(),
    };
    let manifest = Manifest {
        tool: "tpt-armature/clean-room",
        generated_at: timestamp(),
        artifacts: vec![ArtifactMeta {
            kind,
            sha256: sha,
        }],
    };
    CleanRoomExport { manifest, artifact }
}

fn render_driver_profile(p: &DriverProfile) -> String {
    let mut s = String::new();
    s.push_str(&format!("is_driver: {}\n", p.is_driver));
    s.push_str(&format!("framework: {:?}\n", p.framework));
    s.push_str(&format!("driver_entry: 0x{:x}\n", p.driver_entry));
    s.push_str("dispatch:\n");
    for (mj, addr) in &p.dispatch {
        s.push_str(&format!("  {:?} -> 0x{:x}\n", mj, addr));
    }
    s.push_str("ioctls:\n");
    for i in &p.ioctls {
        s.push_str(&format!(
            "  0x{:x} dev=0x{:x} fn=0x{:x} method={} access={}\n",
            i.code, i.ctl.device_type, i.ctl.function, i.ctl.method, i.ctl.access
        ));
    }
    s
}

fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let out = h.finalize();
    let mut hex = String::with_capacity(out.len() * 2);
    for b in out {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

fn timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => format!("{}", d.as_secs()),
        Err(_) => "0".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_register_table_with_audit() {
        let table = RegisterTable::new();
        let out = export(&table);
        assert_eq!(out.artifact.kind, "register-table");
        assert_eq!(out.manifest.artifacts.len(), 1);
        assert_eq!(out.manifest.artifacts[0].kind, "register-table");
        assert_eq!(out.manifest.artifacts[0].sha256, out.artifact.sha256);
        assert!(!out.artifact.sha256.is_empty());
        assert!(out.artifact.content.contains("rnndb"));
    }

    #[test]
    fn exports_driver_profile_with_audit() {
        let profile = DriverProfile::default();
        let out = export(&profile);
        assert_eq!(out.artifact.kind, "driver-profile");
        assert!(!out.artifact.sha256.is_empty());
        assert!(out.artifact.content.contains("is_driver"));
    }
}
