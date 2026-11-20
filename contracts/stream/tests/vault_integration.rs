//! End-to-end wiring test: registers the *real* Stream and Vault contracts
//! and drives them through their public interfaces.
//!
//! The unit tests in `src/test.rs` use a mock vault, so the real call chain
//! (Stream -> `vault_client` -> Vault -> token) is never exercised there. A
//! mismatch in argument order or in the authorized caller between the two
//! crates would pass the unit tests but fail here.

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env,
};
use stellar_stream_hive_stream::{StreamContract, StreamContractClient};
use stellar_stream_hive_vault::{VaultContract, VaultContractClient};

const START: u32 = 1_000;
const END: u32 = 2_000;
const AMOUNT: i128 = 1_000_000;
const FUNDING: i128 = AMOUNT * 10;

struct Wiring {
    env: Env,
    stream: StreamContractClient<'static>,
    stream_id_addr: Address,
    vault: VaultContractClient<'static>,
    vault_id_addr: Address,
    token_address: Address,
    token: token::Client<'static>,
    sender: Address,
    recipient: Address,
}

/// Registers the real vault and stream, then links them in both directions:
/// the vault only accepts `release`/`refund` from the stream contract, and
/// the stream only moves funds through the vault it was initialized with.
fn setup() -> Wiring {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_sequence_number(0);

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_address = sac.address();
    token::StellarAssetClient::new(&env, &token_address).mint(&sender, &FUNDING);
    let token = token::Client::new(&env, &token_address);

    let stream_id_addr = env.register(StreamContract, ());
    let stream = StreamContractClient::new(&env, &stream_id_addr);

    let vault_id_addr = env.register(VaultContract, ());
    let vault = VaultContractClient::new(&env, &vault_id_addr);

    vault.initialize(&admin, &stream_id_addr);
    stream.initialize(&admin, &vault_id_addr);

    Wiring {
        env,
        stream,
        stream_id_addr,
        vault,
        vault_id_addr,
        token_address,
        token,
        sender,
        recipient,
    }
}

fn set_ledger(w: &Wiring, seq: u32) {
    w.env.ledger().set_sequence_number(seq);
}

fn create_stream(w: &Wiring) -> u64 {
    w.stream.create_stream(
        &w.sender,
        &w.recipient,
        &w.token_address,
        &AMOUNT,
        &START,
        &END,
        &START,
        &true,
    )
}

#[test]
fn test_wiring_points_each_contract_at_the_other() {
    let w = setup();
    assert_eq!(w.stream.get_vault(), w.vault_id_addr);
    assert_eq!(w.vault.get_stream_contract(), w.stream_id_addr);
}

#[test]
fn test_create_withdraw_and_cancel_move_real_tokens() {
    let w = setup();

    // Create: funds leave the sender and land in the real vault.
    set_ledger(&w, 0);
    let id = create_stream(&w);
    assert_eq!(w.token.balance(&w.sender), FUNDING - AMOUNT);
    assert_eq!(w.token.balance(&w.vault_id_addr), AMOUNT);
    assert_eq!(w.vault.balance_of_stream(&id), AMOUNT);
    assert_eq!(w.vault.total_locked(&w.token_address), AMOUNT);

    // Partial withdrawal at ledger 1250 (25% streamed).
    set_ledger(&w, 1_250);
    w.stream.withdraw(&w.recipient, &id, &100_000);
    assert_eq!(w.token.balance(&w.recipient), 100_000);
    assert_eq!(w.token.balance(&w.vault_id_addr), AMOUNT - 100_000);
    assert_eq!(w.vault.balance_of_stream(&id), AMOUNT - 100_000);
    assert_eq!(w.vault.total_locked(&w.token_address), AMOUNT - 100_000);

    // Cancel at ledger 1750 (75% streamed). The sender gets the unstreamed
    // 25%; the recipient keeps the streamed 75% and can still withdraw it.
    set_ledger(&w, 1_750);
    w.stream.cancel_stream(&w.sender, &id);
    assert_eq!(w.token.balance(&w.sender), FUNDING - AMOUNT + AMOUNT / 4);
    assert_eq!(w.stream.withdrawable_amount(&id), 750_000 - 100_000);

    let paid = w.stream.withdraw_max(&w.recipient, &id);
    assert_eq!(paid, 650_000);
    assert_eq!(w.token.balance(&w.recipient), 750_000);

    // Nothing is left in the vault, and its books agree with its balance.
    assert_eq!(w.token.balance(&w.vault_id_addr), 0);
    assert_eq!(w.vault.balance_of_stream(&id), 0);
    assert_eq!(w.vault.total_locked(&w.token_address), 0);
}

#[test]
fn test_full_withdrawal_after_end_drains_the_vault() {
    let w = setup();
    set_ledger(&w, 0);
    let id = create_stream(&w);

    set_ledger(&w, END);
    w.stream.withdraw_max(&w.recipient, &id);

    assert_eq!(w.token.balance(&w.recipient), AMOUNT);
    assert_eq!(w.token.balance(&w.vault_id_addr), 0);
    assert_eq!(w.vault.total_locked(&w.token_address), 0);
}

#[test]
fn test_vault_rejects_payouts_from_anyone_but_the_stream_contract() {
    let w = setup();
    set_ledger(&w, 0);
    let id = create_stream(&w);
    set_ledger(&w, END);

    // The sender is authenticated, but is not the configured stream contract,
    // so the real vault must refuse to release funds on its own.
    let attempt = w
        .vault
        .try_release(&w.sender, &w.sender, &w.token_address, &AMOUNT, &id);
    assert!(attempt.is_err());
    assert_eq!(w.token.balance(&w.vault_id_addr), AMOUNT);
    assert_eq!(w.vault.balance_of_stream(&id), AMOUNT);
}
