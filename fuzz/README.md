# Fuzzing the streaming math

`fuzz/fuzz_targets/streamed_math.rs` runs `streamed_amount` and
`withdrawable_amount` from `contracts/stream/src/math.rs` on arbitrary inputs
and checks the invariants listed at the top of that file.

This crate is **not** part of the Cargo workspace, and it is **not** run by
the required CI. It is a local tool for hunting edge cases.

## Run it locally

Fuzzing needs `cargo-fuzz` and a nightly toolchain (for sanitizer flags):

```sh
cargo install cargo-fuzz --locked
rustup toolchain install nightly

# Run from the repository root. Stop after 60 seconds.
cargo +nightly fuzz run --fuzz-dir fuzz streamed_math -- -max_total_time=60
```

If the fuzzer finds a failing input, it writes it to `fuzz/artifacts/` and
prints the path. Reproduce it with:

```sh
cargo +nightly fuzz run --fuzz-dir fuzz streamed_math fuzz/artifacts/streamed_math/<file>
```

Add a regression unit test in `contracts/stream/src/math.rs` for any input you
fix. Do not commit `fuzz/artifacts/` or `fuzz/corpus/`; both are ignored.
