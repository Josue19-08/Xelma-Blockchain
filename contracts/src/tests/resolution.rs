//! Tests for round resolution and winnings distribution.

use crate::contract::{VirtualTokenContract, VirtualTokenContractClient};
use crate::errors::ContractError;
use crate::types::{BetSide, DataKey, PrecisionPrediction, Round, UserPosition};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, Map, Vec,
};

#[test]
fn test_resolve_round_price_unchanged() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create a round with start price 1.5 XLM
    let start_price: u128 = 1_5000000;
    client.create_round(&start_price, &None);

    // Manually set up some test positions using env.as_contract
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Give users initial balances
    client.mint_initial(&user1);
    client.mint_initial(&user2);

    // Manually create positions for testing using as_contract
    env.as_contract(&contract_id, || {
        let mut positions = Map::<Address, UserPosition>::new(&env);
        positions.set(
            user1.clone(),
            UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            },
        );
        positions.set(
            user2.clone(),
            UserPosition {
                amount: 50_0000000,
                side: BetSide::Down,
            },
        );

        // Store positions in UpDownPositions (new storage location)
        env.storage()
            .persistent()
            .set(&DataKey::UpDownPositions, &positions);

        // Update round pools to match positions
        let mut round: Round = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveRound)
            .unwrap();
        round.pool_up = 100_0000000;
        round.pool_down = 50_0000000;
        env.storage()
            .persistent()
            .set(&DataKey::ActiveRound, &round);
    });

    // Get balances before resolution
    let user1_balance_before = client.balance(&user1);
    let user2_balance_before = client.balance(&user2);

    // Advance ledger to allow resolution (default run window is 12)
    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });
    // Resolve with SAME price (unchanged)
    client.resolve_round(&start_price);

    // Check pending winnings (not claimed yet)
    assert_eq!(client.get_pending_winnings(&user1), 100_0000000);
    assert_eq!(client.get_pending_winnings(&user2), 50_0000000);

    // Claim winnings
    let claimed1 = client.claim_winnings(&user1);
    let claimed2 = client.claim_winnings(&user2);

    assert_eq!(claimed1, 100_0000000);
    assert_eq!(claimed2, 50_0000000);

    // Both users should get their bets back
    assert_eq!(client.balance(&user1), user1_balance_before + 100_0000000);
    assert_eq!(client.balance(&user2), user2_balance_before + 50_0000000);

    // Round should be cleared
    assert_eq!(client.get_active_round(), None);
}

#[test]
fn test_resolve_round_price_went_up() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create a round with start price 1.0 XLM
    let start_price: u128 = 1_0000000;
    client.create_round(&start_price, &None);

    // Set up test users
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);

    // Give users initial balances
    client.mint_initial(&alice);
    client.mint_initial(&bob);
    client.mint_initial(&charlie);

    // Create positions using as_contract
    env.as_contract(&contract_id, || {
        let mut positions = Map::<Address, UserPosition>::new(&env);
        positions.set(
            alice.clone(),
            UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            },
        );
        positions.set(
            bob.clone(),
            UserPosition {
                amount: 200_0000000,
                side: BetSide::Up,
            },
        );
        positions.set(
            charlie.clone(),
            UserPosition {
                amount: 150_0000000,
                side: BetSide::Down,
            },
        );

        env.storage()
            .persistent()
            .set(&DataKey::UpDownPositions, &positions);

        let mut round: Round = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveRound)
            .unwrap();
        round.pool_up = 300_0000000;
        round.pool_down = 150_0000000;
        env.storage()
            .persistent()
            .set(&DataKey::ActiveRound, &round);
    });

    let alice_before = client.balance(&alice);
    let bob_before = client.balance(&bob);
    let charlie_before = client.balance(&charlie);

    // Advance ledger to allow resolution
    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });
    // Resolve with HIGHER price (1.5 XLM - price went UP)
    client.resolve_round(&1_5000000);

    // Check pending winnings
    assert_eq!(client.get_pending_winnings(&alice), 150_0000000);
    assert_eq!(client.get_pending_winnings(&bob), 300_0000000);
    assert_eq!(client.get_pending_winnings(&charlie), 0); // Lost

    // Check stats: Alice and Bob won, Charlie lost
    let alice_stats = client.get_user_stats(&alice);
    assert_eq!(alice_stats.total_wins, 1);
    assert_eq!(alice_stats.current_streak, 1);

    let charlie_stats = client.get_user_stats(&charlie);
    assert_eq!(charlie_stats.total_losses, 1);
    assert_eq!(charlie_stats.current_streak, 0);

    // Claim winnings
    client.claim_winnings(&alice);
    client.claim_winnings(&bob);

    assert_eq!(client.balance(&alice), alice_before + 150_0000000);
    assert_eq!(client.balance(&bob), bob_before + 300_0000000);
    assert_eq!(client.balance(&charlie), charlie_before); // No change (lost)
}

