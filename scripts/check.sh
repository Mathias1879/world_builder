#!/usr/bin/env bash
# Full local gate: formatting, lints, native tests, WASM tests.
set -euo pipefail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --target wasm32-wasip1
cargo test --release -p wb-editlog --test perf
cargo test --release -p wb-editlog --test perf --target wasm32-wasip1
