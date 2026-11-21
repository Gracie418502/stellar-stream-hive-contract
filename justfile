# Lists the available recipes.
default:
    @just --list

# Runs every test in the workspace.
test:
    cargo test --workspace

# Runs only tests whose name contains NAME, e.g. test_withdraw_rejects_non_recipient_caller.
test-one NAME:
    cargo test --workspace {{NAME}}

# Formats the code.
fmt:
    cargo fmt --all

# Checks formatting and runs clippy with the same flags as CI.
lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings -A clippy::too_many_arguments -A clippy::needless_borrows_for_generic_args

# Builds rustdoc for the workspace with warnings treated as errors (same as CI's docs job).
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Checks dependencies against the RustSec advisory database (needs cargo-audit).
audit:
    cargo audit

# Builds every contract to the deployment WASM target.
wasm:
    cargo build --target wasm32v1-none --release -p stellar-stream-hive-vault -p stellar-stream-hive-stream -p stellar-stream-hive-registry

# Builds the contracts and prints the size in bytes of each WASM artifact.
wasm-size: wasm
    #!/usr/bin/env bash
    set -euo pipefail
    for f in stellar_stream_hive_vault stellar_stream_hive_stream stellar_stream_hive_registry; do
        printf '%10s bytes  %s.wasm\n' "$(stat -c%s "target/wasm32v1-none/release/$f.wasm")" "$f"
    done

# Runs everything CI runs.
check: lint test wasm