#[test]
fn test_resolve_round_price_went_down() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create a round with start price 2.0 XLM
    let start_price: u128 = 2_0000000;
    client.create_round(&start_price, &None);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint_initial(&alice);
    client.mint_initial(&bob);

    // Create positions using as_contract
    env.as_contract(&contract_id, || {
        let mut positions = Map::<Address, UserPosition>::new(&env);
        positions.set(
            alice.clone(),
            UserPosition {
                amount: 200_0000000,
                side: BetSide::Down,
            },
        );
        positions.set(
            bob.clone(),
            UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            },
        );

        env.storage()
            .persistent()
            .set(&DataKey::UpDownPositions, &positions);

        let mut round: Round = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveRound)
            .unwrap();
        round.pool_up = 100_0000000;
        round.pool_down = 200_0000000;
        env.storage()
            .persistent()
            .set(&DataKey::ActiveRound, &round);
    });

    let alice_before = client.balance(&alice);
    let bob_before = client.balance(&bob);

    // Advance ledger to allow resolution
    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });
    // Resolve with LOWER price (1.0 XLM - price went DOWN)
    client.resolve_round(&1_0000000);

    // Check pending winnings
    assert_eq!(client.get_pending_winnings(&alice), 300_0000000);
    assert_eq!(client.get_pending_winnings(&bob), 0);

    // Alice wins: 200 + (200/200) * 100 = 200 + 100 = 300
    client.claim_winnings(&alice);

    assert_eq!(client.balance(&alice), alice_before + 300_0000000);
    assert_eq!(client.balance(&bob), bob_before); // No change (lost)
}

#[test]
fn test_claim_winnings_when_none() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    env.mock_all_auths();

    // Try to claim with no pending winnings
    let claimed = client.claim_winnings(&user);
    assert_eq!(claimed, 0);
}

#[test]
fn test_user_stats_tracking() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let alice = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &oracle);

    // Initial stats should be all zeros
    let stats = client.get_user_stats(&alice);
    assert_eq!(stats.total_wins, 0);
    assert_eq!(stats.total_losses, 0);
    assert_eq!(stats.current_streak, 0);
    assert_eq!(stats.best_streak, 0);

    // Simulate a win
    env.as_contract(&contract_id, || {
        VirtualTokenContract::_update_stats_win(&env, alice.clone());
    });

    let stats = client.get_user_stats(&alice);
    assert_eq!(stats.total_wins, 1);
    assert_eq!(stats.current_streak, 1);
    assert_eq!(stats.best_streak, 1);

    // Another win - streak increases
    env.as_contract(&contract_id, || {
        VirtualTokenContract::_update_stats_win(&env, alice.clone());
    });

    let stats = client.get_user_stats(&alice);
    assert_eq!(stats.total_wins, 2);
    assert_eq!(stats.current_streak, 2);
    assert_eq!(stats.best_streak, 2);

    // A loss - streak resets
    env.as_contract(&contract_id, || {
        VirtualTokenContract::_update_stats_loss(&env, alice.clone());
    });

    let stats = client.get_user_stats(&alice);
    assert_eq!(stats.total_wins, 2);
    assert_eq!(stats.total_losses, 1);
    assert_eq!(stats.current_streak, 0); // Reset
    assert_eq!(stats.best_streak, 2); // Best remains
}

#[test]
fn test_resolve_round_without_active_round() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Try to resolve without creating a round - should return error
    let result = client.try_resolve_round(&1_0000000);
    assert_eq!(result, Err(Ok(ContractError::NoActiveRound)));
}

// ============================================================================
// PRECISION MODE RESOLUTION TESTS
// ============================================================================

