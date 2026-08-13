# tpt-armature-analysis

**TPT Armature — Layer 2 (Analysis): "Exposing the Framework".**

Consumes the IR produced by [`tpt_armature_disasm`] and builds the mathematical
structures analysts use: a control-flow graph, a cross-reference index, and a
lightweight data-flow summary. It also exposes the top-level `analyze_binary`
pipeline entry point used by the CLI and GUI.

## Getting started

```rust
use tpt_armature_analysis::analyze_binary;

let bytes = std::fs::read("target/release/tpt-armature")?;
let analysis = analyze_binary(&bytes)?;
println!("functions: {}", analysis.module.functions.len());
println!("xrefs: {}", analysis.xrefs.refs_to.len());
```

## Key APIs

| Function | Purpose |
| -------- | ------- |
| `analyze_binary` / `analyze_map` | One-shot pipeline entry points. |
| `build_cfg` | Control-flow graph over a `Module` (`Cfg`, `Edge`, `EdgeKind`). |
| `build_xrefs` | Cross-reference index (code ↔ symbols; `Xref`, `XrefIndex`, `XrefKind`). |
| `analyze` (data-flow) | Reaching / used register & constant summary (`DataFlow`). |
| `extract_strings` / `extract_constants` | String & immediate recovery (`ExtractedString`, `StringKind`). |
| `decompile_function` / `decompile_module` | C-like pseudocode rendering. |
| `recover_functions` | Recursive-descent function recovery (re-exported from the IR). |
| `analysis_to_json` / `export_renames` | Serialization & rename export (`RenameFormat`). |

## Feature flags

None beyond the workspace defaults. Depends on `tpt-armature-ir`,
`tpt-armature-formats`, and `tpt-armature-disasm`.

## Minimum supported Rust version

1.85 (edition 2021).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
