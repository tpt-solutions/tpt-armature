//! TPT Armature — command-line driver.
//!
//! Provides a headless pipeline: load a binary, ingest it, disassemble the code
//! section(s), and run the analysis passes. Useful for scripting and CI.

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "tpt-armature",
    version,
    about = "TPT Armature — Rust reverse engineering suite (CLI)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Path to the binary to analyze.
    #[arg(short = 'f', long, global = true)]
    file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Print a high-level summary of the binary and its analysis.
    Analyze {
        /// Path to the binary.
        path: PathBuf,
        /// Maximum number of instructions to disassemble.
        #[arg(short = 'n', long, default_value_t = 0)]
        limit: usize,
        /// Emit the analysis summary as JSON (for automation / CI).
        #[arg(long)]
        json: bool,
        /// Optional PDB file (PE) whose public symbols are merged into the
        /// analysis. Requires the `debuginfo` feature; without it the run
        /// fails with a clear message instead of an "unexpected argument" error.
        #[arg(long)]
        pdb: Option<PathBuf>,
        /// Load previously exported renames (json|csv|idc) and apply them to
        /// function labels and JSON output (automation round-trip).
        #[arg(long)]
        rename_file: Option<PathBuf>,
        /// Format of `--rename-file` (json|csv|idc).
        #[arg(long, default_value = "json")]
        rename_format: String,
    },
    /// Disassemble and print assembly text.
    Disasm {
        /// Path to the binary.
        path: PathBuf,
        /// Maximum number of instructions to print (0 = all, capped).
        #[arg(short = 'n', long, default_value_t = 0)]
        limit: usize,
    },
    /// Render a C-like pseudocode view of the recovered functions.
    Decompile {
        /// Path to the binary.
        path: PathBuf,
        /// Show only the function containing this address (hex, e.g. 0x401000).
        #[arg(long)]
        function: Option<String>,
        /// Maximum number of functions to emit (0 = all).
        #[arg(short = 'n', long, default_value_t = 0)]
        limit: usize,
    },
    /// Bindiff-style comparison of two binaries (exports, imports, functions).
    Diff {
        /// Path to the "before" binary.
        a: PathBuf,
        /// Path to the "after" binary.
        b: PathBuf,
    },
    /// Print control-flow graph statistics and edges.
    Cfg {
        /// Path to the binary.
        path: PathBuf,
    },
    /// Extract printable strings and immediate constants from the binary.
    Strings {
        /// Path to the binary.
        path: PathBuf,
        /// Minimum string length to report.
        #[arg(short = 'm', long, default_value_t = 4)]
        min_len: usize,
        /// Also print the distinct immediate constants.
        #[arg(long)]
        constants: bool,
    },
    /// Run a Rhai automation script against the analyzed binary.
    #[cfg(feature = "rhai")]
    Script {
        /// Path to the binary.
        path: PathBuf,
        /// Path to the `.rhai` script to execute.
        script: PathBuf,
        /// Write the produced renames to a file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Rename export format: json | csv | idc.
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Run a sandboxed Wasm plugin against the analyzed binary.
    #[cfg(feature = "wasm")]
    Plugin {
        /// Path to the binary.
        path: PathBuf,
        /// Path to the `.wasm` plugin to run.
        plugin: PathBuf,
    },
    /// Run every `.wasm` plugin found in a directory against the binary
    /// (auto-discovery of a plugin folder).
    #[cfg(feature = "wasm")]
    Plugins {
        /// Path to the binary.
        path: PathBuf,
        /// Directory to scan for `.wasm` plugins.
        plugin_dir: PathBuf,
    },
    /// Re-analyze a binary whenever it changes on disk (polls modification
    /// time). Useful while iterating on a build under development.
    Watch {
        /// Path to the binary to watch.
        path: PathBuf,
        /// Poll interval in seconds.
        #[arg(short = 'i', long, default_value_t = 1)]
        interval: u64,
    },
    /// Serve the analysis of a binary over HTTP (headless web UI). Useful for
    /// CI containers and collaboration. Requires the `serve` feature.
    #[cfg(feature = "serve")]
    Serve {
        /// Path to the binary to analyze and serve.
        path: PathBuf,
        /// Port to listen on.
        #[arg(long, default_value_t = 8080)]
        port: u16,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Allow `-f` to override the subcommand's path for convenience.
    let resolve = |p: PathBuf| -> PathBuf { cli.file.clone().unwrap_or(p) };

    match cli.command {
        Command::Analyze {
            path,
            limit,
            json,
            pdb,
            rename_file,
            rename_format,
        } => cmd_analyze(resolve(path), limit, json, pdb, rename_file, &rename_format),
        Command::Disasm { path, limit } => cmd_disasm(resolve(path), limit),
        Command::Decompile {
            path,
            function,
            limit,
        } => cmd_decompile(resolve(path), function.as_deref(), limit),
        Command::Diff { a, b } => cmd_diff(resolve(a), resolve(b)),
        Command::Cfg { path } => cmd_cfg(resolve(path)),
        Command::Strings {
            path,
            min_len,
            constants,
        } => cmd_strings(resolve(path), min_len, constants),
        #[cfg(feature = "rhai")]
        Command::Script {
            path,
            script,
            out,
            format,
        } => cmd_script(resolve(path), script, out, &format),
        #[cfg(feature = "wasm")]
        Command::Plugin { path, plugin } => cmd_plugin(resolve(path), plugin),
        #[cfg(feature = "wasm")]
        Command::Plugins { path, plugin_dir } => cmd_plugins(resolve(path), plugin_dir),
        Command::Watch { path, interval } => cmd_watch(resolve(path), interval),
        #[cfg(feature = "serve")]
        Command::Serve { path, port } => cmd_serve(resolve(path), port),
    }
}

fn load(path: PathBuf) -> anyhow::Result<Vec<u8>> {
    std::fs::read(&path).map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))
}