#[test]
fn test_resolve_precision_closest_guess_wins() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create Precision mode round starting at 2000
    client.create_round(&2000, &Some(1));

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);

    client.mint_initial(&alice);
    client.mint_initial(&bob);
    client.mint_initial(&charlie);

    // Manually create precision predictions using as_contract
    env.as_contract(&contract_id, || {
        let mut predictions = Vec::<PrecisionPrediction>::new(&env);

        // Alice guesses 2297 (closest to actual 2298 - diff 1)
        predictions.push_back(PrecisionPrediction {
            user: alice.clone(),
            predicted_price: 2297,
            amount: 100_0000000,
        });

        // Bob guesses 2300 (diff 2 from actual 2298)
        predictions.push_back(PrecisionPrediction {
            user: bob.clone(),
            predicted_price: 2300,
            amount: 150_0000000,
        });

        // Charlie guesses 2500 (far off - diff 202)
        predictions.push_back(PrecisionPrediction {
            user: charlie.clone(),
            predicted_price: 2500,
            amount: 50_0000000,
        });

        env.storage()
            .persistent()
            .set(&DataKey::PrecisionPositions, &predictions);
    });

    // Advance ledger to allow resolution
    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Resolve with actual price 2298
    client.resolve_round(&2298);

    // Alice should win the entire pot (100 + 150 + 50 = 300)
    assert_eq!(client.get_pending_winnings(&alice), 300_0000000);
    assert_eq!(client.get_pending_winnings(&bob), 0);
    assert_eq!(client.get_pending_winnings(&charlie), 0);

    // Check stats
    let alice_stats = client.get_user_stats(&alice);
    assert_eq!(alice_stats.total_wins, 1);
    assert_eq!(alice_stats.current_streak, 1);

    let bob_stats = client.get_user_stats(&bob);
    assert_eq!(bob_stats.total_losses, 1);
    assert_eq!(bob_stats.current_streak, 0);

    // Round should be cleared
    assert_eq!(client.get_active_round(), None);
}

#[test]
fn test_resolve_precision_tie_splits_pot() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create Precision mode round
    client.create_round(&2000, &Some(1));

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);

    client.mint_initial(&alice);
    client.mint_initial(&bob);
    client.mint_initial(&charlie);

    // Create tied predictions
    env.as_contract(&contract_id, || {
        let mut predictions = Vec::<PrecisionPrediction>::new(&env);

        // Alice guesses 2100 (diff 100 from actual 2200)
        predictions.push_back(PrecisionPrediction {
            user: alice.clone(),
            predicted_price: 2100,
            amount: 100_0000000,
        });

        // Bob guesses 2300 (diff 100 from actual 2200) - TIE with Alice
        predictions.push_back(PrecisionPrediction {
            user: bob.clone(),
            predicted_price: 2300,
            amount: 150_0000000,
        });

        // Charlie guesses 2500 (diff 300 from actual 2200)
        predictions.push_back(PrecisionPrediction {
            user: charlie.clone(),
            predicted_price: 2500,
            amount: 50_0000000,
        });

        env.storage()
            .persistent()
            .set(&DataKey::PrecisionPositions, &predictions);
    });

    // Advance ledger
    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Resolve with actual price 2200
    client.resolve_round(&2200);

    // Total pot is 300, split evenly between Alice and Bob (150 each)
    assert_eq!(client.get_pending_winnings(&alice), 150_0000000);
    assert_eq!(client.get_pending_winnings(&bob), 150_0000000);
    assert_eq!(client.get_pending_winnings(&charlie), 0);

    // Both Alice and Bob should have win stats
    let alice_stats = client.get_user_stats(&alice);
    assert_eq!(alice_stats.total_wins, 1);

    let bob_stats = client.get_user_stats(&bob);
    assert_eq!(bob_stats.total_wins, 1);

    let charlie_stats = client.get_user_stats(&charlie);
    assert_eq!(charlie_stats.total_losses, 1);
}

