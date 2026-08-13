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
    cargo build -p armature-cli
    cargo run -p armature-cli -- analyze target/debug/armature
    cargo run -p armature-cli -- disasm target/debug/armature -n 20

# Headless smoke test: analyze the built binary end-to-end.
smoke:
    cargo build -p armature-cli
    cargo run -p armature-cli -- analyze target/debug/armature

# Run a Rhai automation script against the built binary (requires `rhai`).
script SCRIPT="crates/armature-ext/scripts/summary.rhai":
    cargo build -p armature-cli --features rhai
    cargo run -p armature-cli --features rhai -- script target/debug/armature {{SCRIPT}}

# Build the example Wasm plugin (requires: rustup target add wasm32-unknown-unknown).
build-wasm-example:
    cargo build --release --target wasm32-unknown-unknown \
        --manifest-path crates/armature-ext/examples/plugins/hello/Cargo.toml

# Build the GUI for the browser (wasm32). Mounts into the #armature_canvas element.
build-wasm-gui:
    rustup target add wasm32-unknown-unknown
    cargo build --release --target wasm32-unknown-unknown -p armature-gui --features scripts

# Build the manual-QA sample binary for the host platform (see examples/samples).
build-samples:
    cargo build --release --manifest-path examples/samples/Cargo.toml

# Quick manual-QA pass: analyze the sample binary end-to-end.
qa:
    just build-samples
    $bin = if ($IsWindows) { "examples/samples/target/release/armature-sample.exe" } else { "examples/samples/target/release/armature-sample" }
    cargo run -p armature-cli -- analyze $bin
    cargo run -p armature-cli -- disasm $bin -n 12
