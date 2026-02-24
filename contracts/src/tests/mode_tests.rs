//! Tests for round mode flag and separate prediction storage.

use crate::contract::{VirtualTokenContract, VirtualTokenContractClient};
use crate::errors::ContractError;
use crate::types::{BetSide, RoundMode};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env,
};

#[test]
fn test_create_round_default_mode() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create round without specifying mode (should default to UpDown)
    client.create_round(&1_0000000, &None);

    let round = client.get_active_round().unwrap();
    assert_eq!(round.mode, RoundMode::UpDown);
}

#[test]
fn test_create_round_updown_mode_explicit() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create round with explicit Up/Down mode (0)
    client.create_round(&1_0000000, &Some(0));

    let round = client.get_active_round().unwrap();
    assert_eq!(round.mode, RoundMode::UpDown);
}

#[test]
fn test_create_round_precision_mode() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create round with Precision mode (1)
    client.create_round(&1_0000000, &Some(1));

    let round = client.get_active_round().unwrap();
    assert_eq!(round.mode, RoundMode::Precision);
}

#[test]
fn test_create_round_invalid_mode() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Try to create round with invalid mode (2)
    let result = client.try_create_round(&1_0000000, &Some(2));
    assert_eq!(result, Err(Ok(ContractError::InvalidMode)));
}

#[test]
fn test_place_bet_on_updown_mode() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Up/Down round
    client.create_round(&1_0000000, &Some(0));

    // Place bet should work
    client.place_bet(&user, &100_0000000, &BetSide::Up);

    let position = client.get_user_position(&user).unwrap();
    assert_eq!(position.amount, 100_0000000);
    assert_eq!(position.side, BetSide::Up);
}

#[test]
fn test_place_bet_on_precision_mode_fails() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // place_bet should fail on Precision mode
    let result = client.try_place_bet(&user, &100_0000000, &BetSide::Up);
    assert_eq!(result, Err(Ok(ContractError::WrongModeForPrediction)));
}

#[test]
fn test_place_precision_prediction_on_precision_mode() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // Place precision prediction (predicted price: 0.2297 scaled to 4 decimals = 2297)
    client.place_precision_prediction(&user, &100_0000000, &2297);

    // Verify the prediction was stored
    let prediction = client.get_user_precision_prediction(&user).unwrap();
    assert_eq!(prediction.amount, 100_0000000);
    assert_eq!(prediction.predicted_price, 2297);

    // Verify balance was deducted
    assert_eq!(client.balance(&user), 900_0000000);
}

#[test]
fn test_place_precision_prediction_on_updown_mode_fails() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Up/Down round
    client.create_round(&1_0000000, &Some(0));

    // place_precision_prediction should fail on Up/Down mode
    let result = client.try_place_precision_prediction(&user, &100_0000000, &2297);
    assert_eq!(result, Err(Ok(ContractError::WrongModeForPrediction)));
}

#[test]
fn test_precision_prediction_already_bet() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // First prediction succeeds
    client.place_precision_prediction(&user, &100_0000000, &2297);

    // Second prediction should fail
    let result = client.try_place_precision_prediction(&user, &50_0000000, &2500);
    assert_eq!(result, Err(Ok(ContractError::AlreadyBet)));
}

#[test]
fn test_get_precision_predictions() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&alice);
    client.mint_initial(&bob);

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // Multiple users place predictions
    client.place_precision_prediction(&alice, &100_0000000, &2297);
    client.place_precision_prediction(&bob, &150_0000000, &2500);

    // Get all predictions
    let predictions = client.get_precision_predictions();
    assert_eq!(predictions.len(), 2);

    // Verify first prediction (alice)
    let pred0 = predictions.get(0).unwrap();
    assert_eq!(pred0.user, alice);
    assert_eq!(pred0.amount, 100_0000000);
    assert_eq!(pred0.predicted_price, 2297);

    // Verify second prediction (bob)
    let pred1 = predictions.get(1).unwrap();
    assert_eq!(pred1.user, bob);
    assert_eq!(pred1.amount, 150_0000000);
    assert_eq!(pred1.predicted_price, 2500);
}

