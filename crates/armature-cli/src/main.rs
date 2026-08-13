//! TPT Armature — command-line driver.
//!
//! Provides a headless pipeline: load a binary, ingest it, disassemble the code
//! section, and run the analysis passes. Useful for scripting and CI.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    },
    /// Disassemble and print assembly text.
    Disasm {
        /// Path to the binary.
        path: PathBuf,
        /// Maximum number of instructions to print (0 = all).
        #[arg(short = 'n', long, default_value_t = 64)]
        limit: usize,
    },
    /// Print control-flow graph statistics and edges.
    Cfg {
        /// Path to the binary.
        path: PathBuf,
    },
    /// Run a Rhai automation script against the analyzed binary.
    #[cfg(feature = "rhai")]
    Script {
        /// Path to the binary.
        path: PathBuf,
        /// Path to the `.rhai` script to execute.
        script: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Allow `-f` to override the subcommand's path for convenience.
    let resolve = |p: PathBuf| -> PathBuf { cli.file.clone().unwrap_or(p) };

    match cli.command {
        Command::Analyze { path, limit } => cmd_analyze(resolve(path), limit),
        Command::Disasm { path, limit } => cmd_disasm(resolve(path), limit),
        Command::Cfg { path } => cmd_cfg(resolve(path)),
        #[cfg(feature = "rhai")]
        Command::Script { path, script } => cmd_script(resolve(path), script),
    }
}

fn load(path: PathBuf) -> anyhow::Result<Vec<u8>> {
    std::fs::read(&path).map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))
}

fn cmd_analyze(path: PathBuf, limit: usize) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = armature_analysis::analyze_binary(&bytes)?;
    let map = &analysis.map;

    println!("== TPT Armature :: Analysis ==");
    println!("format       : {}", map.format);
    println!("architecture : {}", map.arch);
    println!("entry point  : 0x{:x}", map.entry_point);
    println!("base address : 0x{:x}", map.base_address);
    println!("sections     : {}", map.sections.len());
    println!("imports      : {}", map.imports.len());
    println!("exports      : {}", map.exports.len());
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

    let iter = analysis.instructions.iter();
    let shown: Vec<_> = if limit == 0 {
        iter.collect()
    } else {
        iter.take(limit).collect()
    };

    for ins in shown {
        println!(
            "0x{:08x}  {:<6}  {}",
            ins.address,
            hex_bytes(&ins.raw),
            ins.text
        );
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
        println!("  ... {} more edge(s) not shown", analysis.cfg.edges.len() - CAP);
    }
    Ok(())
}

#[cfg(feature = "rhai")]
fn cmd_script(path: PathBuf, script: PathBuf) -> anyhow::Result<()> {
    let bytes = load(path)?;
    let analysis = armature_analysis::analyze_binary(&bytes)?;
    let source = std::fs::read_to_string(&script)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", script.display()))?;
    let host = armature_ext::ScriptHost::new(&analysis);
    let renames = host.run(&source)?;
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
    Ok(())
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}
