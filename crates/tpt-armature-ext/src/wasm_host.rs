//! Sandboxed WebAssembly plugin host (feature `wasm`).
//!
//! Plugins are compiled to wasm32 and instantiate inside a [`wasmtime`]
//! sandbox. The guest ABI is the `tpt-armature` module, which exposes the host
//! functions [`PluginApi`] documents. A plugin's entry point is `tpt_armature_run`
//! (falling back to `_start`). Renames and log lines the guest produces are
//! returned in [`PluginOutput`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use tpt_armature_analysis::Analysis;

/// The guest ABI: host functions exposed to plugin Wasm modules.
///
/// * `tpt-armature::log(ptr: i32, len: i32)` — emit a log line from guest memory.
/// * `tpt-armature::get_instruction_count() -> i64` — total decoded instructions.
/// * `tpt-armature::rename(addr: i64, ptr: i32, len: i32)` — propose a symbol rename.
///   `ptr`/`len` describe a UTF-8 name in the guest's exported `memory`.
pub struct PluginApi;

/// What a plugin run produced: log lines and proposed renames.
#[derive(Debug, Default, Clone)]
pub struct PluginOutput {
    /// Lines the plugin emitted via `tpt-armature::log`.
    pub logs: Vec<String>,
    /// Address -> proposed name, from `tpt-armature::rename`.
    pub renames: HashMap<i64, String>,
}

#[derive(Default)]
struct HostState {
    log: Mutex<Vec<String>>,
    instruction_count: i64,
    renames: Mutex<HashMap<i64, String>>,
}

/// A sandboxed plugin loaded from a wasm file.
pub struct PluginHost {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    state: HostState,
}

impl PluginHost {
    /// Load a plugin module from disk.
    pub fn load(path: &Path) -> Result<Self, crate::ExtError> {
        let mut config = wasmtime::Config::new();
        config.wasm_multi_memory(false);
        let engine =
            wasmtime::Engine::new(&config).map_err(|e| crate::ExtError::Wasm(e.to_string()))?;
        let module = wasmtime::Module::from_file(&engine, path)
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;
        Ok(PluginHost {
            engine,
            module,
            state: HostState::default(),
        })
    }

    /// Bind the current analysis so the plugin can query it.
    pub fn bind_analysis(&mut self, analysis: &Analysis) {
        self.state.instruction_count = analysis.instructions.len() as i64;
    }

    /// Instantiate and run the plugin, returning the log lines and renames it
    /// emitted.
    pub fn run(&mut self) -> Result<PluginOutput, crate::ExtError> {
        let mut store = wasmtime::Store::new(&self.engine, HostState::default());
        store.data_mut().instruction_count = self.state.instruction_count;

        let mut linker = wasmtime::Linker::new(&self.engine);

        linker
            .func_wrap(
                "tpt-armature",
                "log",
                |mut caller: wasmtime::Caller<'_, HostState>, ptr: i32, len: i32| {
                    if let Some(mem) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                        let start = ptr.max(0) as usize;
                        let end = start + len.max(0) as usize;
                        let mut buf = vec![0u8; end.saturating_sub(start).max(1)];
                        if mem.read(&caller, start, &mut buf).is_ok() {
                            caller
                                .data()
                                .log
                                .lock()
                                .unwrap()
                                .push(String::from_utf8_lossy(&buf).to_string());
                        }
                    }
                },
            )
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        linker
            .func_wrap(
                "tpt-armature",
                "get_instruction_count",
                |caller: wasmtime::Caller<'_, HostState>| -> i64 {
                    caller.data().instruction_count
                },
            )
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        linker
            .func_wrap(
                "tpt-armature",
                "rename",
                |mut caller: wasmtime::Caller<'_, HostState>, addr: i64, ptr: i32, len: i32| {
                    if let Some(mem) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                        let start = ptr.max(0) as usize;
                        let end = start + len.max(0) as usize;
                        let mut buf = vec![0u8; end.saturating_sub(start).max(1)];
                        if mem.read(&caller, start, &mut buf).is_ok() {
                            caller
                                .data()
                                .renames
                                .lock()
                                .unwrap()
                                .insert(addr, String::from_utf8_lossy(&buf).to_string());
                        }
                    }
                },
            )
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        for entry in ["tpt_armature_run", "_start"] {
            if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, entry) {
                func.call(&mut store, ())
                    .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;
                break;
            }
        }

        let data = store.into_data();
        Ok(PluginOutput {
            logs: data.log.into_inner().unwrap(),
            renames: data.renames.into_inner().unwrap(),
        })
    }
}