#[test]
fn test_get_updown_positions() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&alice);
    client.mint_initial(&bob);

    // Create Up/Down round
    client.create_round(&1_0000000, &Some(0));

    // Multiple users place bets
    client.place_bet(&alice, &100_0000000, &BetSide::Up);
    client.place_bet(&bob, &150_0000000, &BetSide::Down);

    // Get all positions
    let positions = client.get_updown_positions();
    assert_eq!(positions.len(), 2);

    // Verify alice's position
    let alice_pos = positions.get(alice.clone()).unwrap();
    assert_eq!(alice_pos.amount, 100_0000000);
    assert_eq!(alice_pos.side, BetSide::Up);

    // Verify bob's position
    let bob_pos = positions.get(bob.clone()).unwrap();
    assert_eq!(bob_pos.amount, 150_0000000);
    assert_eq!(bob_pos.side, BetSide::Down);
}

#[test]
fn test_precision_insufficient_balance() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user); // Has 1000 vXLM

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // Try to bet more than balance
    let result = client.try_place_precision_prediction(&user, &2000_0000000, &2297);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_precision_round_ended() {
    let env = Env::default();
    env.ledger().with_mut(|li| {
        li.sequence_number = 0;
    });

    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round (default bet window is 6 ledgers)
    client.create_round(&1_0000000, &Some(1));

    // Advance ledger past bet window (bet closes at ledger 6)
    env.ledger().with_mut(|li| {
        li.sequence_number = 6;
    });

    // Try to place prediction after bet window closed
    let result = client.try_place_precision_prediction(&user, &100_0000000, &2297);
    assert_eq!(result, Err(Ok(ContractError::RoundEnded)));
}

#[test]
fn test_precision_invalid_amount() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // Try to bet 0 amount
    let result = client.try_place_precision_prediction(&user, &0, &2297);
    assert_eq!(result, Err(Ok(ContractError::InvalidBetAmount)));

    // Try to bet negative amount
    let result = client.try_place_precision_prediction(&user, &-100, &2297);
    assert_eq!(result, Err(Ok(ContractError::InvalidBetAmount)));
}

#[test]
fn test_predict_price_alias() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // Use predict_price function (alias with different parameter order)
    client.predict_price(&user, &2297, &100_0000000);

    // Verify the prediction was stored
    let prediction = client.get_user_precision_prediction(&user).unwrap();
    assert_eq!(prediction.amount, 100_0000000);
    assert_eq!(prediction.predicted_price, 2297);

    // Verify balance was deducted
    assert_eq!(client.balance(&user), 900_0000000);
}

#[test]
fn test_predict_price_valid_scales() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Test various valid price scales (4 decimal places)
    let test_cases = [
        1u128,        // 0.0001 XLM
        2297u128,     // 0.2297 XLM
        10000u128,    // 1.0000 XLM
        50000u128,    // 5.0000 XLM
        99999999u128, // 9999.9999 XLM (max valid)
    ];

    for price in test_cases.iter() {
        let user = Address::generate(&env);
        client.mint_initial(&user);

        // Create new round for each test
        client.create_round(&1_0000000, &Some(1));

        // Should succeed with valid price scale
        client.predict_price(&user, price, &100_0000000);

        let prediction = client.get_user_precision_prediction(&user).unwrap();
        assert_eq!(prediction.predicted_price, *price);

        // Clean up for next iteration
        env.ledger().with_mut(|li| {
            li.sequence_number += 20;
        });
    }
}

#[test]
fn test_predict_price_invalid_scale() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round
    client.create_round(&1_0000000, &Some(1));

    // Try to predict with price exceeding max scale (> 9999.9999)
    let result = client.try_predict_price(&user, &100_000_000, &100_0000000);
    assert_eq!(result, Err(Ok(ContractError::InvalidPriceScale)));

    // Try with extremely large value
    let result = client.try_predict_price(&user, &999_999_999_999, &100_0000000);
    assert_eq!(result, Err(Ok(ContractError::InvalidPriceScale)));
}

#[test]
fn test_predict_price_event_emission() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let user = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin, &oracle);
    client.mint_initial(&user);

    // Create Precision round at ledger 0
    client.create_round(&1_0000000, &Some(1));

    // Place prediction
    client.predict_price(&user, &2297, &100_0000000);

    // Verify event was emitted
    let events = env.events().all();

    // Should have events (at least the prediction event)
    assert!(!events.is_empty());
}
