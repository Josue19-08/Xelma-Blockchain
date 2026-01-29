//! Core contract implementation for the XLM Price Prediction Market.

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Map, Vec};

use crate::errors::ContractError;
use crate::types::{BetSide, DataKey, PrecisionPrediction, Round, RoundMode, UserPosition, UserStats};

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
        
        // Set default window values
        env.storage().persistent().set(&DataKey::BetWindowLedgers, &6u32);
        env.storage().persistent().set(&DataKey::RunWindowLedgers, &12u32);
        
        Ok(())
    }
    
    /// Creates a new prediction round (admin only)
    /// mode: 0 = Up/Down (default), 1 = Precision (Legends)
    pub fn create_round(env: Env, start_price: u128, mode: Option<u32>) -> Result<(), ContractError> {
        if start_price == 0 {
            return Err(ContractError::InvalidPrice);
        }

        // Default to Up/Down mode (0) if not specified
        let mode_value = mode.unwrap_or(0);

        // Validate mode is either 0 or 1
        if mode_value > 1 {
            return Err(ContractError::InvalidMode);
        }

        let round_mode = if mode_value == 0 {
            RoundMode::UpDown
        } else {
            RoundMode::Precision
        };

        let admin: Address = env.storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;

        admin.require_auth();

        // Get configured windows (with defaults)
        let bet_ledgers: u32 = env.storage()
            .persistent()
            .get(&DataKey::BetWindowLedgers)
            .unwrap_or(6);
        let run_ledgers: u32 = env.storage()
            .persistent()
            .get(&DataKey::RunWindowLedgers)
            .unwrap_or(12);

        let start_ledger = env.ledger().sequence();
        let bet_end_ledger = start_ledger
            .checked_add(bet_ledgers)
            .ok_or(ContractError::Overflow)?;
        let end_ledger = start_ledger
            .checked_add(run_ledgers)
            .ok_or(ContractError::Overflow)?;

        let round = Round {
            price_start: start_price,
            start_ledger,
            bet_end_ledger,
            end_ledger,
            pool_up: 0,
            pool_down: 0,
            mode: round_mode.clone(),
        };

        env.storage().persistent().set(&DataKey::ActiveRound, &round);

        // Clear previous round's positions based on mode
        env.storage().persistent().remove(&DataKey::UpDownPositions);
        env.storage().persistent().remove(&DataKey::PrecisionPositions);

        // Emit round creation event with mode
        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("round"), symbol_short!("created")),
            (start_price, bet_end_ledger, end_ledger, mode_value),
        );

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
    
    /// Sets the betting and execution windows (admin only)
    /// bet_ledgers: Number of ledgers users can place bets
    /// run_ledgers: Total number of ledgers before round can be resolved
    pub fn set_windows(env: Env, bet_ledgers: u32, run_ledgers: u32) -> Result<(), ContractError> {
        let admin: Address = env.storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;
        
        admin.require_auth();
        
        // Validate both values are positive
        if bet_ledgers == 0 || run_ledgers == 0 {
            return Err(ContractError::InvalidDuration);
        }
        
        // Validate bet window closes before run window ends
        if bet_ledgers >= run_ledgers {
            return Err(ContractError::InvalidDuration);
        }
        
        env.storage().persistent().set(&DataKey::BetWindowLedgers, &bet_ledgers);
        env.storage().persistent().set(&DataKey::RunWindowLedgers, &run_ledgers);
        
        // Emit event
        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("windows"), symbol_short!("updated")),
            (bet_ledgers, run_ledgers),
        );
        
        Ok(())
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
    
    /// Places a bet on the active round (Up/Down mode only)
    pub fn place_bet(env: Env, user: Address, amount: i128, side: BetSide) -> Result<(), ContractError> {
        user.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidBetAmount);
        }

        let mut round: Round = env.storage()
            .persistent()
            .get(&DataKey::ActiveRound)
            .ok_or(ContractError::NoActiveRound)?;

        // Verify round is in Up/Down mode
        if round.mode != RoundMode::UpDown {
            return Err(ContractError::WrongModeForPrediction);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger >= round.bet_end_ledger {
            return Err(ContractError::RoundEnded);
        }

        let user_balance = Self::balance(env.clone(), user.clone());
        if user_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Use UpDownPositions storage for Up/Down mode
        let mut positions: Map<Address, UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::UpDownPositions)
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
        positions.set(user.clone(), position);

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

        env.storage().persistent().set(&DataKey::UpDownPositions, &positions);
        env.storage().persistent().set(&DataKey::ActiveRound, &round);

        // Also keep legacy Positions storage for backwards compatibility
        let mut legacy_positions: Map<Address, UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::Positions)
            .unwrap_or(Map::new(&env));
        legacy_positions.set(user, UserPosition { amount, side });
        env.storage().persistent().set(&DataKey::Positions, &legacy_positions);

        Ok(())
    }

    /// Places a precision prediction on the active round (Precision/Legends mode only)
    /// predicted_price: price scaled to 4 decimals (e.g., 0.2297 → 2297)
    pub fn place_precision_prediction(
        env: Env,
        user: Address,
        amount: i128,
        predicted_price: u128,
    ) -> Result<(), ContractError> {
        user.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidBetAmount);
        }

        // Validate price scale (must be 4 decimal places, max value 9999 for 0.9999)
        // Reasonable max: 99999999 (9999.9999 XLM)
        if predicted_price > 99_999_999 {
            return Err(ContractError::InvalidPriceScale);
        }

        let round: Round = env.storage()
            .persistent()
            .get(&DataKey::ActiveRound)
            .ok_or(ContractError::NoActiveRound)?;

        // Verify round is in Precision mode
        if round.mode != RoundMode::Precision {
            return Err(ContractError::WrongModeForPrediction);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger >= round.bet_end_ledger {
            return Err(ContractError::RoundEnded);
        }

        let user_balance = Self::balance(env.clone(), user.clone());
        if user_balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Check if user already has a prediction in this round
        let mut predictions: Vec<PrecisionPrediction> = env.storage()
            .persistent()
            .get(&DataKey::PrecisionPositions)
            .unwrap_or(Vec::new(&env));

        for i in 0..predictions.len() {
            if let Some(pred) = predictions.get(i) {
                if pred.user == user {
                    return Err(ContractError::AlreadyBet);
                }
            }
        }

        // Deduct balance
        let new_balance = user_balance
            .checked_sub(amount)
            .ok_or(ContractError::Overflow)?;
        Self::_set_balance(&env, user.clone(), new_balance);

        // Store prediction
        let prediction = PrecisionPrediction {
            user: user.clone(),
            predicted_price,
            amount,
        };
        predictions.push_back(prediction);

        env.storage().persistent().set(&DataKey::PrecisionPositions, &predictions);

        // Emit event for precision prediction
        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("predict"), symbol_short!("price")),
            (user, predicted_price, round.start_ledger),
        );

        Ok(())
    }

    /// Alias for place_precision_prediction - allows users to submit exact price predictions
    /// guessed_price: price scaled to 4 decimals (e.g., 0.2297 → 2297)
    pub fn predict_price(
        env: Env,
        user: Address,
        guessed_price: u128,
        amount: i128,
    ) -> Result<(), ContractError> {
        Self::place_precision_prediction(env, user, amount, guessed_price)
    }
    
    /// Returns user's position in the current round (Up/Down mode)
    pub fn get_user_position(env: Env, user: Address) -> Option<UserPosition> {
        let positions: Map<Address, UserPosition> = env.storage()
            .persistent()
            .get(&DataKey::UpDownPositions)
            .unwrap_or(Map::new(&env));

        positions.get(user)
    }

    /// Returns user's precision prediction in the current round (Precision mode)
    pub fn get_user_precision_prediction(env: Env, user: Address) -> Option<PrecisionPrediction> {
        let predictions: Vec<PrecisionPrediction> = env.storage()
            .persistent()
            .get(&DataKey::PrecisionPositions)
            .unwrap_or(Vec::new(&env));

        for i in 0..predictions.len() {
            if let Some(pred) = predictions.get(i) {
                if pred.user == user {
                    return Some(pred);
                }
            }
        }
        None
    }

    /// Returns all precision predictions for the current round
    pub fn get_precision_predictions(env: Env) -> Vec<PrecisionPrediction> {
        env.storage()
            .persistent()
            .get(&DataKey::PrecisionPositions)
            .unwrap_or(Vec::new(&env))
    }

    /// Returns all Up/Down positions for the current round
    pub fn get_updown_positions(env: Env) -> Map<Address, UserPosition> {
        env.storage()
            .persistent()
            .get(&DataKey::UpDownPositions)
            .unwrap_or(Map::new(&env))
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
        
        // Verify round has reached end_ledger
        let current_ledger = env.ledger().sequence();
        if current_ledger < round.end_ledger {
            return Err(ContractError::RoundNotEnded);
        }
        
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
        env.storage().persistent().remove(&DataKey::UpDownPositions);
        env.storage().persistent().remove(&DataKey::PrecisionPositions);

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
    
    pub(crate) fn _update_stats_win(env: &Env, user: Address) {
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
    
    pub(crate) fn _update_stats_loss(env: &Env, user: Address) {
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
    
    pub(crate) fn _set_balance(env: &Env, user: Address, amount: i128) {
        let key = DataKey::Balance(user);
        env.storage().persistent().set(&key, &amount);
    }
}

