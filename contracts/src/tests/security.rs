//! Security tests for Oracle data freshness and round validation.

use crate::contract::{VirtualTokenContract, VirtualTokenContractClient};
use crate::errors::ContractError;
use crate::types::OraclePayload;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env,
};

#[test]
fn test_resolve_round_stale_timestamp() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.create_round(&1_0000000, &None);

    // Advance ledger time to 1000
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
        li.sequence_number = 12; // Allow resolution
    });

    // Submit payload with timestamp 600 (400s old, > 300s limit)
    let payload = OraclePayload {
        price: 1_5000000,
        timestamp: 600,
        round_id: 0, // Starts at ledger 0
    };

    let result = client.try_resolve_round(&payload);
    assert_eq!(result, Err(Ok(ContractError::StaleOracleData)));
}

#[test]
fn test_resolve_round_invalid_round_id() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.create_round(&1_0000000, &None);

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Submit payload with wrong round_id (e.g., 999 instead of 0)
    let payload = OraclePayload {
        price: 1_5000000,
        timestamp: env.ledger().timestamp(),
        round_id: 999,
    };

    let result = client.try_resolve_round(&payload);
    assert_eq!(result, Err(Ok(ContractError::InvalidOracleRound)));
}

#[test]
fn test_resolve_round_valid_payload() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.create_round(&1_0000000, &None);

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
        li.timestamp = 1000;
    });

    // Valid payload: within 300s and correct round_id
    let payload = OraclePayload {
        price: 1_5000000,
        timestamp: 900, // 100s old, OK
        round_id: 0,
    };

    client.resolve_round(&payload);
    assert_eq!(client.get_active_round(), None);
}
