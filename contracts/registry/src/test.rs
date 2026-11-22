#![cfg(test)]

use crate::{RegistryContract, RegistryContractClient, RegistryError, StreamStatus};
use soroban_sdk::{testutils::Address as _, Address, Env};

struct TestCtx {
    env: Env,
    registry: RegistryContractClient<'static>,
    admin: Address,
}

fn setup() -> TestCtx {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(RegistryContract, ());
    let registry = RegistryContractClient::new(&env, &id);
    registry.initialize(&admin);
    TestCtx {
        env,
        registry,
        admin,
    }
}

#[test]
fn test_initialize_sets_empty_stats() {
    let ctx = setup();
    let stats = ctx.registry.get_stats();
    assert_eq!(stats.total_streams, 0);
    assert_eq!(stats.active_streams, 0);
    assert_eq!(stats.total_streamed, 0);
    assert_eq!(stats.total_withdrawn, 0);
}

#[test]
fn test_initialize_twice_rejected() {
    let ctx = setup();
    let result = ctx.registry.try_initialize(&ctx.admin);
    assert_eq!(result, Err(Ok(RegistryError::AlreadyInitialized)));
}

#[test]
fn test_get_active_streams_empty_after_initialize() {
    let ctx = setup();
    let active = ctx.registry.get_active_streams();
    assert_eq!(active, soroban_sdk::Vec::<u64>::new(&ctx.env));
    assert_eq!(active.len(), 0);
    assert_eq!(ctx.registry.stream_count(), 0);
}

#[test]
fn test_register_stream_adds_to_all_streams() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);

    let all = ctx.registry.get_all_streams();
    assert_eq!(all.len(), 1);
    assert_eq!(all.get(0).unwrap(), 1);
}

#[test]
fn test_get_all_streams_preserves_insertion_order() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    ctx.registry
        .register_stream(&5, &sender, &recipient, &token, &1000);
    ctx.registry
        .register_stream(&2, &sender, &recipient, &token, &2000);
    ctx.registry
        .register_stream(&9, &sender, &recipient, &token, &3000);

    let all = ctx.registry.get_all_streams();
    assert_eq!(all.len(), 3);
    assert_eq!(all.get(0).unwrap(), 5);
    assert_eq!(all.get(1).unwrap(), 2);
    assert_eq!(all.get(2).unwrap(), 9);
}

#[test]
fn test_register_stream_updates_stats() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);
    ctx.registry
        .register_stream(&2, &sender, &recipient, &token, &500);

    let stats = ctx.registry.get_stats();
    assert_eq!(stats.total_streams, 2);
    assert_eq!(stats.active_streams, 2);
    assert_eq!(stats.total_streamed, 1500);
}

#[test]
fn test_register_stream_rejects_negative_amount() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let before = ctx.registry.get_stats();

    let result = ctx
        .registry
        .try_register_stream(&1, &sender, &recipient, &token, &-1);
    assert_eq!(result, Err(Ok(RegistryError::InvalidAmount)));

    let after = ctx.registry.get_stats();
    assert_eq!(after.total_streams, before.total_streams);
    assert_eq!(after.active_streams, before.active_streams);
    assert_eq!(after.total_streamed, before.total_streamed);
    assert_eq!(after.total_withdrawn, before.total_withdrawn);
}

#[test]
fn test_register_stream_rejects_duplicate_id() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);
    let result = ctx
        .registry
        .try_register_stream(&1, &sender, &recipient, &token, &500);
    assert_eq!(result, Err(Ok(RegistryError::AlreadyRegistered)));
}

#[test]
fn test_update_stream_tracks_withdrawn_delta() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);

    ctx.registry.update_stream(&1, &300, &StreamStatus::Active);
    assert_eq!(ctx.registry.get_stats().total_withdrawn, 300);

    ctx.registry.update_stream(&1, &700, &StreamStatus::Active);
    assert_eq!(ctx.registry.get_stats().total_withdrawn, 700);
}

