//! # Stellar Stream Hive — Stream Contract
//!
//! The core of the system: linear, per-ledger token streaming. A stream
//! moves `total_amount` of a token from `sender` to `recipient` evenly
//! between `start_ledger` and `end_ledger`. At any point the recipient can
//! withdraw whatever has accrued so far (after `cliff_ledger`) and not yet
//! been withdrawn.
//!
//! This contract never custodies funds itself — that's the vault's job
//! (see `stellar-stream-hive-vault`). This contract only tracks stream
//! metadata and instructs the vault to move money via `deposit` /
//! `release` / `refund`.
#![no_std]

mod error;
mod events;
mod math;
mod storage;
mod test;
mod types;
mod vault_client;

use error::StreamError;
use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

/// Upper bound on the page size accepted by the paginated queries, so a
/// single call can never be forced to hydrate an unbounded number of streams.
pub const MAX_PAGE_SIZE: u32 = 50;
use types::{Stream, StreamStatus};

#[contract]
pub struct StreamContract;

#[contractimpl]
impl StreamContract {
    /// One-time setup. Records the admin and the vault that will custody
    /// every stream created through this contract.
    ///
    /// # Errors
    ///
    /// - `StreamError::AlreadyInitialized`: the contract has already been initialized.
    pub fn initialize(env: Env, admin: Address, vault: Address) -> Result<(), StreamError> {
        if storage::has_admin(&env) {
            return Err(StreamError::AlreadyInitialized);
        }
        admin.require_auth();
        storage::set_admin(&env, &admin);
        storage::set_vault(&env, &vault);
        Ok(())
    }

    /// Creates a new stream and pulls `total_amount` of `token` from
    /// `sender` into the vault. Returns the new stream's id.
    ///
    /// # Errors
    ///
    /// - `StreamError::Paused`: the contract is paused (checked first).
    /// - `StreamError::InvalidAmount`: `total_amount` is zero or negative.
    /// - `StreamError::InvalidTimeRange`: `end_ledger` is not strictly after `start_ledger`.
    /// - `StreamError::InvalidCliff`: `cliff_ledger` is outside `[start_ledger, end_ledger]`.
    /// - `StreamError::NotInitialized`: `initialize` has not run, so no vault is configured.
    pub fn create_stream(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        total_amount: i128,
        start_ledger: u32,
        end_ledger: u32,
        cliff_ledger: u32,
        cancellable: bool,
    ) -> Result<u64, StreamError> {
        sender.require_auth();

        if storage::is_paused(&env) {
            return Err(StreamError::Paused);
        }
        if total_amount <= 0 {
            return Err(StreamError::InvalidAmount);
        }
        if end_ledger <= start_ledger {
            return Err(StreamError::InvalidTimeRange);
        }
        if cliff_ledger < start_ledger || cliff_ledger > end_ledger {
            return Err(StreamError::InvalidCliff);
        }

        let vault = storage::get_vault(&env).ok_or(StreamError::NotInitialized)?;
        let id = storage::next_stream_id(&env);
        let now = env.ledger().sequence();

        let stream = Stream {
            id,
            sender: sender.clone(),
            recipient: recipient.clone(),
            token: token.clone(),
            total_amount,
            withdrawn: 0,
            start_ledger,
            end_ledger,
            cliff_ledger,
            cancellable,
            status: StreamStatus::Pending,
            created_at: now,
            cancelled_at_ledger: 0,
        };
        storage::set_stream(&env, &stream);
        storage::add_sender_stream(&env, &sender, id);
        storage::add_recipient_stream(&env, &recipient, id);

        vault_client::deposit(&env, &vault, &sender, &token, total_amount, id);

        events::Create {
            stream_id: id,
            sender,
            recipient,
            token,
            total_amount,
            start_ledger,
            end_ledger,
            cliff_ledger,
            cancellable,
        }
        .publish(&env);

        Ok(id)
    }

    /// Total value accrued so far, ignoring withdrawals. See
    /// `math::streamed_amount` for the exact formula.
    ///
    /// # Errors
    ///
    /// - `StreamError::StreamNotFound`: no stream exists with `stream_id`.
    /// - `StreamError::InvalidTimeRange`: the stored range is empty. `create_stream` rejects such ranges, so this is defensive only.
    /// - `StreamError::MathOverflow`: the accrual arithmetic overflows.
    pub fn streamed_amount(env: Env, stream_id: u64) -> Result<i128, StreamError> {
        let stream = storage::get_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        let now = env.ledger().sequence();
        math::streamed_amount(&stream, now)
    }

