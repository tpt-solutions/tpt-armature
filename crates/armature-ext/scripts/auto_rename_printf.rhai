// Example Rhai script: auto-rename exported functions that reference, via a
// symbol cross-reference, a symbol whose name contains "printf".
//
// Run with:
//   armature-cli script <binary> scripts/auto_rename_printf.rhai
// (or load it from the GUI's script console once that is wired up).

for ex in exports {
    for t in ex.targets {
        let nm = symbol_name(t);
        if nm.contains("printf") {
            rename(ex.addr, "calls_printf_" + ex.name);
        }
    }
}