#[test]
fn test_resolve_precision_exact_match() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    client.create_round(&2000, &Some(1));

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint_initial(&alice);
    client.mint_initial(&bob);

    env.as_contract(&contract_id, || {
        let mut predictions = Vec::<PrecisionPrediction>::new(&env);

        // Alice guesses exactly right (diff 0)
        predictions.push_back(PrecisionPrediction {
            user: alice.clone(),
            predicted_price: 2250,
            amount: 100_0000000,
        });

        // Bob is off by 50
        predictions.push_back(PrecisionPrediction {
            user: bob.clone(),
            predicted_price: 2200,
            amount: 100_0000000,
        });

        env.storage()
            .persistent()
            .set(&DataKey::PrecisionPositions, &predictions);
    });

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Alice guessed exactly right
    client.resolve_round(&2250);

    assert_eq!(client.get_pending_winnings(&alice), 200_0000000); // Wins entire pot
    assert_eq!(client.get_pending_winnings(&bob), 0);
}

#[test]
fn test_resolve_precision_no_predictions() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    // Create Precision mode round with no predictions
    client.create_round(&2000, &Some(1));

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Resolve with no predictions - should succeed without errors
    client.resolve_round(&2250);

    // Round should be cleared
    assert_eq!(client.get_active_round(), None);
}

#[test]
fn test_resolve_precision_three_way_tie() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    client.create_round(&2000, &Some(1));

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);

    client.mint_initial(&alice);
    client.mint_initial(&bob);
    client.mint_initial(&charlie);

    env.as_contract(&contract_id, || {
        let mut predictions = Vec::<PrecisionPrediction>::new(&env);

        // All three tie with diff of 10
        predictions.push_back(PrecisionPrediction {
            user: alice.clone(),
            predicted_price: 2190,
            amount: 100_0000000,
        });

        predictions.push_back(PrecisionPrediction {
            user: bob.clone(),
            predicted_price: 2210,
            amount: 150_0000000,
        });

        predictions.push_back(PrecisionPrediction {
            user: charlie.clone(),
            predicted_price: 2210,
            amount: 150_0000000,
        });

        env.storage()
            .persistent()
            .set(&DataKey::PrecisionPositions, &predictions);
    });

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Actual price 2200 - Alice diff 10, Bob diff 10, Charlie diff 10
    client.resolve_round(&2200);

    // Total pot is 400, split 3 ways = 133 each (integer division)
    let pot_per_winner = 400_0000000 / 3;
    assert_eq!(client.get_pending_winnings(&alice), pot_per_winner);
    assert_eq!(client.get_pending_winnings(&bob), pot_per_winner);
    assert_eq!(client.get_pending_winnings(&charlie), pot_per_winner);
}

#[test]
fn test_resolve_precision_single_prediction() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    client.create_round(&2000, &Some(1));

    let alice = Address::generate(&env);
    client.mint_initial(&alice);

    env.as_contract(&contract_id, || {
        let mut predictions = Vec::<PrecisionPrediction>::new(&env);

        predictions.push_back(PrecisionPrediction {
            user: alice.clone(),
            predicted_price: 2300,
            amount: 100_0000000,
        });

        env.storage()
            .persistent()
            .set(&DataKey::PrecisionPositions, &predictions);
    });

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Single prediction always wins
    client.resolve_round(&2500);

    assert_eq!(client.get_pending_winnings(&alice), 100_0000000);
}

#[test]
fn test_resolve_precision_large_differences() {
    let env = Env::default();
    let contract_id = env.register(VirtualTokenContract, ());
    let client = VirtualTokenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin, &oracle);

    client.create_round(&100_0000, &Some(1));

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint_initial(&alice);
    client.mint_initial(&bob);

    env.as_contract(&contract_id, || {
        let mut predictions = Vec::<PrecisionPrediction>::new(&env);

        // Very large price predictions
        predictions.push_back(PrecisionPrediction {
            user: alice.clone(),
            predicted_price: 1_0000,
            amount: 100_0000000,
        });

        predictions.push_back(PrecisionPrediction {
            user: bob.clone(),
            predicted_price: 9_9999,
            amount: 100_0000000,
        });

        env.storage()
            .persistent()
            .set(&DataKey::PrecisionPositions, &predictions);
    });

    env.ledger().with_mut(|li| {
        li.sequence_number = 12;
    });

    // Actual price is 1_0001 - Alice is closest (diff 1 vs Bob's diff 8_9998)
    client.resolve_round(&1_0001);

    assert_eq!(client.get_pending_winnings(&alice), 200_0000000);
    assert_eq!(client.get_pending_winnings(&bob), 0);
}
