# tpt-armature-sample

A small, deliberately-structured sample binary shipped for **manual QA** of the
TPT Armature analyzer. It has a few named functions and explicit control flow so
the disassembler, CFG builder, string extractor, and decompiler all have
something interesting to show.

## Build

```sh
just build-samples
# or directly
cargo build --release --manifest-path examples/tpt-armature-sample/Cargo.toml
```

Produces `examples/tpt-armature-sample/target/release/tpt-armature-sample`
(`+ .exe` on Windows).

## Analyze

```sh
tpt-armature analyze examples/tpt-armature-sample/target/release/tpt-armature-sample
tpt-armature disasm  examples/tpt-armature-sample/target/release/tpt-armature-sample -n 12
tpt-armature decompile examples/tpt-armature-sample/target/release/tpt-armature-sample
```

`just qa` runs the quick analysis pass end-to-end. `just quickstart` runs the
guided tour from [`docs/QUICKSTART.md`](../../docs/QUICKSTART.md).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
