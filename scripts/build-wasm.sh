#!/usr/bin/env bash
#
# Builds the three contracts for wasm32v1-none and prints each WASM's path,
# size in bytes and sha256, so local builds can be compared with CI.
#
# Requirements:
#   - cargo with the `wasm32v1-none` target (rustup target add wasm32v1-none)
#   - `sha256sum` or `shasum` (one of them is found on Linux and macOS)
#
# Usage:
#   scripts/build-wasm.sh
#   scripts/build-wasm.sh -h | --help

set -euo pipefail

case "${1:-}" in
  -h | --help)
    sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  "") ;;
  *)
    echo "unknown argument: $1 (try --help)" >&2
    exit 2
    ;;
esac

cd "$(dirname "$0")/.."

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  echo "error: neither sha256sum nor shasum found" >&2
  exit 1
fi

# Portable file size in bytes (GNU and BSD stat differ).
size_bytes() { wc -c <"$1" | tr -d '[:space:]'; }

cargo build --target wasm32v1-none --release \
  -p stellar-stream-hive-vault \
  -p stellar-stream-hive-stream \
  -p stellar-stream-hive-registry

wasm_dir="target/wasm32v1-none/release"

echo
printf '%-60s %10s  %s\n' "PATH" "BYTES" "SHA256"
for name in stellar_stream_hive_vault stellar_stream_hive_stream stellar_stream_hive_registry; do
  wasm="$wasm_dir/$name.wasm"
  if [ ! -f "$wasm" ]; then
    echo "error: $wasm was not produced" >&2
    exit 1
  fi
  printf '%-60s %10s  %s\n' "$wasm" "$(size_bytes "$wasm")" "$(sha256 "$wasm")"
done
