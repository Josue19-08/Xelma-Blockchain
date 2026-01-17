#![no_std]
//! # XLM Price Prediction Market
//! 
//! Secure Soroban-based prediction market for XLM price movements.
//! Users bet on price direction (UP/DOWN) using virtual XLM tokens.
//! 
//! ## Key Features
//! - Role-based access control (Admin, Oracle, Users)
//! - Checked arithmetic prevents overflow
//! - Proportional payout distribution
//! - Comprehensive error handling

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Map, Vec};

/// Contract error types
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// Contract has already been initialized
    AlreadyInitialized = 1,
    /// Admin address not set - call initialize first
    AdminNotSet = 2,
    /// Oracle address not set - call initialize first
    OracleNotSet = 3,
    /// Only admin can perform this action
    UnauthorizedAdmin = 4,
    /// Only oracle can perform this action
    UnauthorizedOracle = 5,
    /// Bet amount must be greater than zero
    InvalidBetAmount = 6,
    /// No active round exists
    NoActiveRound = 7,
    /// Round has already ended
    RoundEnded = 8,
    /// User has insufficient balance
    InsufficientBalance = 9,
    /// User has already placed a bet in this round
    AlreadyBet = 10,
    /// Arithmetic overflow occurred
    Overflow = 11,
    /// Invalid price value
    InvalidPrice = 12,
    /// Invalid duration value
    InvalidDuration = 13,
}

/// Storage keys for contract data
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Balance(Address),
    Admin,
    Oracle,
    ActiveRound,
    Positions,
    PendingWinnings(Address),
    UserStats(Address),
}

