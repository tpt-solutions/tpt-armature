# tpt-armature-cli

**TPT Armature — Layer 5 (Driver): "Headless Pipeline".**

The command-line front-end that wires every layer together: ingestion → IR →
disassembly → analysis → (optional) presentation. Ships the `tpt-armature`
binary.

## Install / build

```sh
cargo build --release -p tpt-armature-cli
# or the whole workspace
cargo build --workspace
```

## Commands

| Command | Purpose |
| ------- | ------- |
| `analyze` | High-level summary (format, arch, sections, CFG, xrefs, debug syms). |
| `disasm` | Print assembly text (`-n` limits instructions). |
| `cfg` | Print CFG statistics and edges. |
| `strings` | Extract printable strings and immediate constants. |
| `decompile` | Render a C-like pseudocode view of recovered functions. |
| `script` | Run a Rhai automation script (`--features rhai`). |
| `plugin` / `plugins` | Run one or all sandboxed Wasm plugins (`--features wasm`). |
| `watch` | Re-analyze a binary whenever it changes on disk. |

`analyze` also accepts `--json` (CI-friendly summary) and, with
`--features debuginfo`, `--pdb <file.pdb>` to merge PDB public symbols.

## Examples

```sh
tpt-armature analyze path/to/binary
tpt-armature disasm  path/to/binary -n 20
tpt-armature decompile path/to/binary
tpt-armature strings path/to/binary
tpt-armature script path/to/binary crates/tpt-armature-ext/scripts/summary.rhai
```

`just demo` builds the CLI and analyzes the freshly built binary itself.

## Feature flags

| Flag | Crate | Enables |
| ---- | ----- | ------- |
| `arm` | `tpt-armature-cli`, `tpt-armature-disasm` | ARM/AArch64 disassembly. |
| `rhai` | `tpt-armature-cli`, `tpt-armature-ext` | Rhai scripting + `script` subcommand. |
| `wasm` | `tpt-armature-cli`, `tpt-armature-ext` | Sandboxed Wasm plugins + `plugin`/`plugins`. |
| `debuginfo` | `tpt-armature-cli`, `tpt-armature-formats` | ELF `.symtab` + PE PDB symbol import. |

## Minimum supported Rust version

1.85 (edition 2021).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