fn cmd_analyze(
    path: PathBuf,
    limit: usize,
    json: bool,
    pdb: Option<PathBuf>,
    rename_file: Option<PathBuf>,
    rename_format: &str,
) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let mut analysis = if let Some(pdb_path) = pdb {
        #[cfg(feature = "debuginfo")]
        {
            let pdb_bytes = std::fs::read(&pdb_path)
                .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", pdb_path.display()))?;
            let map = tpt_armature_formats::debuginfo::parse_with_pdb(&bytes, &pdb_bytes)
                .map_err(|e| anyhow::anyhow!("debug-info import failed: {e}"))?;
            tpt_armature_analysis::analyze_map(map)?
        }
        #[cfg(not(feature = "debuginfo"))]
        {
            let _ = pdb_path;
            anyhow::bail!(
                "this build of tpt-armature was compiled without the `debuginfo` feature; \
                 rebuild with `--features debuginfo` to use `--pdb`"
            )
        }
    } else {
        tpt_armature_analysis::analyze_binary(&bytes)?
    };

    // Load and apply any previously exported renames (automation round-trip).
    let mut renames: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
    if let Some(rf) = rename_file {
        let text = std::fs::read_to_string(&rf)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", rf.display()))?;
        let fmt = tpt_armature_analysis::RenameFormat::parse(rename_format).ok_or_else(|| {
            anyhow::anyhow!("unknown rename format '{rename_format}' (expected json|csv|idc)")
        })?;
        renames = tpt_armature_analysis::parse_renames(&text, fmt)
            .map_err(|e| anyhow::anyhow!("cannot parse renames: {e}"))?;
        for func in &mut analysis.module.functions {
            if let Some(name) = renames.get(&func.start) {
                func.name = Some(name.clone());
            }
        }
    }

    let map = &analysis.map;

    if json {
        println!(
            "{}",
            tpt_armature_analysis::analysis_to_json(&analysis, &renames)
        );
        return Ok(());
    }

    println!("== TPT Armature :: Analysis ==");
    println!("format       : {}", map.format);
    println!("architecture : {}", map.arch);
    println!("entry point  : 0x{:x}", map.entry_point);
    println!("base address : 0x{:x}", map.base_address);
    println!("sections     : {}", map.sections.len());
    println!("imports      : {}", map.imports.len());
    println!("exports      : {}", map.exports.len());
    if !map.debug_symbols.is_empty() {
        println!(
            "debug syms   : {} ({} functions, {} data)",
            map.debug_symbols.len(),
            map.debug_symbols
                .iter()
                .filter(|s| s.kind == tpt_armature_formats::DebugSymbolKind::Function)
                .count(),
            map.debug_symbols
                .iter()
                .filter(|s| s.kind == tpt_armature_formats::DebugSymbolKind::Data)
                .count(),
        );
    }
    println!(
        "code section : {} ({} bytes)",
        analysis
            .code_section()
            .map(|s| s.name.as_str())
            .unwrap_or("<none>"),
        analysis.code_section().map(|s| s.data.len()).unwrap_or(0)
    );
    let total = if limit == 0 {
        analysis.instructions.len()
    } else {
        analysis.instructions.len().min(limit)
    };
    println!(
        "instructions : {} (showing {})",
        analysis.instructions.len(),
        total
    );
    println!("functions    : {}", analysis.module.functions.len());
    println!("{}", analysis.cfg.summary());
    println!("xrefs        : {}", analysis.xrefs.count());
    println!(
        "registers    : {}",
        analysis.dataflow.registers().join(", ")
    );
    if !renames.is_empty() {
        println!("renames      : {} applied", renames.len());
    }

    Ok(())
}

