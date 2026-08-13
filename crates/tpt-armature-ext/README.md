# tpt-armature-ext

**TPT Armature — Layer 4 (Extension): "The Custom Gears".**

Community extensibility without touching the core Rust code:

- **Rhai scripting** (`rhai` feature) — a tiny Rust-like language for quick
  automation (rename functions, mine imports, summarize).
- **Sandboxed WebAssembly plugins** (`wasm` feature) — heavy-duty extensions
  compiled to `wasm32` and run inside a `wasmtime` sandbox.

Both backends are feature-gated so the default workspace build stays light.

## Rhai scripting

```rust
use tpt_armature_ext::ScriptHost;
use tpt_armature_analysis::analyze_binary;

let bytes = std::fs::read("target/release/tpt-armature")?;
let analysis = analyze_binary(&bytes)?;
let host = ScriptHost::new(&analysis);
let renames = host.run("rename(0x1000, \"entry\");")?;
```

Bindings available to every script: `format`, `arch`, `entry`,
`instruction_count`, `function_count`, `imports`, `exports`, `symbol_xrefs`,
`symbol_names`, plus the native `rename(addr, name)` and
`symbol_name(addr)` functions. Ready-made scripts live in
[`scripts/`](./scripts).

## Wasm plugins

Guests implement the `tpt-armature` ABI (`log`, `get_instruction_count`,
`rename`) and export `tpt_armature_run`. Host with `PluginHost::load` + `run`:

```rust
use tpt_armature_ext::PluginHost;
use tpt_armature_analysis::analyze_binary;

let bytes = std::fs::read("target/release/tpt-armature")?;
let analysis = analyze_binary(&bytes)?;
let mut host = PluginHost::load("plugin.wasm")?;
host.bind_analysis(&analysis);
let output = host.run()?;
println!("renames: {:?}", output.renames);
```

A working guest lives in
[`examples/plugins/tpt-armature-hello-plugin`](./examples/plugins/tpt-armature-hello-plugin).

## Feature flags

| Flag | Enables |
| ---- | ------- |
| `rhai` | Rhai scripting host (`ScriptHost`, `default_rename_script`). |
| `wasm` | wasmtime sandboxed plugin host (`PluginHost`, `PluginApi`, `PluginOutput`). |

## Minimum supported Rust version

1.85 (edition 2021).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
