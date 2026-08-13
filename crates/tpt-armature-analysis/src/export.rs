//! Export of analysis results and symbol renames for automation / CI.
//!
//! Renames produced by scripts or Wasm plugins are in-memory only by default;
//! these helpers serialize them to JSON, CSV, or an IDA `.idc` script so the
//! results can be persisted or fed into other tooling.

use std::collections::HashMap;

use crate::Analysis;

/// Output format for [`export_renames`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameFormat {
    /// UTF-8 JSON: `{ "renames": [ { "addr": "...", "name": "..." } ] }`.
    Json,
    /// Two-column CSV: `addr,name`.
    Csv,
    /// IDA Pro IDC script of `MakeName(addr, "name");` calls.
    Idc,
}

impl RenameFormat {
    /// Parse a format name from a CLI string (`json`/`csv`/`idc`).
    pub fn parse(s: &str) -> Option<RenameFormat> {
        match s.to_ascii_lowercase().as_str() {
            "json" => Some(RenameFormat::Json),
            "csv" => Some(RenameFormat::Csv),
            "idc" => Some(RenameFormat::Idc),
            _ => None,
        }
    }
}

/// Serialize address -> name renames to the requested [`RenameFormat`].
pub fn export_renames(renames: &HashMap<u64, String>, format: RenameFormat) -> String {
    let mut entries: Vec<(&u64, &String)> = renames.iter().collect();
    entries.sort_by_key(|(a, _)| **a);

    match format {
        RenameFormat::Json => {
            let mut s = String::from("{\n  \"renames\": [\n");
            for (i, (addr, name)) in entries.iter().enumerate() {
                let comma = if i + 1 < entries.len() { "," } else { "" };
                s.push_str(&format!(
                    "    {{ \"addr\": \"0x{:x}\", \"name\": \"{}\" }}{}\n",
                    addr,
                    escape_json(name),
                    comma
                ));
            }
            s.push_str("  ]\n}\n");
            s
        }
        RenameFormat::Csv => {
            let mut s = String::from("addr,name\n");
            for (addr, name) in entries {
                s.push_str(&format!("0x{:x},{}\n", addr, name));
            }
            s
        }
        RenameFormat::Idc => {
            let mut s = String::from("#include <idc.idc>\n\nstatic main() {\n");
            for (addr, name) in entries {
                s.push_str(&format!("    MakeName(0x{:x}, \"{}\");\n", addr, name));
            }
            s.push_str("}\n");
            s
        }
    }
}

/// A compact JSON summary of the analysis, suitable for CI automation and diffing.
pub fn analysis_to_json(analysis: &Analysis, renames: &HashMap<u64, String>) -> String {
    let map = &analysis.map;
    let mut renames_json = String::new();
    let mut first = true;
    let mut entries: Vec<(&u64, &String)> = renames.iter().collect();
    entries.sort_by_key(|(a, _)| **a);
    for (addr, name) in entries {
        if !first {
            renames_json.push(',');
        }
        renames_json.push_str(&format!(
            "\n    {{ \"addr\": \"0x{:x}\", \"name\": \"{}\" }}",
            addr,
            escape_json(name)
        ));
        first = false;
    }
    format!(
        "{{\n  \"format\": \"{}\",\n  \"arch\": \"{}\",\n  \"entry_point\": \"0x{:x}\",\n  \
         \"base_address\": \"0x{:x}\",\n  \"sections\": {},\n  \"imports\": {},\n  \
         \"exports\": {},\n  \"instructions\": {},\n  \"functions\": {},\n  \"blocks\": {},\n  \
         \"edges\": {},\n  \"loops\": {},\n  \"xrefs\": {},\n  \"registers\": [{}],\n  \
         \"renames\": [{}]\n}}\n",
        map.format,
        map.arch,
        map.entry_point,
        map.base_address,
        map.sections.len(),
        map.imports.len(),
        map.exports.len(),
        analysis.instructions.len(),
        analysis.module.functions.len(),
        analysis.cfg.block_count(),
        analysis.cfg.edge_count(),
        analysis.cfg.loop_count,
        analysis.xrefs.count(),
        analysis
            .dataflow
            .registers()
            .iter()
            .map(|r| format!("\"{r}\""))
            .collect::<Vec<_>>()
            .join(", "),
        renames_json,
    )
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}
