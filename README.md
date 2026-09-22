# World Builder

New coding project — scope and stack to be defined.

## Development

Requires `rustup` and `wasmtime` (`brew install wasmtime`). The toolchain is pinned in `rust-toolchain.toml`.

    scripts/check.sh    # fmt, clippy, native tests, wasm32-wasip1 tests
    scripts/ci-local.sh # check.sh + x86-64 tests in an amd64 container (Apple Container); replaces GitHub Actions

GitHub Actions is manual-only (`workflow_dispatch`) until alpha/beta; run `scripts/ci-local.sh` before merging to main.
