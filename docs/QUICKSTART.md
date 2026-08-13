# 5-Minute Quickstart

A whirlwind tour of TPT Armature using the bundled sample binary. After this
you will have loaded a binary, read its disassembly, browsed its control-flow
graph, and produced a pseudocode view — all from the command line.

## 0. Build

```sh
git clone https://github.com/TPT-Solutions/tpt-armature
cd tpt-armature
cargo build --workspace
```

## 1. Build the sample

The repo ships a small, deliberately-structured sample you can analyze without
supplying your own target:

```sh
just build-samples
# produces examples/tpt-armature-sample/target/release/tpt-armature-sample  (+ .exe on Windows)
```

## 2. Analyze it

```sh
just qa
```

`just qa` runs two commands. The first prints:

```
== TPT Armature :: Analysis ==
format       : ELF            (or PE / Mach-O on your platform)
architecture : x86_64
entry point  : 0x401000
base address : 0x400000
sections     : 4
imports      : 2
exports      : 1
code section : .text (2048 bytes)
instructions : 128 (showing 128)
functions    : 3
CFG: 18 blocks, 22 edges, 1 loop(s)
xrefs        : 14
registers    : ...
```

(The exact addresses, counts, and format/architecture depend on your platform;
the shape of the output is what matters.)

## 3. Disassemble a window

```sh
cargo run -p tpt-armature-cli -- disasm examples/tpt-armature-sample/target/release/tpt-armature-sample -n 12
```

Prints address, raw bytes, and assembly text for the first 12 instructions.

## 4. Read the strings and constants

```sh
cargo run -p tpt-armature-cli -- strings examples/tpt-armature-sample/target/release/tpt-armature-sample
```

Lists printable ASCII/UTF-16 strings found in the image (format strings,
URLs, …) and their virtual addresses.

## 5. Pseudocode view

```sh
cargo run -p tpt-armature-cli -- decompile examples/tpt-armature-sample/target/release/tpt-armature-sample
```

Emits a C-like rendering of every recovered function:

```
// compute @ 0x401040
compute() {
  rax = 1;
  rax = rax + 2;
  fn_401000();
  return;
}
```

## 6. Explore interactively (GUI)

```sh
cargo run -p tpt-armature-gui --features app -- examples/tpt-armature-sample/target/release/tpt-armature-sample
```

The GUI tiles three views (Hex / Assembly / Graph) plus a right-hand info
panel with **Strings** and **Pseudocode** tabs. Use the **goto** box to jump to
an address, the **search** box to find instructions, and the arrow keys to step
through the Assembly view.

## 7. Script it (optional)

```sh
cargo run -p tpt-armature-cli --features rhai -- script \
    examples/tpt-armature-sample/target/release/tpt-armature-sample \
    crates/tpt-armature-ext/scripts/auto_rename_printf.rhai
```

Automation scripts (Rhai) and sandboxed Wasm plugins let you rename
functions, mine for patterns, and extend Armature without touching the core.

## Where to go next

- [GETTING_STARTED.md](./GETTING_STARTED.md) — full command reference.
- [SCRIPTING.md](./SCRIPTING.md) — write your own Rhai automation.
- [`templates/`](../templates) — Rhai cheat-sheet and a first-analysis script.
- `spec.txt` — the System Design Document.
