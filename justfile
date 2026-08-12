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