/// Represents which side a user bet on
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BetSide {
    Up,
    Down,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct UserPosition {
    pub amount: i128,
    pub side: BetSide,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct UserStats {
    pub total_wins: u32,
    pub total_losses: u32,
    pub current_streak: u32,
    pub best_streak: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Round {
    pub price_start: u128,  // Starting XLM price in stroops
    pub end_ledger: u32,     // Ledger when round ends (~5s per ledger)
    pub pool_up: i128,       // Total vXLM bet on UP
    pub pool_down: i128,     // Total vXLM bet on DOWN
}

#[contract]
pub struct VirtualTokenContract;

#[contractimpl]
impl VirtualTokenContract {
    /// Initializes the contract with admin and oracle addresses (one-time only)
    pub fn initialize(env: Env, admin: Address, oracle: Address) -> Result<(), ContractError> {
        admin.require_auth();
        
        if env.storage().persistent().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Oracle, &oracle);
        
        Ok(())
    }
    
    /// Creates a new prediction round (admin only)
    pub fn create_round(env: Env, start_price: u128, duration_ledgers: u32) -> Result<(), ContractError> {
        if start_price == 0 {
            return Err(ContractError::InvalidPrice);
        }
        
        if duration_ledgers == 0 || duration_ledgers > 100_000 {
            return Err(ContractError::InvalidDuration);
        }
        
        let admin: Address = env.storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;
        
        admin.require_auth();
        
        let current_ledger = env.ledger().sequence();
        let end_ledger = current_ledger
            .checked_add(duration_ledgers)
            .ok_or(ContractError::Overflow)?;
        
        let round = Round {
            price_start: start_price,
            end_ledger,
            pool_up: 0,
            pool_down: 0,
        };
        
        env.storage().persistent().set(&DataKey::ActiveRound, &round);
        
        Ok(())
    }
    
    /// Returns the currently active round, if any
    pub fn get_active_round(env: Env) -> Option<Round> {
        env.storage().persistent().get(&DataKey::ActiveRound)
    }
    
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }
    
    pub fn get_oracle(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Oracle)
    }
    
    /// Returns user statistics (wins, losses, streaks)
    pub fn get_user_stats(env: Env, user: Address) -> UserStats {
        let key = DataKey::UserStats(user);
        env.storage().persistent().get(&key).unwrap_or(UserStats {
            total_wins: 0,
            total_losses: 0,
            current_streak: 0,
            best_streak: 0,
        })
    }
    
    /// Returns user's claimable winnings
    pub fn get_pending_winnings(env: Env, user: Address) -> i128 {
        let key = DataKey::PendingWinnings(user);
        env.storage().persistent().get(&key).unwrap_or(0)
    }
    
    /// Places a bet on the active round
    pub fn place_bet(env: Env, user: Address, amount: i128, side: BetSide) -> Result<(), ContractError> {
        user.require_auth();
        
        if amount <= 0 {
            return Err(ContractError::InvalidBetAmount);
        }
        
        let mut round: Round = env.storage()
            .persistent()
            .get(&DataKey::ActiveRound)
            .ok_or(ContractError::NoActiveRound)?;
        
        let current_ledger = env.ledger().sequence();
        if current_ledger >= round.end_ledger {
            return Err(ContractError::RoundEnded);
        }
        
        let user_balance = Self::balance(env.clone(), user.clone());
        if user_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }
        
        let mut positions: Map<Address, UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::Positions)
            .unwrap_or(Map::new(&env));
        
        if positions.contains_key(user.clone()) {
            return Err(ContractError::AlreadyBet);
        }
        
        let new_balance = user_balance
            .checked_sub(amount)
            .ok_or(ContractError::Overflow)?;
        Self::_set_balance(&env, user.clone(), new_balance);
        
        let position = UserPosition {
            amount,
            side: side.clone(),
        };
        positions.set(user, position);
        
        match side {
            BetSide::Up => {
                round.pool_up = round.pool_up
                    .checked_add(amount)
                    .ok_or(ContractError::Overflow)?;
            },
            BetSide::Down => {
                round.pool_down = round.pool_down
                    .checked_add(amount)
                    .ok_or(ContractError::Overflow)?;
            },
        }
        
        env.storage().persistent().set(&DataKey::Positions, &positions);
        env.storage().persistent().set(&DataKey::ActiveRound, &round);
        
        Ok(())
    }
    
    /// Returns user's position in the current round
    pub fn get_user_position(env: Env, user: Address) -> Option<UserPosition> {
        let positions: Map<Address, UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::Positions)
            .unwrap_or(Map::new(&env));
        
        positions.get(user)
    }
    
    /// Resolves the round with final price (oracle only)
    /// Winners split losers' pool proportionally; ties get refunds
    pub fn resolve_round(env: Env, final_price: u128) -> Result<(), ContractError> {
        if final_price == 0 {
            return Err(ContractError::InvalidPrice);
        }
        
        let oracle: Address = env.storage()
            .persistent()
            .get(&DataKey::Oracle)
            .ok_or(ContractError::OracleNotSet)?;
        
        oracle.require_auth();
        
        let round: Round = env.storage()
            .persistent()
            .get(&DataKey::ActiveRound)
            .ok_or(ContractError::NoActiveRound)?;
        
        let positions: Map<Address, UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::Positions)
            .unwrap_or(Map::new(&env));
        
        let price_went_up = final_price > round.price_start;
        let price_went_down = final_price < round.price_start;
        let price_unchanged = final_price == round.price_start;
        
        if price_unchanged {
            Self::_record_refunds(&env, positions)?;
        } else if price_went_up {
            Self::_record_winnings(&env, positions, BetSide::Up, round.pool_up, round.pool_down)?;
        } else if price_went_down {
            Self::_record_winnings(&env, positions, BetSide::Down, round.pool_down, round.pool_up)?;
        }
        
        env.storage().persistent().remove(&DataKey::ActiveRound);
        env.storage().persistent().remove(&DataKey::Positions);
        
        Ok(())
    }
    
    /// Claims pending winnings and adds to balance
    pub fn claim_winnings(env: Env, user: Address) -> i128 {
        user.require_auth();
        
        let key = DataKey::PendingWinnings(user.clone());
        let pending: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        
        if pending == 0 {
            return 0;
        }
        
        let current_balance = Self::balance(env.clone(), user.clone());
        let new_balance = current_balance + pending;
        Self::_set_balance(&env, user.clone(), new_balance);
        
        env.storage().persistent().remove(&key);
        
        pending
    }
    
    /// Records refunds when price unchanged
    fn _record_refunds(env: &Env, positions: Map<Address, UserPosition>) -> Result<(), ContractError> {
        let keys: Vec<Address> = positions.keys();
        
        for i in 0..keys.len() {
            if let Some(user) = keys.get(i) {
                if let Some(position) = positions.get(user.clone()) {
                    let key = DataKey::PendingWinnings(user.clone());
                    let existing_pending: i128 = env.storage().persistent().get(&key).unwrap_or(0);
                    let new_pending = existing_pending
                        .checked_add(position.amount)
                        .ok_or(ContractError::Overflow)?;
                    env.storage().persistent().set(&key, &new_pending);
                }
            }
        }
        
        Ok(())
    }
    
    /// Records winnings for winning side
    /// Formula: payout = bet + (bet / winning_pool) * losing_pool
    fn _record_winnings(
        env: &Env,
        positions: Map<Address, UserPosition>,
        winning_side: BetSide,
        winning_pool: i128,
        losing_pool: i128,
    ) -> Result<(), ContractError> {
        if winning_pool == 0 {
            return Ok(());
        }
        
        let keys: Vec<Address> = positions.keys();
        
        for i in 0..keys.len() {
            if let Some(user) = keys.get(i) {
                if let Some(position) = positions.get(user.clone()) {
                    if position.side == winning_side {
                        let share_numerator = position.amount
                            .checked_mul(losing_pool)
                            .ok_or(ContractError::Overflow)?;
                        let share = share_numerator / winning_pool;
                        let payout = position.amount
                            .checked_add(share)
                            .ok_or(ContractError::Overflow)?;
                        
                        let key = DataKey::PendingWinnings(user.clone());
                        let existing_pending: i128 = env.storage().persistent().get(&key).unwrap_or(0);
                        let new_pending = existing_pending
                            .checked_add(payout)
                            .ok_or(ContractError::Overflow)?;
                        env.storage().persistent().set(&key, &new_pending);
                        
                        Self::_update_stats_win(env, user);
                    } else {
                        Self::_update_stats_loss(env, user);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn _update_stats_win(env: &Env, user: Address) {
        let key = DataKey::UserStats(user);
        let mut stats: UserStats = env.storage().persistent().get(&key).unwrap_or(UserStats {
            total_wins: 0,
            total_losses: 0,
            current_streak: 0,
            best_streak: 0,
        });
        
        stats.total_wins += 1;
        stats.current_streak += 1;
        
        if stats.current_streak > stats.best_streak {
            stats.best_streak = stats.current_streak;
        }
        
        env.storage().persistent().set(&key, &stats);
    }
    
    fn _update_stats_loss(env: &Env, user: Address) {
        let key = DataKey::UserStats(user);
        let mut stats: UserStats = env.storage().persistent().get(&key).unwrap_or(UserStats {
            total_wins: 0,
            total_losses: 0,
            current_streak: 0,
            best_streak: 0,
        });
        
        stats.total_losses += 1;
        stats.current_streak = 0;
        
        env.storage().persistent().set(&key, &stats);
    }
    
    /// Mints 1000 vXLM for new users (one-time only)
    pub fn mint_initial(env: Env, user: Address) -> i128 {
        user.require_auth();
        
        let key = DataKey::Balance(user.clone());
        
        if let Some(existing_balance) = env.storage().persistent().get(&key) {
            return existing_balance;
        }
        
        let initial_amount: i128 = 1000_0000000;
        env.storage().persistent().set(&key, &initial_amount);
        
        initial_amount
    }
    
    /// Returns user's vXLM balance
    pub fn balance(env: Env, user: Address) -> i128 {
        let key = DataKey::Balance(user);
        env.storage().persistent().get(&key).unwrap_or(0)
    }
    
    fn _set_balance(env: &Env, user: Address, amount: i128) {
        let key = DataKey::Balance(user);
        env.storage().persistent().set(&key, &amount);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger as _}, Env};

    #[test]
    fn test_mint_initial() {
        // Create a test environment
        let env = Env::default();
        
        // Register our contract in the test environment
        // This deploys the contract to the test blockchain and returns its unique ID
        // Think of it as: installing your app on a test phone before you can use it
        // The () means we're not passing any initialization arguments
        let contract_id = env.register(VirtualTokenContract, ());
        
        // Create a client to interact with the contract
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        // Generate a random test user address
        let user = Address::generate(&env);
        
        // Mock the authorization (in tests, we need to simulate user approval)
        env.mock_all_auths();
        
        // Call mint_initial for the user
        let balance = client.mint_initial(&user);
        
        // Verify the user received 1000 vXLM
        assert_eq!(balance, 1000_0000000);
        
        // Verify we can query the balance
        let queried_balance = client.balance(&user);
        assert_eq!(queried_balance, 1000_0000000);
    }
    
    #[test]
    fn test_mint_initial_only_once() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);
        
        env.mock_all_auths();
        
        // First mint
        let first_mint = client.mint_initial(&user);
        assert_eq!(first_mint, 1000_0000000);
        
        // Try to mint again - should return existing balance, not mint more
        let second_mint = client.mint_initial(&user);
        assert_eq!(second_mint, 1000_0000000);
        
        // Balance should still be 1000, not 2000
        let balance = client.balance(&user);
        assert_eq!(balance, 1000_0000000);
    }
    
    #[test]
    fn test_balance_for_new_user() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);
        
        // Query balance for a user who never minted
        let balance = client.balance(&user);
        
        // Should return 0
        assert_eq!(balance, 0);
    }
    
    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        // Generate an admin and oracle address
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        
        env.mock_all_auths();
        
        // Initialize the contract
        client.initialize(&admin, &oracle);
        
        // Verify admin and oracle are set
        let stored_admin = client.get_admin();
        let stored_oracle = client.get_oracle();
        assert_eq!(stored_admin, Some(admin));
        assert_eq!(stored_oracle, Some(oracle));
    }
    
    #[test]
    fn test_initialize_twice_fails() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        
        env.mock_all_auths();
        
        // Initialize once
        client.initialize(&admin, &oracle);
        
        // Try to initialize again - should return error
        let result = client.try_initialize(&admin, &oracle);
        assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
    }
    
    #[test]
    fn test_create_round() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        // Set up admin
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        env.mock_all_auths();
        client.initialize(&admin, &oracle);
        
        // Create a round
        let start_price: u128 = 1_5000000; // 1.5 XLM in stroops
        let duration: u32 = 60; // 60 ledgers
        
        client.create_round(&start_price, &duration);
        
        // Verify the round was created
        let round = client.get_active_round().expect("Round should exist");
        
        assert_eq!(round.price_start, start_price);
        assert_eq!(round.pool_up, 0);
        assert_eq!(round.pool_down, 0);
        
        // Verify end_ledger is set correctly (current ledger + duration)
        // Note: In tests, current ledger starts at 0
        assert_eq!(round.end_ledger, duration);
    }
    
    #[test]
    fn test_create_round_without_init_fails() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        env.mock_all_auths();
        
        // Try to create round without initializing - should return error
        let result = client.try_create_round(&1_0000000, &60);
        assert_eq!(result, Err(Ok(ContractError::AdminNotSet)));
    }
    
    #[test]
    fn test_get_active_round_when_none() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        // No round created yet
        let round = client.get_active_round();
        
        assert_eq!(round, None);
    }
    
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
        client.create_round(&start_price, &60);
        
        // Manually set up some test positions using env.as_contract
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        
        // Give users initial balances
        client.mint_initial(&user1);
        client.mint_initial(&user2);
        
        // Manually create positions for testing using as_contract
        env.as_contract(&contract_id, || {
            let mut positions = Map::<Address, UserPosition>::new(&env);
            positions.set(user1.clone(), UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            });
            positions.set(user2.clone(), UserPosition {
                amount: 50_0000000,
                side: BetSide::Down,
            });
            
            // Store positions
            env.storage().persistent().set(&DataKey::Positions, &positions);
            
            // Update round pools to match positions
            let mut round: Round = env.storage().persistent().get(&DataKey::ActiveRound).unwrap();
            round.pool_up = 100_0000000;
            round.pool_down = 50_0000000;
            env.storage().persistent().set(&DataKey::ActiveRound, &round);
        });
        
        // Get balances before resolution
        let user1_balance_before = client.balance(&user1);
        let user2_balance_before = client.balance(&user2);
        
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
        client.create_round(&start_price, &60);
        
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
            positions.set(alice.clone(), UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            });
            positions.set(bob.clone(), UserPosition {
                amount: 200_0000000,
                side: BetSide::Up,
            });
            positions.set(charlie.clone(), UserPosition {
                amount: 150_0000000,
                side: BetSide::Down,
            });
            
            env.storage().persistent().set(&DataKey::Positions, &positions);
            
            let mut round: Round = env.storage().persistent().get(&DataKey::ActiveRound).unwrap();
            round.pool_up = 300_0000000;
            round.pool_down = 150_0000000;
            env.storage().persistent().set(&DataKey::ActiveRound, &round);
        });
        
        let alice_before = client.balance(&alice);
        let bob_before = client.balance(&bob);
        let charlie_before = client.balance(&charlie);
        
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
        client.create_round(&start_price, &60);
        
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        
        client.mint_initial(&alice);
        client.mint_initial(&bob);
        
        // Create positions using as_contract
        env.as_contract(&contract_id, || {
            let mut positions = Map::<Address, UserPosition>::new(&env);
            positions.set(alice.clone(), UserPosition {
                amount: 200_0000000,
                side: BetSide::Down,
            });
            positions.set(bob.clone(), UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            });
            
            env.storage().persistent().set(&DataKey::Positions, &positions);
            
            let mut round: Round = env.storage().persistent().get(&DataKey::ActiveRound).unwrap();
            round.pool_up = 100_0000000;
            round.pool_down = 200_0000000;
            env.storage().persistent().set(&DataKey::ActiveRound, &round);
        });
        
        let alice_before = client.balance(&alice);
        let bob_before = client.balance(&bob);
        
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
    
    // ============================================
    // FULL LIFECYCLE TESTS
    // ============================================
    
    #[test]
    fn test_full_round_lifecycle() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        // Setup
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let charlie = Address::generate(&env);
        
        env.mock_all_auths();
        
        // STEP 1: Initialize contract
        client.initialize(&admin, &oracle);
        
        // STEP 2: Users get initial tokens
        client.mint_initial(&alice);
        client.mint_initial(&bob);
        client.mint_initial(&charlie);
        
        assert_eq!(client.balance(&alice), 1000_0000000);
        assert_eq!(client.balance(&bob), 1000_0000000);
        assert_eq!(client.balance(&charlie), 1000_0000000);
        
        // STEP 3: Admin creates a round
        let start_price: u128 = 1_0000000; // 1.0 XLM
        client.create_round(&start_price, &100);
        
        let round = client.get_active_round().unwrap();
        assert_eq!(round.price_start, start_price);
        assert_eq!(round.pool_up, 0);
        assert_eq!(round.pool_down, 0);
        
        // STEP 4: Users place bets
        client.place_bet(&alice, &100_0000000, &BetSide::Up);
        client.place_bet(&bob, &200_0000000, &BetSide::Up);
        client.place_bet(&charlie, &150_0000000, &BetSide::Down);
        
        // Verify balances deducted
        assert_eq!(client.balance(&alice), 900_0000000);
        assert_eq!(client.balance(&bob), 800_0000000);
        assert_eq!(client.balance(&charlie), 850_0000000);
        
        // Verify positions recorded
        let alice_pos = client.get_user_position(&alice).unwrap();
        assert_eq!(alice_pos.amount, 100_0000000);
        assert_eq!(alice_pos.side, BetSide::Up);
        
        // Verify pools updated
        let round = client.get_active_round().unwrap();
        assert_eq!(round.pool_up, 300_0000000);
        assert_eq!(round.pool_down, 150_0000000);
        
        // STEP 5: Oracle resolves round (price went UP)
        let final_price: u128 = 1_5000000; // 1.5 XLM
        client.resolve_round(&final_price);
        
        // Round should be cleared
        assert_eq!(client.get_active_round(), None);
        
        // STEP 6: Verify pending winnings
        // Alice: 100 + (100/300)*150 = 150
        // Bob: 200 + (200/300)*150 = 300
        // Charlie: 0 (lost)
        assert_eq!(client.get_pending_winnings(&alice), 150_0000000);
        assert_eq!(client.get_pending_winnings(&bob), 300_0000000);
        assert_eq!(client.get_pending_winnings(&charlie), 0);
        
        // STEP 7: Verify stats updated
        let alice_stats = client.get_user_stats(&alice);
        assert_eq!(alice_stats.total_wins, 1);
        assert_eq!(alice_stats.current_streak, 1);
        
        let charlie_stats = client.get_user_stats(&charlie);
        assert_eq!(charlie_stats.total_losses, 1);
        assert_eq!(charlie_stats.current_streak, 0);
        
        // STEP 8: Users claim winnings
        let alice_claimed = client.claim_winnings(&alice);
        let bob_claimed = client.claim_winnings(&bob);
        
        assert_eq!(alice_claimed, 150_0000000);
        assert_eq!(bob_claimed, 300_0000000);
        
        // STEP 9: Verify final balances
        assert_eq!(client.balance(&alice), 1050_0000000); // 900 + 150
        assert_eq!(client.balance(&bob), 1100_0000000);   // 800 + 300
        assert_eq!(client.balance(&charlie), 850_0000000); // Lost 150
        
        // STEP 10: Pending winnings cleared
        assert_eq!(client.get_pending_winnings(&alice), 0);
        assert_eq!(client.get_pending_winnings(&bob), 0);
    }
    
    #[test]
    fn test_multiple_rounds_lifecycle() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let alice = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        client.mint_initial(&alice);
        
        // ROUND 1: Alice bets UP and wins
        client.create_round(&1_0000000, &100);
        client.place_bet(&alice, &100_0000000, &BetSide::Up);
        
        env.as_contract(&contract_id, || {
            let mut positions = Map::<Address, UserPosition>::new(&env);
            positions.set(alice.clone(), UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            });
            env.storage().persistent().set(&DataKey::Positions, &positions);
            
            let mut round: Round = env.storage().persistent().get(&DataKey::ActiveRound).unwrap();
            round.pool_up = 100_0000000;
            round.pool_down = 50_0000000;
            env.storage().persistent().set(&DataKey::ActiveRound, &round);
        });
        
        client.resolve_round(&1_5000000); // UP wins
        client.claim_winnings(&alice);
        
        let stats = client.get_user_stats(&alice);
        assert_eq!(stats.total_wins, 1);
        assert_eq!(stats.current_streak, 1);
        
        // ROUND 2: Alice bets DOWN and wins again
        client.create_round(&2_0000000, &100);
        client.place_bet(&alice, &100_0000000, &BetSide::Down);
        
        env.as_contract(&contract_id, || {
            let mut positions = Map::<Address, UserPosition>::new(&env);
            positions.set(alice.clone(), UserPosition {
                amount: 100_0000000,
                side: BetSide::Down,
            });
            env.storage().persistent().set(&DataKey::Positions, &positions);
            
            let mut round: Round = env.storage().persistent().get(&DataKey::ActiveRound).unwrap();
            round.pool_up = 80_0000000;
            round.pool_down = 100_0000000;
            env.storage().persistent().set(&DataKey::ActiveRound, &round);
        });
        
        client.resolve_round(&1_5000000); // DOWN wins
        
        let stats = client.get_user_stats(&alice);
        assert_eq!(stats.total_wins, 2);
        assert_eq!(stats.current_streak, 2);
        assert_eq!(stats.best_streak, 2);
    }
    
    // ============================================
    // EDGE CASE TESTS
    // ============================================
    
    #[test]
    fn test_place_bet_zero_amount() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let user = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        client.mint_initial(&user);
        client.create_round(&1_0000000, &100);
        
        // Try to bet 0 amount - should return error
        let result = client.try_place_bet(&user, &0, &BetSide::Up);
        assert_eq!(result, Err(Ok(ContractError::InvalidBetAmount)));
    }
    
    #[test]
    fn test_place_bet_negative_amount() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let user = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        client.mint_initial(&user);
        client.create_round(&1_0000000, &100);
        
        // Try to bet negative amount - should return error
        let result = client.try_place_bet(&user, &-100, &BetSide::Up);
        assert_eq!(result, Err(Ok(ContractError::InvalidBetAmount)));
    }
    
    #[test]
    fn test_place_bet_no_active_round() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let user = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        client.mint_initial(&user);
        
        // Try to bet without active round - should return error
        let result = client.try_place_bet(&user, &100_0000000, &BetSide::Up);
        assert_eq!(result, Err(Ok(ContractError::NoActiveRound)));
    }
    
    #[test]
    fn test_place_bet_after_round_ended() {
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
        
        // Create round that ends at ledger 50
        client.create_round(&1_0000000, &50);
        
        // Advance ledger past end time
        env.ledger().with_mut(|li| {
            li.sequence_number = 100;
        });
        
        // Try to bet after round ended - should return error
        let result = client.try_place_bet(&user, &100_0000000, &BetSide::Up);
        assert_eq!(result, Err(Ok(ContractError::RoundEnded)));
    }
    
    #[test]
    fn test_place_bet_insufficient_balance() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let user = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        client.mint_initial(&user); // Has 1000 vXLM
        client.create_round(&1_0000000, &100);
        
        // Try to bet more than balance - should return error
        let result = client.try_place_bet(&user, &2000_0000000, &BetSide::Up);
        assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
    }
    
    #[test]
    fn test_place_bet_twice_same_round() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let user = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        client.mint_initial(&user);
        client.create_round(&1_0000000, &100);
        
        // First bet succeeds
        client.place_bet(&user, &100_0000000, &BetSide::Up);
        
        // Second bet should fail with error
        let result = client.try_place_bet(&user, &50_0000000, &BetSide::Down);
        assert_eq!(result, Err(Ok(ContractError::AlreadyBet)));
    }
    
    #[test]
    fn test_round_with_no_participants() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        
        // Create round with no bets
        client.create_round(&1_0000000, &100);
        
        let round = client.get_active_round().unwrap();
        assert_eq!(round.pool_up, 0);
        assert_eq!(round.pool_down, 0);
        
        // Resolve with no participants
        client.resolve_round(&1_5000000);
        
        // Should clear round without errors
        assert_eq!(client.get_active_round(), None);
    }
    
    #[test]
    fn test_round_with_only_one_side() {
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
        
        // Create round and only bet on UP
        client.create_round(&1_0000000, &100);
        client.place_bet(&alice, &100_0000000, &BetSide::Up);
        client.place_bet(&bob, &150_0000000, &BetSide::Up);
        
        let round = client.get_active_round().unwrap();
        assert_eq!(round.pool_up, 250_0000000);
        assert_eq!(round.pool_down, 0);
        
        // Resolve - UP wins but no losers to take from
        client.resolve_round(&1_5000000);
        
        // Winners should only get their bets back (no losing pool to split)
        assert_eq!(client.get_pending_winnings(&alice), 100_0000000);
        assert_eq!(client.get_pending_winnings(&bob), 150_0000000);
    }
    
    #[test]
    fn test_get_user_position_no_bet() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let user = Address::generate(&env);
        
        // No position should return None
        let position = client.get_user_position(&user);
        assert_eq!(position, None);
    }
    
    #[test]
    fn test_accumulate_pending_winnings() {
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let alice = Address::generate(&env);
        
        env.mock_all_auths();
        
        client.initialize(&admin, &oracle);
        client.mint_initial(&alice);
        
        // Round 1: Alice bets UP and wins
        client.create_round(&1_0000000, &100);
        client.place_bet(&alice, &100_0000000, &BetSide::Up);
        
        env.as_contract(&contract_id, || {
            let mut positions = Map::<Address, UserPosition>::new(&env);
            positions.set(alice.clone(), UserPosition {
                amount: 100_0000000,
                side: BetSide::Up,
            });
            env.storage().persistent().set(&DataKey::Positions, &positions);
            
            let mut round: Round = env.storage().persistent().get(&DataKey::ActiveRound).unwrap();
            round.pool_up = 100_0000000;
            round.pool_down = 50_0000000;
            env.storage().persistent().set(&DataKey::ActiveRound, &round);
        });
        
        client.resolve_round(&1_5000000); // UP wins
        
        let first_pending = client.get_pending_winnings(&alice);
        assert!(first_pending > 0);
        
        // Round 2: Alice bets and gets refund
        client.create_round(&2_0000000, &100);
        client.place_bet(&alice, &50_0000000, &BetSide::Down);
        
        client.resolve_round(&2_0000000); // Price unchanged - refund
        
        // Should have accumulated pending from both rounds
        let total_pending = client.get_pending_winnings(&alice);
        assert_eq!(total_pending, first_pending + 50_0000000);
        
        // Claim all at once
        let claimed = client.claim_winnings(&alice);
        assert_eq!(claimed, total_pending);
        assert_eq!(client.get_pending_winnings(&alice), 0);
    }
}