#[test]
fn test_lowering_withdrawn_does_not_reduce_total_withdrawn() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);

    ctx.registry.update_stream(&1, &100, &StreamStatus::Active);
    ctx.registry.update_stream(&1, &40, &StreamStatus::Active);

    assert_eq!(ctx.registry.get_stream_record(&1).withdrawn, 40);
    assert_eq!(ctx.registry.get_stats().total_withdrawn, 100);
}

#[test]
fn test_update_stream_rejects_negative_withdrawn() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);

    let before = ctx.registry.get_stats();

    let result = ctx
        .registry
        .try_update_stream(&1, &-1, &StreamStatus::Active);
    assert_eq!(result, Err(Ok(RegistryError::InvalidAmount)));

    let after = ctx.registry.get_stats();
    assert_eq!(after.total_streams, before.total_streams);
    assert_eq!(after.active_streams, before.active_streams);
    assert_eq!(after.total_streamed, before.total_streamed);
    assert_eq!(after.total_withdrawn, before.total_withdrawn);
    assert_eq!(ctx.registry.get_stream_record(&1).withdrawn, 0);
}

#[test]
fn test_get_admin_before_initialize_returns_not_initialized() {
    let env = Env::default();
    let id = env.register(RegistryContract, ());
    let registry = RegistryContractClient::new(&env, &id);

    assert_eq!(
        registry.try_get_admin(),
        Err(Ok(RegistryError::NotInitialized))
    );
}

#[test]
fn test_update_stream_to_pending_decrements_active_count() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);
    assert_eq!(ctx.registry.get_stats().active_streams, 1);

    ctx.registry.update_stream(&1, &0, &StreamStatus::Pending);
    assert_eq!(ctx.registry.get_stats().active_streams, 0);

    ctx.registry.update_stream(&1, &0, &StreamStatus::Active);
    assert_eq!(ctx.registry.get_stats().active_streams, 1);
}

#[test]
fn test_cancelled_to_completed_keeps_active_count_at_zero() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);

    ctx.registry.update_stream(&1, &0, &StreamStatus::Cancelled);
    assert_eq!(ctx.registry.get_stats().active_streams, 0);

    ctx.registry.update_stream(&1, &0, &StreamStatus::Completed);
    assert_eq!(ctx.registry.get_stats().active_streams, 0);
}

#[test]
fn test_update_stream_to_completed_decrements_active_count() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);
    assert_eq!(ctx.registry.get_stats().active_streams, 1);

    ctx.registry
        .update_stream(&1, &1000, &StreamStatus::Completed);
    assert_eq!(ctx.registry.get_stats().active_streams, 0);
}

#[test]
fn test_update_stream_to_cancelled_decrements_active_count() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);

    ctx.registry
        .update_stream(&1, &400, &StreamStatus::Cancelled);
    let stats = ctx.registry.get_stats();
    assert_eq!(stats.active_streams, 0);
    assert_eq!(stats.total_withdrawn, 400);
}

