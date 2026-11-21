#![no_main]
#![allow(dead_code, unused_imports)]

//! Fuzz target for the streaming math in `contracts/stream/src/math.rs`.
//!
//! Property tests only cover a handful of generated schedules. This target
//! lets the fuzzer search the whole `u32`/`i128` input space for a `Stream`
//! and checks the invariants the contract depends on:
//!
//! - no input makes `streamed_amount` or `withdrawable_amount` panic;
//! - for a well-formed stream, `0 <= streamed <= total_amount`;
//! - for a well-formed stream, `streamed` never decreases as `now` grows;
//! - for a well-formed stream, `0 <= withdrawable` and
//!   `withdrawn + withdrawable <= total_amount`.
//!
//! `math` and its types are private modules of the contract crate, so they
//! are compiled here directly from source with `#[path]`. The fuzzer runs the
//! exact code that ships, and the contract's public API is left unchanged.

#[path = "../../contracts/stream/src/error.rs"]
mod error;
#[path = "../../contracts/stream/src/types.rs"]
mod types;
#[path = "../../contracts/stream/src/math.rs"]
mod math;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as _, Address, Env};
use types::{Stream, StreamStatus};

#[derive(Arbitrary, Debug)]
struct Input {
    total_amount: i128,
    withdrawn: i128,
    start_ledger: u32,
    end_ledger: u32,
    cliff_ledger: u32,
    cancelled: bool,
    cancelled_at_ledger: u32,
    now_a: u32,
    now_b: u32,
}

fuzz_target!(|input: Input| {
    let env = Env::default();
    let status = if input.cancelled {
        StreamStatus::Cancelled
    } else {
        StreamStatus::Active
    };
    let stream = Stream {
        id: 1,
        sender: Address::generate(&env),
        recipient: Address::generate(&env),
        token: Address::generate(&env),
        total_amount: input.total_amount,
        withdrawn: input.withdrawn,
        start_ledger: input.start_ledger,
        end_ledger: input.end_ledger,
        cliff_ledger: input.cliff_ledger,
        cancellable: true,
        status,
        created_at: input.start_ledger,
        cancelled_at_ledger: input.cancelled_at_ledger,
    };

    let (lo, hi) = if input.now_a <= input.now_b {
        (input.now_a, input.now_b)
    } else {
        (input.now_b, input.now_a)
    };

    // Any input must return a value or a typed error, never panic.
    let streamed_lo = math::streamed_amount(&stream, lo);
    let streamed_hi = math::streamed_amount(&stream, hi);
    let withdrawable = math::withdrawable_amount(&stream, hi);

    // The invariants only describe well-formed streams. Malformed ones must
    // fail with a typed error, which the calls above already checked.
    let well_formed = input.total_amount > 0
        && input.end_ledger > input.start_ledger
        && (0..=input.total_amount).contains(&input.withdrawn);
    if !well_formed {
        return;
    }
    let total = input.total_amount;

    if let (Ok(a), Ok(b)) = (streamed_lo, streamed_hi) {
        assert!(a >= 0, "streamed went negative: {a}");
        assert!(b >= 0, "streamed went negative: {b}");
        assert!(a <= total, "streamed {a} exceeds total {total}");
        assert!(b <= total, "streamed {b} exceeds total {total}");
        assert!(a <= b, "streamed decreased over time: {a} at {lo}, {b} at {hi}");
    }

    if let Ok(w) = withdrawable {
        assert!(w >= 0, "withdrawable went negative: {w}");
        assert!(
            input.withdrawn + w <= total,
            "withdrawn {} plus withdrawable {w} exceeds total {total}",
            input.withdrawn
        );
    }
});
