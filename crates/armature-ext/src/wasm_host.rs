//! Sandboxed WebAssembly plugin host (feature `wasm`).
//!
//! Plugins are compiled to wasm32 and instantiate inside a [`wasmtime`]
//! sandbox. The guest ABI is the `armature` module, which exposes the host
//! functions [`PluginApi`] documents. A plugin's entry point is `armature_run`
//! (falling back to `_start`).

use std::path::Path;
use std::sync::Mutex;

use armature_analysis::Analysis;

/// The guest ABI: host functions exposed to plugin Wasm modules.
///
/// * `armature::log(ptr: i32, len: i32)` — emit a log line from guest memory.
/// * `armature::get_instruction_count() -> i64` — total decoded instructions.
/// * `armature::rename(addr: i64, ptr: i32, len: i32)` — propose a symbol rename.
pub struct PluginApi;

#[derive(Default)]
struct HostState {
    log: Mutex<Vec<String>>,
    instruction_count: i64,
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
        let engine = wasmtime::Engine::new(&config)
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;
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

    /// Instantiate and run the plugin, returning the log lines it emitted.
    pub fn run(&mut self) -> Result<Vec<String>, crate::ExtError> {
        let mut store = wasmtime::Store::new(&self.engine, HostState::default());
        store.data_mut().instruction_count = self.state.instruction_count;

        let mut linker = wasmtime::Linker::new(&self.engine);

        linker
            .func_wrap(
                "armature",
                "log",
                |mut caller: wasmtime::Caller<'_, HostState>, ptr: i32, len: i32| {
                    if let Some(mem) = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                    {
                        let start = ptr as usize;
                        let end = start + len as usize;
                        if let Ok(data) = mem.read(&caller, start, end - start) {
                            let line = String::from_utf8_lossy(data).to_string();
                            caller.data().log.lock().unwrap().push(line);
                        }
                    }
                },
            )
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        linker
            .func_wrap(
                "armature",
                "get_instruction_count",
                |caller: wasmtime::Caller<'_, HostState>| -> i64 {
                    caller.data().instruction_count
                },
            )
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        linker
            .func_wrap(
                "armature",
                "rename",
                |caller: wasmtime::Caller<'_, HostState>, _addr: i64, _ptr: i32, _len: i32| {
                    let _ = caller.data();
                },
            )
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;

        for entry in ["armature_run", "_start"] {
            if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, entry) {
                func.call(&mut store, ())
                    .map_err(|e| crate::ExtError::Wasm(e.to_string()))?;
                break;
            }
        }

        Ok(store.into_data().log.into_inner().unwrap())
    }
}
