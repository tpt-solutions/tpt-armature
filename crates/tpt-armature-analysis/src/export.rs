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

/// Parse address -> name renames from the requested [`RenameFormat`].
///
/// This is the inverse of [`export_renames`] and lets renames produced by a
/// script or plugin be persisted and re-loaded (e.g. `analyze --rename-file`)
/// so they round-trip between automation runs.
pub fn parse_renames(text: &str, format: RenameFormat) -> Result<HashMap<u64, String>, String> {
    let mut out = HashMap::new();
    match format {
        RenameFormat::Json => {
            let value: serde_json::Value =
                serde_json::from_str(text).map_err(|e| format!("invalid rename JSON: {e}"))?;
            let arr = value
                .get("renames")
                .and_then(|r| r.as_array())
                .ok_or_else(|| "rename JSON missing a \"renames\" array".to_string())?;
            for item in arr {
                let addr_raw = item
                    .get("addr")
                    .and_then(|a| a.as_str().map(|s| s.to_string()))
                    .or_else(|| item.get("addr").map(|a| a.to_string()));
                let name = item.get("name").and_then(|n| n.as_str());
                if let (Some(a), Some(n)) = (addr_raw, name) {
                    if let Some(addr) = parse_addr_any(&a) {
                        out.insert(addr, n.to_string());
                    }
                }
            }
        }
        RenameFormat::Csv => {
            for line in text
                .lines()
                .skip_while(|l| l.trim().eq_ignore_ascii_case("addr,name"))
            {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Some((a, n)) = line.split_once(',') {
                    if let Some(addr) = parse_addr_any(a.trim()) {
                        out.insert(addr, n.trim().to_string());
                    }
                }
            }
        }
        RenameFormat::Idc => {
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("MakeName(") {
                    if let Some(end) = rest.find(')') {
                        let inner = &rest[..end];
                        if let Some((a, n)) = inner.split_once(',') {
                            let name = n.trim().trim_matches('"').trim_matches('\'');
                            if let Some(addr) = parse_addr_any(a.trim()) {
                                out.insert(addr, name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Parse an address from `0x…` / `0X…` (hex) or a plain decimal string.
fn parse_addr_any(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        t.parse::<u64>().ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_round_trip_json() {
        let mut renames = HashMap::new();
        renames.insert(0x401000u64, "main".to_string());
        renames.insert(0x402000u64, "helper".to_string());
        let json = export_renames(&renames, RenameFormat::Json);
        let parsed = parse_renames(&json, RenameFormat::Json).unwrap();
        assert_eq!(parsed, renames);
    }

    #[test]
    fn parses_csv_and_idc() {
        let csv = "addr,name\n0x401000,main\n0x402000,helper\n";
        let parsed = parse_renames(csv, RenameFormat::Csv).unwrap();
        assert_eq!(parsed.get(&0x401000), Some(&"main".to_string()));
        assert_eq!(parsed.get(&0x402000), Some(&"helper".to_string()));

        let idc = "#include <idc.idc>\nstatic main() {\n    MakeName(0x401000, \"main\");\n}\n";
        let parsed = parse_renames(idc, RenameFormat::Idc).unwrap();
        assert_eq!(parsed.get(&0x401000), Some(&"main".to_string()));
    }
}