fn cmd_disasm(path: PathBuf, limit: usize) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;

    // Default cap mirrors the `cfg` view: without `-n`, show a sane window rather
    // than dumping the entire instruction stream.
    const DEFAULT_CAP: usize = 256;
    let (shown, truncated) = if limit == 0 {
        let n = analysis.instructions.len().min(DEFAULT_CAP);
        (n, analysis.instructions.len() > DEFAULT_CAP)
    } else {
        let n = analysis.instructions.len().min(limit);
        (n, analysis.instructions.len() > limit)
    };

    for ins in &analysis.instructions[..shown] {
        println!(
            "0x{:08x}  {:<6}  {}",
            ins.address,
            hex_bytes(&ins.raw),
            ins.text
        );
    }
    if truncated {
        println!(
            "  ... {} more instruction(s) not shown (pass -n <count> for more)",
            analysis.instructions.len() - shown
        );
    }
    Ok(())
}

fn cmd_decompile(path: PathBuf, function: Option<&str>, limit: usize) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;

    let target = function
        .map(parse_addr)
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let funcs = &analysis.module.functions;
    let mut shown = 0usize;
    for func in funcs {
        if let Some(addr) = target {
            if func.start != addr {
                continue;
            }
        }
        if limit != 0 && shown >= limit {
            break;
        }
        print!(
            "{}",
            tpt_armature_analysis::decompile_function(func, &Default::default())
        );
        shown += 1;
    }
    if target.is_some() && shown == 0 {
        eprintln!("no function found at the requested address");
    }
    Ok(())
}

/// Bindiff-style comparison of two binaries: which exports/imports changed,
/// how the function/instruction counts moved, and string-count deltas.
fn cmd_diff(a: PathBuf, b: PathBuf) -> anyhow::Result<()> {
    let ba = load(a.clone())?;
    let bb = load(b.clone())?;
    let la = tpt_armature_analysis::analyze_binary(&ba)?;
    let lb = tpt_armature_analysis::analyze_binary(&bb)?;

    let name = |p: &PathBuf| {
        p.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    println!("== TPT Armature :: Diff ==");
    println!("  A: {}  ({} {})", name(&a), la.map.format, la.map.arch);
    println!("  B: {}  ({} {})", name(&b), lb.map.format, lb.map.arch);

    println!("\n[counts]");
    println!(
        "  functions : {} -> {} ({:+})",
        la.module.functions.len(),
        lb.module.functions.len(),
        lb.module.functions.len() as i64 - la.module.functions.len() as i64
    );
    println!(
        "  instructions: {} -> {} ({:+})",
        la.instructions.len(),
        lb.instructions.len(),
        lb.instructions.len() as i64 - la.instructions.len() as i64
    );
    println!(
        "  strings   : {} -> {} ({:+})",
        la.strings.len(),
        lb.strings.len(),
        lb.strings.len() as i64 - la.strings.len() as i64
    );
    println!(
        "  constants : {} -> {} ({:+})",
        la.constants.len(),
        lb.constants.len(),
        lb.constants.len() as i64 - la.constants.len() as i64
    );

    diff_set(
        "exports",
        &names_of(&la.map.exports),
        &names_of(&lb.map.exports),
    );
    diff_set(
        "imports",
        &import_names(&la.map.imports),
        &import_names(&lb.map.imports),
    );
    Ok(())
}

fn names_of(exports: &[tpt_armature_formats::Export]) -> Vec<String> {
    exports.iter().map(|e| e.name.clone()).collect()
}

fn import_names(imports: &[tpt_armature_formats::Import]) -> Vec<String> {
    imports
        .iter()
        .map(|i| format!("{}/{}", i.library, i.name.clone().unwrap_or_default()))
        .collect()
}

fn diff_set(label: &str, a: &[String], b: &[String]) {
    let sa: std::collections::BTreeSet<&String> = a.iter().collect();
    let sb: std::collections::BTreeSet<&String> = b.iter().collect();
    let added: Vec<_> = sb.difference(&sa).collect();
    let removed: Vec<_> = sa.difference(&sb).collect();
    println!("\n[{label}]  (+{}, -{})", added.len(), removed.len());
    for n in added {
        println!("  + {n}");
    }
    for n in removed {
        println!("  - {n}");
    }
}

fn cmd_cfg(path: PathBuf) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;
    println!("{}", analysis.cfg.summary());
    const CAP: usize = 200;
    let shown = analysis.cfg.edges.len().min(CAP);
    for e in &analysis.cfg.edges[..shown] {
        let from = analysis.cfg.nodes[e.from].start;
        println!(
            "  block 0x{:x} --[{}]--> block 0x{:x} (0x{:x})",
            from, e.kind, analysis.cfg.nodes[e.to].start, e.target_addr
        );
    }
    if analysis.cfg.edges.len() > CAP {
        println!(
            "  ... {} more edge(s) not shown",
            analysis.cfg.edges.len() - CAP
        );
    }
    Ok(())
}