#[test]
fn test_registry_stats_after_mix_of_register_and_update() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    enum Action {
        Register {
            id: u64,
            amount: i128,
        },
        Update {
            id: u64,
            withdrawn: i128,
            status: StreamStatus,
        },
    }

    struct Step {
        action: Action,
        expected_total_streams: u64,
        expected_active_streams: u64,
        expected_total_streamed: i128,
        expected_total_withdrawn: i128,
    }

    let steps = [
        // 1. Register stream 1 (Active by default)
        Step {
            action: Action::Register {
                id: 1,
                amount: 1000,
            },
            expected_total_streams: 1,
            expected_active_streams: 1,
            expected_total_streamed: 1000,
            expected_total_withdrawn: 0,
        },
        // 2. Update stream 1 to Pending (was Active -> active decreases)
        Step {
            action: Action::Update {
                id: 1,
                withdrawn: 100,
                status: StreamStatus::Pending,
            },
            expected_total_streams: 1,
            expected_active_streams: 0,
            expected_total_streamed: 1000,
            expected_total_withdrawn: 100,
        },
        // 3. Register stream 2 (Active by default)
        Step {
            action: Action::Register {
                id: 2,
                amount: 2000,
            },
            expected_total_streams: 2,
            expected_active_streams: 1,
            expected_total_streamed: 3000,
            expected_total_withdrawn: 100,
        },
        // 4. Update stream 2 while keeping Active (total_withdrawn increases by 500)
        Step {
            action: Action::Update {
                id: 2,
                withdrawn: 500,
                status: StreamStatus::Active,
            },
            expected_total_streams: 2,
            expected_active_streams: 1,
            expected_total_streamed: 3000,
            expected_total_withdrawn: 600,
        },
        // 5. Register stream 3 (Active by default)
        Step {
            action: Action::Register {
                id: 3,
                amount: 3000,
            },
            expected_total_streams: 3,
            expected_active_streams: 2,
            expected_total_streamed: 6000,
            expected_total_withdrawn: 600,
        },
        // 6. Update stream 1 back to Active (was Pending -> active increases)
        Step {
            action: Action::Update {
                id: 1,
                withdrawn: 300,
                status: StreamStatus::Active,
            },
            expected_total_streams: 3,
            expected_active_streams: 3,
            expected_total_streamed: 6000,
            expected_total_withdrawn: 800,
        },
        // 7. Update stream 2 to Completed (was Active -> active decreases)
        Step {
            action: Action::Update {
                id: 2,
                withdrawn: 2000,
                status: StreamStatus::Completed,
            },
            expected_total_streams: 3,
            expected_active_streams: 2,
            expected_total_streamed: 6000,
            expected_total_withdrawn: 2300,
        },
        // 8. Update stream 3 to Cancelled (was Active -> active decreases)
        Step {
            action: Action::Update {
                id: 3,
                withdrawn: 1200,
                status: StreamStatus::Cancelled,
            },
            expected_total_streams: 3,
            expected_active_streams: 1,
            expected_total_streamed: 6000,
            expected_total_withdrawn: 3500,
        },
        // 9. Register stream 4 (Active by default)
        Step {
            action: Action::Register {
                id: 4,
                amount: 1500,
            },
            expected_total_streams: 4,
            expected_active_streams: 2,
            expected_total_streamed: 7500,
            expected_total_withdrawn: 3500,
        },
        // 10. Update stream 4 to Pending (was Active -> active decreases)
        Step {
            action: Action::Update {
                id: 4,
                withdrawn: 200,
                status: StreamStatus::Pending,
            },
            expected_total_streams: 4,
            expected_active_streams: 1,
            expected_total_streamed: 7500,
            expected_total_withdrawn: 3700,
        },
        // 11. Update stream 4 from Pending to Completed (active stays 1)
        Step {
            action: Action::Update {
                id: 4,
                withdrawn: 600,
                status: StreamStatus::Completed,
            },
            expected_total_streams: 4,
            expected_active_streams: 1,
            expected_total_streamed: 7500,
            expected_total_withdrawn: 4100,
        },
        // 12. Update stream 1 to Cancelled (was Active -> active drops to 0)
        Step {
            action: Action::Update {
                id: 1,
                withdrawn: 1000,
                status: StreamStatus::Cancelled,
            },
            expected_total_streams: 4,
            expected_active_streams: 0,
            expected_total_streamed: 7500,
            expected_total_withdrawn: 4800,
        },
    ];

    for (i, step) in steps.iter().enumerate() {
        match &step.action {
            Action::Register { id, amount } => {
                ctx.registry
                    .register_stream(id, &sender, &recipient, &token, amount);
            }
            Action::Update {
                id,
                withdrawn,
                status,
            } => {
                ctx.registry.update_stream(id, withdrawn, status);
            }
        }
        let stats = ctx.registry.get_stats();
        assert_eq!(
            stats.total_streams, step.expected_total_streams,
            "step {} total_streams mismatch",
            i
        );
        assert_eq!(
            stats.active_streams, step.expected_active_streams,
            "step {} active_streams mismatch",
            i
        );
        assert_eq!(
            stats.total_streamed, step.expected_total_streamed,
            "step {} total_streamed mismatch",
            i
        );
        assert_eq!(
            stats.total_withdrawn, step.expected_total_withdrawn,
            "step {} total_withdrawn mismatch",
            i
        );
    }
}

