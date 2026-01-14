#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

/// Storage keys for organizing data in the contract
/// Think of these as "labels" for different storage compartments
/// 
/// The #[contracttype] attribute tells Soroban this can be stored in the contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Stores the balance for a specific user address
    Balance(Address),
}

/// The main contract structure
/// This represents your vXLM (virtual XLM) token contract
#[contract]
pub struct VirtualTokenContract;

#[contractimpl]
impl VirtualTokenContract {
    /// Mints (creates) initial vXLM tokens for a user on their first interaction
    /// 
    /// # Parameters
    /// * `env` - The contract environment (provided by Soroban, gives access to storage, etc.)
    /// * `user` - The address of the user who will receive tokens
    /// 
    /// # How it works
    /// 1. Checks if user already has a balance
    /// 2. If not, gives them 1000 vXLM as a starting amount
    /// 3. Stores this balance in the contract's persistent storage
    pub fn mint_initial(env: Env, user: Address) -> i128 {
        // Verify that the user is authorized to call this function
        // This ensures only the user themselves can mint tokens for their account
        user.require_auth();
        
        // Create a storage key for this user's balance
        let key = DataKey::Balance(user.clone());
        
        // Check if the user already has a balance
        // get() returns an Option: Some(balance) if exists, None if not
        if let Some(existing_balance) = env.storage().persistent().get(&key) {
            // User already has tokens, return their existing balance
            return existing_balance;
        }
        
        // User is new! Give them 1000 vXLM as initial tokens
        // Note: We use 1000_0000000 because Stellar uses 7 decimal places (stroops)
        let initial_amount: i128 = 1000_0000000; // 1000 vXLM
        
        // Save the balance to persistent storage
        // This data will remain even after the transaction completes
        env.storage().persistent().set(&key, &initial_amount);
        
        // Return the newly minted amount
        initial_amount
    }
    
    /// Queries (reads) the current vXLM balance for a user
    /// 
    /// # Parameters
    /// * `env` - The contract environment
    /// * `user` - The address of the user whose balance we want to check
    /// 
    /// # Returns
    /// The user's balance as an i128 (128-bit integer)
    /// Returns 0 if the user has never received tokens
    pub fn balance(env: Env, user: Address) -> i128 {
        // Create the storage key for this user
        let key = DataKey::Balance(user);
        
        // Try to get the balance from storage
        // unwrap_or(0) means: if balance exists, use it; otherwise, return 0
        env.storage().persistent().get(&key).unwrap_or(0)
    }
    
    /// Internal helper function to update a user's balance
    /// The underscore prefix means this is a private/internal function
    /// 
    /// # Parameters
    /// * `env` - The contract environment
    /// * `user` - The address whose balance to update
    /// * `amount` - The new balance amount
    fn _set_balance(env: &Env, user: Address, amount: i128) {
        let key = DataKey::Balance(user);
        env.storage().persistent().set(&key, &amount);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

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
}
