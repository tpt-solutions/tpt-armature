# tpt-armature-hello-plugin

Example **TPT Armature** Wasm plugin guest. It demonstrates the `tpt-armature`
plugin ABI consumed by [`tpt-armature-ext`](../)'s `wasm` feature.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

The produced
`target/wasm32-unknown-unknown/release/tpt_armature_hello_plugin.wasm`
can be loaded by the host:

```sh
tpt-armature plugin path/to/binary \
  target/wasm32-unknown-unknown/release/tpt_armature_hello_plugin.wasm
```

## ABI

The guest imports three host functions from the `tpt-armature` module:

- `log(ptr: i32, len: i32)` — write a UTF-8 line to the host log.
- `get_instruction_count() -> i64` — total decoded instructions.
- `rename(addr: i64, ptr: i32, len: i32)` — propose a symbol rename.

and exports `tpt_armature_run` as its entry point (falling back to `_start`).

## License

Licensed under either of [MIT](https://opensource.org/licenses/MIT) or
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
See the repository `LICENSE-MIT` / `LICENSE-APACHE` files.