    /// `streamed_amount - withdrawn`, or 0 before the cliff.
    ///
    /// # Errors
    ///
    /// - `StreamError::StreamNotFound`: no stream exists with `stream_id`.
    /// - `StreamError::InvalidTimeRange`: the stored range is empty. `create_stream` rejects such ranges, so this is defensive only.
    /// - `StreamError::MathOverflow`: the accrual or subtraction overflows.
    pub fn withdrawable_amount(env: Env, stream_id: u64) -> Result<i128, StreamError> {
        let stream = storage::get_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        let now = env.ledger().sequence();
        math::withdrawable_amount(&stream, now)
    }

    /// The recipient withdraws `amount` (up to `withdrawable_amount`) from
    /// their stream.
    ///
    /// # Errors
    ///
    /// - `StreamError::InvalidAmount`: `amount` is zero or negative.
    /// - `StreamError::StreamNotFound`: no stream exists with `stream_id`.
    /// - `StreamError::Unauthorized`: `recipient` is not the stream's recipient.
    /// - `StreamError::ExceedsWithdrawable`: `amount` is greater than `withdrawable_amount`. This includes every call made before the cliff, which returns this error rather than a cliff-specific one.
    /// - `StreamError::MathOverflow`: the accrual or the `withdrawn` update overflows.
    pub fn withdraw(
        env: Env,
        recipient: Address,
        stream_id: u64,
        amount: i128,
    ) -> Result<(), StreamError> {
        recipient.require_auth();
        if amount <= 0 {
            return Err(StreamError::InvalidAmount);
        }

        let mut stream = storage::get_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.recipient != recipient {
            return Err(StreamError::Unauthorized);
        }

        let now = env.ledger().sequence();
        let available = math::withdrawable_amount(&stream, now)?;
        if amount > available {
            return Err(StreamError::ExceedsWithdrawable);
        }

        stream.withdrawn = stream
            .withdrawn
            .checked_add(amount)
            .ok_or(StreamError::MathOverflow)?;
        storage::set_stream(&env, &stream);

        let vault = storage::get_vault(&env).ok_or(StreamError::NotInitialized)?;
        vault_client::release(
            &env,
            &vault,
            &env.current_contract_address(),
            &recipient,
            &stream.token,
            amount,
            stream_id,
        );

        events::Withdraw {
            stream_id,
            recipient,
            amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Convenience: withdraws the entire currently-withdrawable balance.
    ///
    /// Note: authorization is checked once, inside the delegated call to
    /// `withdraw` -- calling `require_auth()` a second time here
    /// for the same address within the same invocation is rejected by
    /// Soroban's auth framework ("frame is already authorized"), so this
    /// function must NOT also call `recipient.require_auth()` itself.
    ///
    /// # Errors
    ///
    /// - `StreamError::StreamNotFound`: no stream exists with `stream_id`.
    /// - `StreamError::Unauthorized`: `recipient` is not the stream's recipient.
    /// - `StreamError::ExceedsWithdrawable`: nothing is withdrawable yet (e.g. before the cliff).
    /// - `StreamError::MathOverflow`: the accrual or the `withdrawn` update overflows.
    pub fn withdraw_max(env: Env, recipient: Address, stream_id: u64) -> Result<(), StreamError> {
        let stream = storage::get_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.recipient != recipient {
            return Err(StreamError::Unauthorized);
        }

        let now = env.ledger().sequence();
        let amount = math::withdrawable_amount(&stream, now)?;
        if amount <= 0 {
            return Err(StreamError::ExceedsWithdrawable);
        }

        Self::withdraw(env, recipient, stream_id, amount)
    }

    /// Cancels a cancellable stream. The recipient keeps what has already
    /// accrued; the sender is refunded the remainder.
    ///
    /// # Errors
    ///
    /// - `StreamError::StreamNotFound`: no stream exists with `stream_id`.
    /// - `StreamError::Unauthorized`: `sender` is not the stream's sender.
    /// - `StreamError::NotCancellable`: the stream was created with `cancellable = false`.
    /// - `StreamError::AlreadyCancelled`: the stream is already cancelled.
    /// - `StreamError::MathOverflow`: the accrual or refund arithmetic overflows.
    pub fn cancel(env: Env, sender: Address, stream_id: u64) -> Result<(), StreamError> {
        sender.require_auth();

        let mut stream = storage::get_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.sender != sender {
            return Err(StreamError::Unauthorized);
        }
        if !stream.cancellable {
            return Err(StreamError::NotCancellable);
        }
        if stream.status == StreamStatus::Cancelled {
            return Err(StreamError::AlreadyCancelled);
        }

        let now = env.ledger().sequence();
        let streamed = math::streamed_amount(&stream, now)?;
        let refund = stream
            .total_amount
            .checked_sub(streamed)
            .ok_or(StreamError::MathOverflow)?;

        stream.status = StreamStatus::Cancelled;
        stream.cancelled_at_ledger = now;
        storage::set_stream(&env, &stream);

        let vault = storage::get_vault(&env).ok_or(StreamError::NotInitialized)?;
        if refund > 0 {
            vault_client::refund(
                &env,
                &vault,
                &env.current_contract_address(),
                &sender,
                &stream.token,
                refund,
                stream_id,
            );
        }

        events::Cancel {
            stream_id,
            sender,
            refund,
        }
        .publish(&env);
        Ok(())
    }

    /// Returns the stream with `stream_id`, if it exists.
    pub fn get_stream(env: Env, stream_id: u64) -> Option<Stream> {
        storage::get_stream(&env, stream_id)
    }

    /// Number of streams created by `sender`.
    pub fn sender_stream_count(env: Env, sender: Address) -> u32 {
        storage::get_sender_streams(&env, &sender).len()
    }

    /// Number of streams received by `recipient`.
    pub fn recipient_stream_count(env: Env, recipient: Address) -> u32 {
        storage::get_recipient_streams(&env, &recipient).len()
    }

    /// A page of stream ids created by `sender`, starting at `offset`.
    ///
    /// # Errors
    ///
    /// - `StreamError::InvalidPageSize`: `limit` is zero or greater than `MAX_PAGE_SIZE`.
    pub fn get_sender_streams(
        env: Env,
        sender: Address,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<u64>, StreamError> {
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(StreamError::InvalidPageSize);
        }
        let all = storage::get_sender_streams(&env, &sender);
        Ok(paginate(&all, offset, limit))
    }

    /// A page of stream ids received by `recipient`, starting at `offset`.
    ///
    /// # Errors
    ///
    /// - `StreamError::InvalidPageSize`: `limit` is zero or greater than `MAX_PAGE_SIZE`.
    pub fn get_recipient_streams(
        env: Env,
        recipient: Address,
        offset: u32,
        limit: u32,
    ) -> Vec<Stream> {
        let ids = storage::get_recipient_streams(&env, &recipient);
        Self::hydrate(&env, &Self::slice(&env, &ids, offset, limit))
    }

    /// Admin only. Blocks `create_stream` until `unpause` is called.
    /// Deliberately does NOT block `withdraw`, `withdraw_max` or
    /// `cancel_stream`: pausing is an emergency brake on new deposits, never
    /// a way to trap funds that are already streaming.
    ///
    /// # Errors
    ///
    /// - `StreamError::NotInitialized`: the contract has not been initialized.
    pub fn pause(env: Env) -> Result<(), StreamError> {
        let admin = storage::get_admin(&env).ok_or(StreamError::NotInitialized)?;
        admin.require_auth();
        storage::set_paused(&env, true);
        events::Pause { admin }.publish(&env);
        Ok(())
    }

    /// Admin only. Re-enables `create_stream`.
    ///
    /// # Errors
    ///
    /// - `StreamError::NotInitialized`: the contract has not been initialized.
    pub fn unpause(env: Env) -> Result<(), StreamError> {
        let admin = storage::get_admin(&env).ok_or(StreamError::NotInitialized)?;
        admin.require_auth();
        storage::set_paused(&env, false);
        events::Unpause { admin }.publish(&env);
        Ok(())
    }

    /// Whether new stream creation is currently paused.
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    /// Admin only. Hands the admin role to `new_admin`. Both the current
    /// admin and the new admin must authorize, so the role can never be
    /// transferred to an address nobody controls by mistake.
    ///
    /// # Errors
    ///
    /// - `StreamError::NotInitialized`: the contract has not been initialized.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), StreamError> {
        let admin = storage::get_admin(&env).ok_or(StreamError::NotInitialized)?;
        admin.require_auth();
        new_admin.require_auth();
        storage::set_admin(&env, &new_admin);
        events::AdminTransfer {
            previous_admin: admin,
            new_admin,
        }
        let all = storage::get_recipient_streams(&env, &recipient);
        Ok(paginate(&all, offset, limit))
    }
}

/// Slices `all` to `[offset, offset + limit)`, clamping to the end.
fn paginate(all: &Vec<u64>, offset: u32, limit: u32) -> Vec<u64> {
    let mut page = Vec::new(all.env());
    let len = all.len();
    if offset >= len {
        return page;
    }
    let end = offset.saturating_add(limit).min(len);
    let mut i = offset;
    while i < end {
        page.push_back(all.get_unchecked(i));
        i += 1;
    }
    page
}
