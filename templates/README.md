# Templates & Samples

Reusable starting points for working with TPT Armature.

## `first_analysis.rhai` — your first automation script

A minimal Rhai script that walks every recovered function and renames any
function that calls `printf` (or any other imported symbol you choose) to
`print_*`. Run it with:

```sh
cargo run -p tpt-armature-cli --features rhai -- script \
    <binary> templates/first_analysis.rhai
```

It prints the produced renames and also writes them back via the host's
`rename()` call so downstream tooling (and the GUI) sees the new names.

## `rhai_cheatsheet.md` — Rhai API reference

A concise cheat-sheet of the bindings the [`tpt-armature-ext`](../crates/tpt-armature-ext)
scripting host injects into every Rhai session. Copy snippets into your own
scripts.

## Annotated sample binary

The minimal, annotated sample binary lives in
[`examples/tpt-armature-sample`](../examples/tpt-armature-sample). It is a tiny Rust program with a few
named functions and deliberate control flow so the disassembler, CFG builder,
string extractor, and decompiler all have something interesting to show. Build
it with `just build-samples` (or `cargo build --release --manifest-path
examples/tpt-armature-sample/Cargo.toml`) and analyze it with `just qa`.
