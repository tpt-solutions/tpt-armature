//! Example TPT Armature Wasm plugin (guest).
//!
//! Compile with:
//!   rustup target add wasm32-unknown-unknown
//!   cargo build --release --target wasm32-unknown-unknown
//!
//! Then load the produced `target/wasm32-unknown-unknown/release/tpt-armature-hello-plugin.wasm`
//! from the host (`tpt-armature-ext` `wasm` feature) via `PluginHost::load` + `run`.
//!
//! The host exposes the `tpt-armature` module with three imports:
//!   * `log(ptr: i32, len: i32)`            — write a UTF-8 line to the host log
//!   * `get_instruction_count() -> i64`     — total decoded instructions
//!   * `rename(addr: i64, ptr: i32, len: i32)` — propose a symbol rename

#[link(wasm_import_module = "tpt-armature")]
extern "C" {
    fn log(ptr: *const u8, len: i32);
    fn get_instruction_count() -> i64;
    fn rename(addr: i64, ptr: *const u8, len: i32);
}

fn emit(s: &str) {
    unsafe { log(s.as_ptr(), s.len() as i32) };
}

#[no_mangle]
pub extern "C" fn tpt_armature_run() {
    let count = unsafe { get_instruction_count() };
    emit("hello from tpt-armature plugin; instructions = ");
    emit(&count.to_string());
    emit("\n");
    let name = b"hello_plugin";
    unsafe { rename(0x1000, name.as_ptr(), name.len() as i32) };
}