#[cfg(feature = "rhai")]
fn cmd_script(
    path: PathBuf,
    script: PathBuf,
    out: Option<PathBuf>,
    format: &str,
) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;
    let source = std::fs::read_to_string(&script)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", script.display()))?;
    let host = tpt_armature_ext::ScriptHost::new(&analysis);
    let renames = host.run(&source)?;

    let fmt = tpt_armature_analysis::RenameFormat::parse(format).ok_or_else(|| {
        anyhow::anyhow!("unknown rename format '{format}' (expected json|csv|idc)")
    })?;
    let serialized = tpt_armature_analysis::export_renames(&renames, fmt);

    match out {
        Some(file) => {
            std::fs::write(&file, &serialized)
                .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", file.display()))?;
            println!("wrote {} rename(s) to {}", renames.len(), file.display());
        }
        None => {
            println!("== TPT Armature :: Script ==");
            if renames.is_empty() {
                println!("script produced no renames");
            } else {
                let mut entries: Vec<_> = renames.iter().collect();
                entries.sort_by_key(|(addr, _)| **addr);
                for (addr, name) in entries {
                    println!("0x{addr:x} -> {name}");
                }
            }
        }
    }
    Ok(())
}

fn cmd_strings(path: PathBuf, min_len: usize, show_constants: bool) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;
    let strings = tpt_armature_analysis::extract_strings(&analysis.map, min_len);
    println!("== TPT Armature :: Strings ==");
    println!(
        "{} string(s) found (min length {}):",
        strings.len(),
        min_len
    );
    for s in &strings {
        let kind = match s.kind {
            tpt_armature_analysis::StringKind::Ascii => "ascii",
            tpt_armature_analysis::StringKind::Utf16 => "utf16",
        };
        println!("  0x{:08x}  [{:5}]  {}", s.addr, kind, s.text);
    }
    if show_constants {
        println!("\n{} distinct constant(s):", analysis.constants.len());
        for c in &analysis.constants {
            println!("  0x{c:x}");
        }
    }
    Ok(())
}

#[cfg(feature = "wasm")]
fn cmd_plugins(path: PathBuf, plugin_dir: PathBuf) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;

    let mut wasm_files: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(&plugin_dir)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", plugin_dir.display()))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("wasm") {
            wasm_files.push(p);
        }
    }
    wasm_files.sort();

    if wasm_files.is_empty() {
        println!("no .wasm plugins found in {}", plugin_dir.display());
        return Ok(());
    }

    for plugin in &wasm_files {
        let mut host = tpt_armature_ext::PluginHost::load(plugin)
            .map_err(|e| anyhow::anyhow!("cannot load plugin {}: {e}", plugin.display()))?;
        host.bind_analysis(&analysis);
        let output = host
            .run()
            .map_err(|e| anyhow::anyhow!("plugin {} failed: {e}", plugin.display()))?;
        println!(
            "== TPT Armature :: Plugin {} ==",
            plugin.file_name().and_then(|s| s.to_str()).unwrap_or("?")
        );
        if output.renames.is_empty() {
            println!("  plugin produced no renames");
        } else {
            let mut entries: Vec<_> = output.renames.iter().collect();
            entries.sort_by_key(|(addr, _)| **addr);
            for (addr, name) in entries {
                println!("  0x{addr:x} -> {name}");
            }
        }
        for line in &output.logs {
            println!("  [plugin] {line}");
        }
    }
    Ok(())
}

