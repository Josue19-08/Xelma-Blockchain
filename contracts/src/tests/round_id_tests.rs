//! Tests for round ID functionality.

use crate::contract::{VirtualTokenContract, VirtualTokenContractClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

#[test]
fn test_round_id_starts_at_one() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle);

    // Create first round
    client.create_round(&1_0000000, &None);

    let round = client.get_active_round().expect("Round should exist");
    assert_eq!(round.round_id, 1);

    // Verify get_last_round_id
    assert_eq!(client.get_last_round_id(), 1);
}

#[test]
fn test_round_id_increments() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle);

    // Create first round
    client.create_round(&1_0000000, &None);
    let round1 = client.get_active_round().unwrap();
    assert_eq!(round1.round_id, 1);

    // Resolve first round
    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });
    client.resolve_round(&1_5000000);

    // Create second round
    client.create_round(&1_5000000, &None);
    let round2 = client.get_active_round().unwrap();
    assert_eq!(round2.round_id, 2);
    assert_eq!(client.get_last_round_id(), 2);

    // Resolve second round
    env.ledger().with_mut(|li| {
        li.sequence_number = 24;
    });
    client.resolve_round(&2_0000000);

    // Create third round
    client.create_round(&2_0000000, &None);
    let round3 = client.get_active_round().unwrap();
    assert_eq!(round3.round_id, 3);
    assert_eq!(client.get_last_round_id(), 3);
}

#[test]
fn test_round_id_persists_across_modes() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle);

    // Create Up/Down mode round (mode 0)
    client.create_round(&1_0000000, &Some(0));
    let round1 = client.get_active_round().unwrap();
    assert_eq!(round1.round_id, 1);

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });
    client.resolve_round(&1_5000000);

    // Create Precision mode round (mode 1)
    client.create_round(&1_5000000, &Some(1));
    let round2 = client.get_active_round().unwrap();
    assert_eq!(round2.round_id, 2);

    env.ledger().with_mut(|li| {
        li.sequence_number = 24;
    });
    client.resolve_round(&2_0000000);

    // Create another Up/Down mode round
    client.create_round(&2_0000000, &Some(0));
    let round3 = client.get_active_round().unwrap();
    assert_eq!(round3.round_id, 3);

    assert_eq!(client.get_last_round_id(), 3);
}

#[test]
fn test_get_last_round_id_before_any_rounds() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle);

    // Before creating any rounds, last_round_id should be 0
    assert_eq!(client.get_last_round_id(), 0);
}
