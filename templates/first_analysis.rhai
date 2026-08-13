/// First analysis — rename exported functions that call `printf`.
///
/// Run:
///   cargo run -p tpt-armature-cli --features rhai -- script <binary> templates/first_analysis.rhai
///
/// The Rhai host injects these bindings (see templates/rhai_cheatsheet.md):
///   format, arch, entry, instruction_count, function_count : scalars
///   imports        : array of #{ name, dll }
///   exports        : array of #{ name, addr, targets }
///   symbol_xrefs   : array of #{ from, to }
///   symbol_name(a) : resolve a symbol address to its name
///   rename(a, n)   : record a symbol rename
///
/// This script mirrors the built-in default but is annotated so you can adapt
/// it. It scans every export's symbol cross-references; any export that reaches
/// a `printf`-style import is renamed `calls_printf_<name>`.

const TARGET = "printf";

print("first_analysis: scanning for calls to imports containing '" + TARGET + "'");

let renamed = 0;
for ex in exports {
    let hit = false;
    for t in ex.targets {
        let nm = symbol_name(t);
        if nm.contains(TARGET) {
            hit = true;
            break;
        }
    }
    if hit {
        rename(ex.addr, "calls_" + TARGET + "_" + ex.name);
        renamed = renamed + 1;
    }
}

print("first_analysis: renamed " + renamed + " export(s)");
