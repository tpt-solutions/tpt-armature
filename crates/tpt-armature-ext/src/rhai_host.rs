//! Rhai scripting host.
//!
//! Binds the analyzed binary's symbols and cross-references into a Rhai scope
//! and exposes a `rename(addr, name)` function so scripts can mutate the view
//! (e.g. auto-rename functions that call `printf`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use tpt_armature_analysis::Analysis;

/// The accumulated renames a script produced, keyed by virtual address.
pub type Renames = HashMap<u64, String>;

/// Host that runs Rhai automation scripts against an [`Analysis`].
pub struct ScriptHost {
    engine: rhai::Engine,
    scope: rhai::Scope<'static>,
    renames: Rc<RefCell<Renames>>,
}

impl ScriptHost {
    /// Build a host bound to a specific analysis result.
    pub fn new(analysis: &Analysis) -> Self {
        let mut engine = rhai::Engine::new();

        let renames: Rc<RefCell<Renames>> = Rc::new(RefCell::new(HashMap::new()));
        let renames_fn = renames.clone();
        engine.register_fn("rename", move |addr: i64, name: &str| {
            renames_fn
                .borrow_mut()
                .insert(addr as u64, name.to_string());
        });

        let mut scope = rhai::Scope::new();
        scope.push("format", analysis.map.format.to_string());
        scope.push("arch", analysis.map.arch.to_string());
        scope.push("entry", analysis.map.entry_point as i64);
        scope.push("instruction_count", analysis.instructions.len() as i64);
        scope.push("function_count", analysis.module.functions.len() as i64);

        // Symbol name lookup: export address (as a string) -> name.
        let mut symbol_names = rhai::Map::new();
        let mut symbol_by_addr: HashMap<i64, String> = HashMap::new();
        for e in &analysis.map.exports {
            if e.addr != 0 {
                symbol_names.insert(e.addr.to_string().into(), e.name.clone().into());
                symbol_by_addr.insert(e.addr as i64, e.name.clone());
            }
        }
        scope.push("symbol_names", symbol_names);

        // Native lookup so scripts can resolve a symbol name by its numeric
        // address (Rhai Map keys must be strings, so `symbol_names[addr]` would
        // not match an integer key).
        let sym_fn = symbol_by_addr.clone();
        engine.register_fn("symbol_name", move |addr: i64| -> String {
            sym_fn.get(&addr).cloned().unwrap_or_default()
        });

        // Imports: array of { name, dll }.
        let imports: rhai::Array = analysis
            .map
            .imports
            .iter()
            .map(|i| {
                let mut m = rhai::Map::new();
                m.insert("name".into(), i.name.clone().unwrap_or_default().into());
                m.insert("dll".into(), i.library.clone().into());
                m.into()
            })
            .collect();
        scope.push("imports", imports);

        // Per-export symbol cross-reference targets, computed over the export's
        // approximate address range.
        let mut exports_sorted: Vec<_> = analysis
            .map
            .exports
            .iter()
            .filter(|e| e.addr != 0)
            .map(|e| (e.name.clone(), e.addr))
            .collect();
        exports_sorted.sort_by_key(|(_, a)| *a);
        let mut ranges = Vec::new();
        for (idx, (name, addr)) in exports_sorted.iter().enumerate() {
            let next = exports_sorted
                .get(idx + 1)
                .map(|(_, a)| *a)
                .unwrap_or(u64::MAX);
            let mut targets: Vec<i64> = Vec::new();
            for (_target, refs) in &analysis.xrefs.refs_to {
                for r in refs {
                    if r.kind == tpt_armature_analysis::XrefKind::Symbol
                        && r.from >= *addr
                        && r.from < next
                    {
                        targets.push(*_target as i64);
                    }
                }
            }
            targets.sort_unstable();
            targets.dedup();
            ranges.push((name.clone(), *addr, targets));
        }

        let exports: rhai::Array = ranges
            .into_iter()
            .map(|(name, addr, targets)| {
                let target_arr: rhai::Array = targets.into_iter().map(|t| t.into()).collect();
                let mut m = rhai::Map::new();
                m.insert("name".into(), name.into());
                m.insert("addr".into(), (addr as i64).into());
                m.insert("targets".into(), target_arr.into());
                m.into()
            })
            .collect();
        scope.push("exports", exports);

        // Flat symbol xref list: array of { from, to }.
        let symbol_xrefs: rhai::Array = analysis
            .xrefs
            .refs_to
            .iter()
            .flat_map(|(to, refs)| {
                refs.iter()
                    .filter(|r| r.kind == tpt_armature_analysis::XrefKind::Symbol)
                    .map(move |r| {
                        let mut m = rhai::Map::new();
                        m.insert("from".into(), (r.from as i64).into());
                        m.insert("to".into(), (*to as i64).into());
                        m.into()
                    })
            })
            .collect();
        scope.push("symbol_xrefs", symbol_xrefs);

        ScriptHost {
            engine,
            scope,
            renames,
        }
    }

    /// Run a Rhai script against the bound analysis. Returns the renames the
    /// script produced.
    pub fn run(&self, script: &str) -> Result<Renames, crate::ExtError> {
        self.engine
            .run_with_scope(&mut self.scope.clone(), script)
            .map_err(|e| crate::ExtError::Rhai(e.to_string()))?;
        Ok(self.renames.borrow().clone())
    }
}

/// Default example script: auto-rename every exported function that references,
/// via a symbol cross-reference, a symbol whose name contains "printf".
pub fn default_rename_script() -> &'static str {
    r#"
    // Auto-rename functions that call `printf`.
    for ex in exports {
        for t in ex.targets {
            let nm = symbol_name(t);
            if nm.contains("printf") {
                rename(ex.addr, "calls_printf_" + ex.name);
            }
        }
    }
    "#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_script_records_renames() {
        // Construct a minimal analysis via the pipeline on a tiny hand-made input
        // is non-trivial; instead we verify the host builds and runs a trivial
        // script that calls `rename`.
        let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let analysis = tpt_armature_analysis::analyze_binary(&bytes).unwrap();
        let host = ScriptHost::new(&analysis);
        let script = r#"
            rename(0x1234, "hello_from_rhai");
        "#;
        let renames = host.run(script).unwrap();
        assert_eq!(renames.get(&0x1234), Some(&"hello_from_rhai".to_string()));
    }

    #[test]
    fn default_script_runs_without_error() {
        // Guards the symbol_name() lookup fix: the previous version indexed a
        // string-keyed map with an integer and threw at runtime.
        let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let analysis = tpt_armature_analysis::analyze_binary(&bytes).unwrap();
        let host = ScriptHost::new(&analysis);
        let renames = host.run(default_rename_script()).unwrap();
        let _ = renames;
    }
}
