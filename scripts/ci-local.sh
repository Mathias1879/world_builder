#!/usr/bin/env bash
# Local replacement for GitHub Actions: everything check.sh does, plus the
# x86-64 run of every test (incl. the golden determinism hash) in an amd64
# Linux container via Apple Container. Cleans up build output and the image.
#
# Requires: wasmtime, Apple Container (`container`). Usage: scripts/ci-local.sh [--skip-x86]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

"$ROOT/scripts/check.sh"

if [[ "${1:-}" == "--skip-x86" ]]; then
  echo "ci-local: skipped x86-64 container run"
  exit 0
fi

TARGET=x86_64-unknown-linux-musl
IMAGE=alpine:3.20
# Scratch + x86 build cache live under cargo's target dir (honours machine-level target-dir config).
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
WORK="$TARGET_DIR/ci-local"
rm -rf "$WORK/bins" && mkdir -p "$WORK"
STARTED_SERVICE=0
cleanup() {
  rm -rf "$WORK/bins"   # keep $WORK/target as an incremental x86 build cache
  container image delete "$IMAGE" >/dev/null 2>&1 || true
  if [[ $STARTED_SERVICE -eq 1 ]]; then container system stop >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT

rustup target add "$TARGET" >/dev/null
CARGO_TARGET_DIR="$WORK/target" CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
  cargo test --workspace --no-run --target "$TARGET" --message-format=json 2>/dev/null \
  | sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' > "$WORK/bins.txt"
mkdir -p "$WORK/bins"
while read -r bin; do cp "$bin" "$WORK/bins/"; done < "$WORK/bins.txt"
echo "ci-local: built $(wc -l < "$WORK/bins.txt" | tr -d ' ') x86-64 test binaries"

if ! container system status >/dev/null 2>&1; then
  container system start --enable-kernel-install >/dev/null
  STARTED_SERVICE=1
fi

container run --rm --arch amd64 -v "$WORK/bins":/t "$IMAGE" sh -c '
  [ "$(uname -m)" = x86_64 ] || { echo "not x86_64"; exit 1; }
  fail=0
  for f in /t/*; do
    out=$("$f" 2>&1) || { echo "FAILED: $(basename "$f")"; echo "$out" | tail -20; fail=1; }
  done
  exit $fail'
echo "ci-local: x86-64 tests passed"
