# Scripting with Rhai

TPT Armature embeds [Rhai](https://rhai.rs), a tiny Rust-like scripting language,
so you can automate triage without touching the core. Scripts run against a
loaded `Analysis` and can propose symbol renames.

## Running a script

```sh
cargo run -p armature-cli --features rhai -- script <binary> <script.rhai>
```

Example (ships in `crates/armature-ext/scripts/`):

```sh
cargo run -p armature-cli --features rhai -- script target/debug/armature \
    crates/armature-ext/scripts/auto_rename_printf.rhai
```

The CLI prints every rename the script produced as `0x<addr> -> <name>`.

## Bindings available to every script

| Name             | Type            | Meaning                                      |
| ---------------- | --------------- | -------------------------------------------- |
| `format`         | `String`        | Container format (`PE` / `ELF` / `Mach-O`).  |
| `arch`           | `String`        | Architecture (`x86` / `x86_64` / ...).       |
| `entry`          | `i64`           | Entry point virtual address.                |
| `instruction_count` | `i64`        | Total decoded instructions.                  |
| `imports`        | `Array` of `{name, dll}` | Imported symbols.                    |
| `exports`        | `Array` of `{name, addr, targets}` | Exported symbols; `targets` are symbol addresses each export references. |
| `symbol_xrefs`   | `Array` of `{from, to}` | Symbol cross-references (from instruction to symbol address). |
| `symbol_names`   | `Map` (string keys) | Export address (as string) → name.     |

## Native functions

- `rename(addr: i64, name: String)` — propose a symbol rename.
- `symbol_name(addr: i64) -> String` — resolve an export name by address
  (Rhai `Map` keys are strings, so use this instead of `symbol_names[addr]`).

`print(...)` writes a line to stdout (handy for listing/summary scripts).

## Templates

- `auto_rename_printf.rhai` — rename exports that reference `printf`.
- `list_imports.rhai` — list every import.
- `find_crypto.rhai` — flag/rename crypto-looking exports.
- `rename_by_prefix.rhai` — prefix all exports with `tpt_`.
- `summary.rhai` — print a triage summary.

## Wasm plugins (heavier duty)

For sandboxed, language-agnostic plugins, implement the `armature` ABI
(`log`, `get_instruction_count`, `rename`) and compile to
`wasm32-unknown-unknown`. See
`crates/armature-ext/examples/plugins/hello` for a working guest and the
`just build-wasm-example` recipe.