#[test]
fn test_update_stream_rejects_unknown_id() {
    let ctx = setup();
    let result = ctx
        .registry
        .try_update_stream(&999, &100, &StreamStatus::Active);
    assert_eq!(result, Err(Ok(RegistryError::StreamNotFound)));
}

#[test]
fn test_get_stream_record_rejects_unknown_id() {
    let ctx = setup();
    let result = ctx.registry.try_get_stream_record(&999);
    assert_eq!(result, Err(Ok(RegistryError::StreamNotFound)));
}

#[test]
fn test_get_active_streams_filters_correctly() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    ctx.registry
        .register_stream(&1, &sender, &recipient, &token, &1000);
    ctx.registry
        .register_stream(&2, &sender, &recipient, &token, &2000);
    ctx.registry
        .register_stream(&3, &sender, &recipient, &token, &3000);

    ctx.registry
        .update_stream(&2, &2000, &StreamStatus::Completed);

    let active = ctx.registry.get_active_streams();
    assert_eq!(active.len(), 2);
    assert_eq!(active.get(0).unwrap(), 1);
    assert_eq!(active.get(1).unwrap(), 3);
}

#[test]
fn test_get_active_streams_with_many_streams() {
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    // Register 200 records with a repeating status pattern across all 4 statuses:
    // id % 4 == 1 => Active (stays Active from registration)
    // id % 4 == 2 => Completed
    // id % 4 == 3 => Cancelled
    // id % 4 == 0 => Pending
    for id in 1..=200 {
        ctx.registry
            .register_stream(&id, &sender, &recipient, &token, &1000);
        match id % 4 {
            1 => {}
            2 => {
                ctx.registry
                    .update_stream(&id, &1000, &StreamStatus::Completed);
            }
            3 => {
                ctx.registry
                    .update_stream(&id, &500, &StreamStatus::Cancelled);
            }
            0 => {
                ctx.registry.update_stream(&id, &0, &StreamStatus::Pending);
            }
            _ => unreachable!(),
        }
    }

    // Disable default transaction resource limits since scanning 200 stream records
    // touches >400 ledger entries, allowing us to measure the full execution cost.
    ctx.env.cost_estimate().disable_resource_limits();

    let active = ctx.registry.get_active_streams();

    // Exactly 50 streams match the Active pattern (ids 1, 5, 9, ..., 197).
    assert_eq!(active.len(), 50);
    for (idx, expected_id) in (1..=200).filter(|id| id % 4 == 1).enumerate() {
        assert_eq!(active.get(idx as u32).unwrap(), expected_id);
    }

    // Resource budget ceiling measured via env.cost_estimate() for get_active_streams()
    // across 200 registered streams (50 active):
    // CPU instructions: ~26,593,681
    // Memory bytes:     ~2,671,484
    let budget = ctx.env.cost_estimate().budget();
    assert!(budget.cpu_instruction_cost() > 0);
    assert!(budget.memory_bytes_cost() > 0);
}

#[test]
fn test_register_stream_stores_every_field_of_the_record() {
    // Each input gets a distinct value, so a field that is dropped, swapped
    // with another field, or defaulted would fail its own assertion.
    let ctx = setup();
    let sender = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    let stream_id: u64 = 42;
    let amount: i128 = 7_500;

    ctx.registry
        .register_stream(&stream_id, &sender, &recipient, &token, &amount);

    let record = ctx.registry.get_stream_record(&stream_id);
    assert_eq!(record.stream_id, stream_id);
    assert_eq!(record.sender, sender);
    assert_eq!(record.recipient, recipient);
    assert_eq!(record.token, token);
    assert_eq!(record.amount, amount);
    // Not a `register_stream` input: a new record starts with nothing withdrawn.
    assert_eq!(record.withdrawn, 0);
    assert_eq!(record.status, StreamStatus::Active);
}