#[cfg(feature = "wasm")]
fn cmd_plugin(path: PathBuf, plugin: PathBuf) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;
    let mut host = tpt_armature_ext::PluginHost::load(&plugin)
        .map_err(|e| anyhow::anyhow!("cannot load plugin: {e}"))?;
    host.bind_analysis(&analysis);
    let output = host
        .run()
        .map_err(|e| anyhow::anyhow!("plugin run failed: {e}"))?;
    println!("== TPT Armature :: Plugin ==");
    if output.renames.is_empty() {
        println!("plugin produced no renames");
    } else {
        let mut entries: Vec<_> = output.renames.iter().collect();
        entries.sort_by_key(|(addr, _)| **addr);
        for (addr, name) in entries {
            println!("0x{addr:x} -> {name}");
        }
    }
    for line in &output.logs {
        println!("[plugin] {line}");
    }
    Ok(())
}

fn cmd_watch(path: PathBuf, interval: u64) -> anyhow::Result<()> {
    use std::time::{Duration, UNIX_EPOCH};

    println!(
        "watching {} (every {}s; Ctrl-C to stop)",
        path.display(),
        interval
    );

    let mut last = None::<u64>;
    loop {
        let mtime = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        if mtime != last {
            last = mtime;
            match analyze_and_summarize(&path) {
                Ok(summary) => println!("{summary}"),
                Err(e) => eprintln!("re-analysis failed: {e}"),
            }
        }
        std::thread::sleep(Duration::from_secs(interval.max(1)));
    }
}

/// Analyze a binary and return a one-line status summary. Shared by `watch`.
fn analyze_and_summarize(path: &Path) -> anyhow::Result<String> {
    let bytes = load(path.to_path_buf())?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;
    let map = &analysis.map;
    Ok(format!(
        "[{}] {} {} | {} instructions | {} functions | {}",
        chrono_stamp(),
        map.format,
        map.arch,
        analysis.instructions.len(),
        analysis.module.functions.len(),
        analysis.cfg.summary()
    ))
}

fn chrono_stamp() -> String {
    // Lightweight wall-clock stamp without another dependency.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (h, m, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
    format!("{h:02}:{m:02}:{s:02}")
}

fn parse_addr(s: &str) -> Result<u64, String> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).map_err(|_| format!("invalid hex address: {s}"))
    } else {
        t.parse::<u64>()
            .map_err(|_| format!("invalid address: {s}"))
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

/// Escape the few characters that would break an HTML `<pre>` block.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Serve the analysis of a binary over HTTP (headless web UI).
///
/// The binary is analyzed once at startup, then every request returns the
/// analysis as JSON (`/api/analyze`) or a small HTML viewer (`/`) so the data
/// can be inspected from a browser or pulled by automation without a GUI.
#[cfg(feature = "serve")]
fn cmd_serve(path: PathBuf, port: u16) -> anyhow::Result<()> {
    use std::net::SocketAddr;

    let bytes = load(path)?;
    let analysis = tpt_armature_analysis::analyze_binary(&bytes)?;
    let renames = std::collections::HashMap::new();
    let json = tpt_armature_analysis::analysis_to_json(&analysis, &renames);
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>TPT Armature</title></head>\
         <body><h1>TPT Armature — headless analysis</h1>\
         <p>JSON API: <a href=\"/api/analyze\">/api/analyze</a></p>\
         <pre>{}</pre></body></html>",
        escape_html(&json)
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let server = tiny_http::Server::http(addr)
        .map_err(|e| anyhow::anyhow!("cannot start HTTP server on {addr}: {e}"))?;
    println!("tpt-armature serving on http://{addr} (Ctrl-C to stop)");

    for request in server.incoming_requests() {
        let (content_type, body) = if request.url() == "/api/analyze" {
            ("application/json; charset=utf-8", json.clone())
        } else {
            ("text/html; charset=utf-8", html.clone())
        };
        let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
            .expect("Content-Type is valid ASCII");
        let response = tiny_http::Response::from_string(body).with_header(header);
        let _ = request.respond(response);
    }
    Ok(())
}
