# Rhai Cheat-Sheet

The [`tpt-armature-ext`](../crates/tpt-armature-ext) scripting host injects a fixed set
of bindings into every Rhai session. This cheat-sheet is the canonical
reference — copy snippets into your own scripts.

## Run a script

```sh
cargo run -p tpt-armature-cli --features rhai -- script <binary> my_script.rhai
```

The host prints produced renames to stdout (and the GUI applies them to the
function labels in the Graph view).

## Scalar bindings

| Name                | Type   | Meaning                              |
| ------------------- | ------ | ------------------------------------ |
| `format`            | string | Container format (`PE`/`ELF`/`Mach-O`) |
| `arch`              | string | Architecture (`x86_64`, …)           |
| `entry`             | int    | Entry-point virtual address         |
| `instruction_count` | int    | Total decoded instructions          |
| `function_count`    | int    | Recovered functions                 |

## Collections

| Name          | Shape                                          |
| ------------- | ---------------------------------------------- |
| `imports`     | array of `#{ name: string, dll: string }`      |
| `exports`     | array of `#{ name, addr, targets: int[] }`     |
| `symbol_xrefs`| array of `#{ from: int, to: int }`             |

- `exports[i].targets` — symbol cross-reference target addresses reachable
  from that export's body.
- `symbol_xrefs` — flat list of every symbol reference (`from` calls/refs `to`).

## Functions

| Call                 | Returns  | Purpose                                  |
| -------------------- | -------- | ---------------------------------------- |
| `symbol_name(addr)`  | string   | Resolve a symbol address to its name.    |
| `rename(addr, name)` | —        | Record a rename (surfaced by CLI/GUI).   |

`print(...)` (Rhai built-in) logs to stdout / the GUI console.

## Idioms

```rhai
// Iterate exports and their symbol xref targets.
for ex in exports {
    for t in ex.targets {
        let nm = symbol_name(t);
        if nm.contains("printf") {
            rename(ex.addr, "calls_printf_" + ex.name);
        }
    }
}

// Find which functions reference a given imported symbol.
let want = "memcpy";
for sx in symbol_xrefs {
    if symbol_name(sx.to).contains(want) {
        rename(sx.from, "uses_" + want);
    }
}

// Gate on a minimum size or count.
if function_count > 100 {
    print("large binary: " + function_count + " functions");
}
```

## Notes

- Addresses are `int` (Rhai is 64-bit aware); cast with `addr as int` if needed.
- There is no live `Analysis` object exposed — only the flattened bindings
  above. Request additional bindings by extending `ScriptHost::new` in
  `crates/tpt-armature-ext/src/rhai_host.rs`.
