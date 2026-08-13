# Task recipes for TPT Armature.
# Run with `just <recipe>` (https://github.com/casey/just).

set shell := ["pwsh", "-NoProfile", "-Command"]

# List available recipes.
default:
    @just --list

# Format all code.
fmt:
    cargo fmt --all

# Verify formatting (CI gate).
fmt-check:
    cargo fmt --all -- --check

# Lint with clippy.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run the test suite.
test:
    cargo test --workspace

# Build the default workspace.
build:
    cargo build --workspace

# Build all features (heavier; pulls in ARM, GUI, Rhai, wasm).
build-all:
    cargo build --workspace --all-features

# Analyze the freshly built CLI binary (no external sample required).
# Great for a first-run demo and for CI smoke testing.
demo:
    cargo build -p tpt-armature-cli
    cargo run -p tpt-armature-cli -- analyze target/debug/tpt-armature
    cargo run -p tpt-armature-cli -- disasm target/debug/tpt-armature -n 20

# Headless smoke test: analyze the built binary end-to-end.
smoke:
    cargo build -p tpt-armature-cli
    cargo run -p tpt-armature-cli -- analyze target/debug/tpt-armature

# Run a Rhai automation script against the built binary (requires `rhai`).
script SCRIPT="crates/tpt-armature-ext/scripts/summary.rhai":
    cargo build -p tpt-armature-cli --features rhai
    cargo run -p tpt-armature-cli --features rhai -- script target/debug/tpt-armature {{SCRIPT}}

# Build the example Wasm plugin (requires: rustup target add wasm32-unknown-unknown).
build-wasm-example:
    cargo build --release --target wasm32-unknown-unknown \
        --manifest-path crates/tpt-armature-ext/examples/plugins/tpt-armature-hello-plugin/Cargo.toml

# Build + run the example Wasm plugin against the freshly built CLI binary.
# Demonstrates the `tpt-armature` plugin ABI end-to-end (needs the `wasm` feature).
run-wasm:
    just build-wasm-example
    cargo build -p tpt-armature-cli --features wasm
    cargo run -p tpt-armature-cli --features wasm -- plugin target/debug/tpt-armature \
        crates/tpt-armature-ext/examples/plugins/tpt-armature-hello-plugin/target/wasm32-unknown-unknown/release/tpt_armature_hello_plugin.wasm

# Build the GUI for the browser (wasm32). Mounts into the #armature_canvas element.
build-wasm-gui:
    rustup target add wasm32-unknown-unknown
    cargo build --release --target wasm32-unknown-unknown -p tpt-armature-gui --features scripts

# Build the manual-QA sample binary for the host platform (see examples/tpt-armature-sample).
build-samples:
    cargo build --release --manifest-path examples/tpt-armature-sample/Cargo.toml

# Quick manual-QA pass: analyze the sample binary end-to-end.
qa:
    just build-samples
    $bin = if ($IsWindows) { "examples/tpt-armature-sample/target/release/tpt-armature-sample.exe" } else { "examples/tpt-armature-sample/target/release/tpt-armature-sample" }
    cargo run -p tpt-armature-cli -- analyze $bin
    cargo run -p tpt-armature-cli -- disasm $bin -n 12

# Guided 5-minute tour against the sample binary (see docs/QUICKSTART.md).
quickstart:
    just build-samples
    $bin = if ($IsWindows) { "examples/tpt-armature-sample/target/release/tpt-armature-sample.exe" } else { "examples/tpt-armature-sample/target/release/tpt-armature-sample" }
    cargo run -p tpt-armature-cli -- analyze $bin
    cargo run -p tpt-armature-cli -- strings $bin
    cargo run -p tpt-armature-cli -- decompile $bin

# Decompile a binary to a C-like pseudocode view (first N functions, 0 = all).
decompile BINARY="target/debug/tpt-armature" N="0":
    cargo run -p tpt-armature-cli -- decompile {{BINARY}} -n {{N}}

# Extract strings and immediate constants from a binary.
strings BINARY="target/debug/tpt-armature":
    cargo run -p tpt-armature-cli -- strings {{BINARY}}

# Re-analyze a binary whenever it changes on disk (polls every second).
watch BINARY="target/debug/tpt-armature":
    cargo run -p tpt-armature-cli -- watch {{BINARY}} -i 1
