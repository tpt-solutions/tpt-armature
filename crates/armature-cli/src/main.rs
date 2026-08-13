//! TPT Armature — command-line driver.
//!
//! Provides a headless pipeline: load a binary, ingest it, disassemble the code
//! section(s), and run the analysis passes. Useful for scripting and CI.

use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "armature",
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
        /// analysis (enables the `debuginfo` feature).
        #[cfg(feature = "debuginfo")]
        #[arg(long)]
        pdb: Option<PathBuf>,
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
            #[cfg(feature = "debuginfo")]
            pdb,
        } => {
            #[cfg(feature = "debuginfo")]
            let pdb_arg = pdb;
            #[cfg(not(feature = "debuginfo"))]
            let pdb_arg: Option<PathBuf> = None;
            cmd_analyze(resolve(path), limit, json, pdb_arg)
        }
        Command::Disasm { path, limit } => cmd_disasm(resolve(path), limit),
        Command::Decompile {
            path,
            function,
            limit,
        } => cmd_decompile(resolve(path), function.as_deref(), limit),
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
    }
}

fn load(path: PathBuf) -> anyhow::Result<Vec<u8>> {
    std::fs::read(&path).map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))
}

fn cmd_analyze(path: PathBuf, limit: usize, json: bool, pdb: Option<PathBuf>) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = if let Some(pdb_path) = pdb {
        let pdb_bytes = std::fs::read(&pdb_path)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", pdb_path.display()))?;
        let map = armature_formats::debuginfo::parse_with_pdb(&bytes, &pdb_bytes)
            .map_err(|e| anyhow::anyhow!("debug-info import failed: {e}"))?;
        armature_analysis::analyze_map(map)?
    } else {
        armature_analysis::analyze_binary(&bytes)?
    };
    let map = &analysis.map;

    if json {
        println!(
            "{}",
            armature_analysis::analysis_to_json(&analysis, &HashMap::new())
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
                .filter(|s| s.kind == armature_formats::DebugSymbolKind::Function)
                .count(),
            map.debug_symbols
                .iter()
                .filter(|s| s.kind == armature_formats::DebugSymbolKind::Data)
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

    Ok(())
}

fn cmd_disasm(path: PathBuf, limit: usize) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = armature_analysis::analyze_binary(&bytes)?;

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

fn cmd_decompile(
    path: PathBuf,
    function: Option<&str>,
    limit: usize,
) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = armature_analysis::analyze_binary(&bytes)?;

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
        print!("{}", armature_analysis::decompile_function(func, &Default::default()));
        shown += 1;
    }
    if target.is_some() && shown == 0 {
        eprintln!("no function found at the requested address");
    }
    Ok(())
}

fn cmd_cfg(path: PathBuf) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = armature_analysis::analyze_binary(&bytes)?;
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
    let analysis = armature_analysis::analyze_binary(&bytes)?;
    let source = std::fs::read_to_string(&script)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", script.display()))?;
    let host = armature_ext::ScriptHost::new(&analysis);
    let renames = host.run(&source)?;

    let fmt = armature_analysis::RenameFormat::parse(format).ok_or_else(|| {
        anyhow::anyhow!("unknown rename format '{format}' (expected json|csv|idc)")
    })?;
    let serialized = armature_analysis::export_renames(&renames, fmt);

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
    let analysis = armature_analysis::analyze_binary(&bytes)?;
    let strings = armature_analysis::extract_strings(&analysis.map, min_len);
    println!("== TPT Armature :: Strings ==");
    println!("{} string(s) found (min length {}):", strings.len(), min_len);
    for s in &strings {
        let kind = match s.kind {
            armature_analysis::StringKind::Ascii => "ascii",
            armature_analysis::StringKind::Utf16 => "utf16",
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
    let analysis = armature_analysis::analyze_binary(&bytes)?;

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
        let mut host = armature_ext::PluginHost::load(plugin)
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
    let analysis = armature_analysis::analyze_binary(&bytes)?;
    let mut host = armature_ext::PluginHost::load(&plugin)
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
    let analysis = armature_analysis::analyze_binary(&bytes)?;
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
        t.parse::<u64>().map_err(|_| format!("invalid address: {s}"))
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}
