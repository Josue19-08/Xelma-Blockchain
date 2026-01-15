import { Buffer } from "buffer";
import { Address } from '@stellar/stellar-sdk';
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from '@stellar/stellar-sdk/contract';
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Typepoint,
  Duration,
} from '@stellar/stellar-sdk/contract';
export * from '@stellar/stellar-sdk'
export * as contract from '@stellar/stellar-sdk/contract'
export * as rpc from '@stellar/stellar-sdk/rpc'

if (typeof window !== 'undefined') {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}





/**
 * Represents a prediction round
 * This stores all the information about an active betting round
 */
export interface Round {
  /**
 * The ledger number when this round ends
 * Ledgers are like blocks in blockchain - they increment every ~5 seconds
 */
end_ledger: u32;
  /**
 * Total vXLM in the "DOWN" pool (people betting price will go down)
 */
pool_down: i128;
  /**
 * Total vXLM in the "UP" pool (people betting price will go up)
 */
pool_up: i128;
  /**
 * The starting price of XLM when the round begins (in stroops)
 */
price_start: u128;
}

/**
 * Represents which side a user bet on
 */
export type BetSide = {tag: "Up", values: void} | {tag: "Down", values: void};

/**
 * Storage keys for organizing data in the contract
 * Think of these as "labels" for different storage compartments
 * 
 * The #[contracttype] attribute tells Soroban this can be stored in the contract
 */
export type DataKey = {tag: "Balance", values: readonly [string]} | {tag: "Admin", values: void} | {tag: "Oracle", values: void} | {tag: "ActiveRound", values: void} | {tag: "Positions", values: void} | {tag: "PendingWinnings", values: readonly [string]} | {tag: "UserStats", values: readonly [string]};


/**
 * Tracks a user's prediction performance
 */
export interface UserStats {
  /**
 * Best winning streak ever achieved
 */
best_streak: u32;
  /**
 * Current winning streak (consecutive wins)
 */
current_streak: u32;
  /**
 * Total number of rounds lost
 */
total_losses: u32;
  /**
 * Total number of rounds won
 */
total_wins: u32;
}


/**
 * Stores an individual user's bet in a round
 */
export interface UserPosition {
  /**
 * How much vXLM the user bet
 */
amount: i128;
  /**
 * Which side they bet on
 */
side: BetSide;
}

/**
 * Custom error types for the contract
 * Using explicit error codes helps with debugging and provides clear feedback
 */
export const ContractError = {
  /**
   * Contract has already been initialized
   */
  1: {message:"AlreadyInitialized"},
  /**
   * Admin address not set - call initialize first
   */
  2: {message:"AdminNotSet"},
  /**
   * Oracle address not set - call initialize first
   */
  3: {message:"OracleNotSet"},
  /**
   * Only admin can perform this action
   */
  4: {message:"UnauthorizedAdmin"},
  /**
   * Only oracle can perform this action
   */
  5: {message:"UnauthorizedOracle"},
  /**
   * Bet amount must be greater than zero
   */
  6: {message:"InvalidBetAmount"},
  /**
   * No active round exists
   */
  7: {message:"NoActiveRound"},
  /**
   * Round has already ended
   */
  8: {message:"RoundEnded"},
  /**
   * User has insufficient balance
   */
  9: {message:"InsufficientBalance"},
  /**
   * User has already placed a bet in this round
   */
  10: {message:"AlreadyBet"},
  /**
   * Arithmetic overflow occurred
   */
  11: {message:"Overflow"},
  /**
   * Invalid price value
   */
  12: {message:"InvalidPrice"},
  /**
   * Invalid duration value
   */
  13: {message:"InvalidDuration"}
}

export interface Client {
  /**
   * Construct and simulate a balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Queries (reads) the current vXLM balance for a user
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `user` - The address of the user whose balance we want to check
   * 
   * # Returns
   * The user's balance as an i128 (128-bit integer)
   * Returns 0 if the user has never received tokens
   */
  balance: ({user}: {user: string}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gets the admin address
   * 
   * # Returns
   * Option<Address> - Some(admin) if set, None if not initialized
   */
  get_admin: (options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a place_bet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Places a bet on the active round
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `user` - The address of the user placing the bet
   * * `amount` - Amount of vXLM to bet (must be > 0)
   * * `side` - Which side to bet on (Up or Down)
   * 
   * # Security
   * - Requires user authorization (prevents unauthorized betting)
   * - Validates bet amount is positive
   * - Checks round is still active (prevents late bets)
   * - Verifies sufficient balance (prevents negative balances)
   * - Prevents double betting in same round
   * - Uses checked arithmetic to prevent overflow
   * - No reentrancy risk: state updates before external calls (CEI pattern)
   * 
   * # Errors
   * - `ContractError::InvalidBetAmount` if amount <= 0
   * - `ContractError::NoActiveRound` if no round exists
   * - `ContractError::RoundEnded` if round has ended
   * - `ContractError::InsufficientBalance` if user balance too low
   * - `ContractError::AlreadyBet` if user already bet in this round
   * - `ContractError::Overflow` if pool calculation overflows
   */
  place_bet: ({user, amount, side}: {user: string, amount: i128, side: BetSide}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_oracle transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gets the oracle address
   * 
   * # Returns
   * Option<Address> - Some(oracle) if set, None if not initialized
   */
  get_oracle: (options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initializes the contract by setting the admin and oracle
   * This should be called once when deploying the contract
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `admin` - The address that will have admin privileges (creates rounds)
   * * `oracle` - The address that provides price data and resolves rounds
   * 
   * # Security
   * - Prevents re-initialization attacks
   * - Requires admin authorization
   * - Admin and oracle cannot be the same (separation of concerns)
   * 
   * # Errors
   * Returns `ContractError::AlreadyInitialized` if contract was already initialized
   */
  initialize: ({admin, oracle}: {admin: string, oracle: string}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a create_round transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Creates a new prediction round
   * Only the admin can call this function
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `start_price` - The current XLM price in stroops (e.g., 1 XLM = 10,000,000 stroops)
   * * `duration_ledgers` - How many ledgers (blocks) the round should last
   * Example: 60 ledgers ≈ 5 minutes (since ledgers are ~5 seconds)
   * 
   * # Security
   * - Only admin can create rounds (prevents unauthorized round creation)
   * - Validates price is non-zero
   * - Validates duration is reasonable (prevents DoS)
   * - Checks for overflow when calculating end_ledger
   * 
   * # Errors
   * - `ContractError::AdminNotSet` if contract not initialized
   * - `ContractError::InvalidPrice` if start_price is 0
   * - `ContractError::InvalidDuration` if duration is 0 or too large
   * - `ContractError::Overflow` if end_ledger calculation overflows
   */
  create_round: ({start_price, duration_ledgers}: {start_price: u128, duration_ledgers: u32}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a mint_initial transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Mints (creates) initial vXLM tokens for a user on their first interaction
   * 
   * # Parameters
   * * `env` - The contract environment (provided by Soroban, gives access to storage, etc.)
   * * `user` - The address of the user who will receive tokens
   * 
   * # How it works
   * 1. Checks if user already has a balance
   * 2. If not, gives them 1000 vXLM as a starting amount
   * 3. Stores this balance in the contract's persistent storage
   */
  mint_initial: ({user}: {user: string}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a resolve_round transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Resolves a round with the final price and calculates winnings
   * Only the oracle can call this function
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `final_price` - The XLM price at round end (in stroops)
   * 
   * # Security
   * - Only oracle can resolve (prevents unauthorized resolution)
   * - Validates final price is non-zero
   * - Uses checked arithmetic in payout calculations
   * - No reentrancy: state cleared after all calculations
   * - Proportional distribution prevents manipulation
   * 
   * # Errors
   * - `ContractError::OracleNotSet` if oracle not configured
   * - `ContractError::NoActiveRound` if no round to resolve
   * - `ContractError::InvalidPrice` if final_price is 0
   * 
   * # Payout logic
   * - If price went UP: UP bettors split the DOWN pool proportionally
   * - If price went DOWN: DOWN bettors split the UP pool proportionally
   * - If price UNCHANGED: Everyone gets their bet back (no winners/losers)
   */
  resolve_round: ({final_price}: {final_price: u128}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_winnings transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claims pending winnings for a user
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `user` - The address claiming winnings
   * 
   * # How it works
   * 1. Check if user has pending winnings
   * 2. Add winnings to user's balance
   * 3. Clear pending winnings
   * 
   * # Returns
   * Amount claimed (0 if no pending winnings)
   */
  claim_winnings: ({user}: {user: string}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_user_stats transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gets a user's statistics (wins, losses, streaks)
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `user` - The address of the user
   * 
   * # Returns
   * UserStats if the user has participated, or default stats (all zeros)
   */
  get_user_stats: ({user}: {user: string}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<UserStats>>

  /**
   * Construct and simulate a get_active_round transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gets the currently active round
   * 
   * # Returns
   * Option<Round> - Some(round) if there's an active round, None if not
   * 
   * # Use case
   * Frontend can call this to display current round info to users
   */
  get_active_round: (options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Option<Round>>>

  /**
   * Construct and simulate a get_user_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gets a user's position in the current round
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `user` - The address of the user
   * 
   * # Returns
   * Option<UserPosition> - Some(position) if user has bet, None if not
   */
  get_user_position: ({user}: {user: string}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<Option<UserPosition>>>

  /**
   * Construct and simulate a get_pending_winnings transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gets a user's pending winnings (amount they can claim)
   * 
   * # Parameters
   * * `env` - The contract environment
   * * `user` - The address of the user
   * 
   * # Returns
   * Amount of vXLM the user can claim (0 if none)
   */
  get_pending_winnings: ({user}: {user: string}, options?: {
    /**
     * The fee to pay for the transaction. Default: BASE_FEE
     */
    fee?: number;

    /**
     * The maximum amount of time to wait for the transaction to complete. Default: DEFAULT_TIMEOUT
     */
    timeoutInSeconds?: number;

    /**
     * Whether to automatically simulate the transaction when constructing the AssembledTransaction. Default: true
     */
    simulate?: boolean;
  }) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAQAAAFtSZXByZXNlbnRzIGEgcHJlZGljdGlvbiByb3VuZApUaGlzIHN0b3JlcyBhbGwgdGhlIGluZm9ybWF0aW9uIGFib3V0IGFuIGFjdGl2ZSBiZXR0aW5nIHJvdW5kAAAAAAAAAAAFUm91bmQAAAAAAAAEAAAAblRoZSBsZWRnZXIgbnVtYmVyIHdoZW4gdGhpcyByb3VuZCBlbmRzCkxlZGdlcnMgYXJlIGxpa2UgYmxvY2tzIGluIGJsb2NrY2hhaW4gLSB0aGV5IGluY3JlbWVudCBldmVyeSB+NSBzZWNvbmRzAAAAAAAKZW5kX2xlZGdlcgAAAAAABAAAAEFUb3RhbCB2WExNIGluIHRoZSAiRE9XTiIgcG9vbCAocGVvcGxlIGJldHRpbmcgcHJpY2Ugd2lsbCBnbyBkb3duKQAAAAAAAAlwb29sX2Rvd24AAAAAAAALAAAAPVRvdGFsIHZYTE0gaW4gdGhlICJVUCIgcG9vbCAocGVvcGxlIGJldHRpbmcgcHJpY2Ugd2lsbCBnbyB1cCkAAAAAAAAHcG9vbF91cAAAAAALAAAAPFRoZSBzdGFydGluZyBwcmljZSBvZiBYTE0gd2hlbiB0aGUgcm91bmQgYmVnaW5zIChpbiBzdHJvb3BzKQAAAAtwcmljZV9zdGFydAAAAAAK",
        "AAAAAgAAACNSZXByZXNlbnRzIHdoaWNoIHNpZGUgYSB1c2VyIGJldCBvbgAAAAAAAAAAB0JldFNpZGUAAAAAAgAAAAAAAAAAAAAAAlVwAAAAAAAAAAAAAAAAAAREb3du",
        "AAAAAgAAAL5TdG9yYWdlIGtleXMgZm9yIG9yZ2FuaXppbmcgZGF0YSBpbiB0aGUgY29udHJhY3QKVGhpbmsgb2YgdGhlc2UgYXMgImxhYmVscyIgZm9yIGRpZmZlcmVudCBzdG9yYWdlIGNvbXBhcnRtZW50cwoKVGhlICNbY29udHJhY3R0eXBlXSBhdHRyaWJ1dGUgdGVsbHMgU29yb2JhbiB0aGlzIGNhbiBiZSBzdG9yZWQgaW4gdGhlIGNvbnRyYWN0AAAAAAAAAAAAB0RhdGFLZXkAAAAABwAAAAEAAAAuU3RvcmVzIHRoZSBiYWxhbmNlIGZvciBhIHNwZWNpZmljIHVzZXIgYWRkcmVzcwAAAAAAB0JhbGFuY2UAAAAAAQAAABMAAAAAAAAAO1N0b3JlcyB0aGUgYWRtaW4gYWRkcmVzcyAodGhlIHBlcnNvbiB3aG8gY2FuIGNyZWF0ZSByb3VuZHMpAAAAAAVBZG1pbgAAAAAAAAAAAAA2U3RvcmVzIHRoZSBvcmFjbGUgYWRkcmVzcyAodGhlIHRydXN0ZWQgcHJpY2UgcHJvdmlkZXIpAAAAAAAGT3JhY2xlAAAAAAAAAAAAIVN0b3JlcyB0aGUgY3VycmVudGx5IGFjdGl2ZSByb3VuZAAAAAAAAAtBY3RpdmVSb3VuZAAAAAAAAAAASlN0b3JlcyB1c2VyIHBvc2l0aW9ucyBmb3IgdGhlIGFjdGl2ZSByb3VuZDogTWFwIG9mIEFkZHJlc3MgLT4gVXNlclBvc2l0aW9uAAAAAAAJUG9zaXRpb25zAAAAAAAAAQAAAD9TdG9yZXMgcGVuZGluZyB3aW5uaW5ncyBmb3IgdXNlcnMgKEFkZHJlc3MgLT4gY2xhaW1hYmxlIGFtb3VudCkAAAAAD1BlbmRpbmdXaW5uaW5ncwAAAAABAAAAEwAAAAEAAAAtU3RvcmVzIHVzZXIgc3RhdGlzdGljcyAoQWRkcmVzcyAtPiBVc2VyU3RhdHMpAAAAAAAACVVzZXJTdGF0cwAAAAAAAAEAAAAT",
        "AAAAAQAAACZUcmFja3MgYSB1c2VyJ3MgcHJlZGljdGlvbiBwZXJmb3JtYW5jZQAAAAAAAAAAAAlVc2VyU3RhdHMAAAAAAAAEAAAAIUJlc3Qgd2lubmluZyBzdHJlYWsgZXZlciBhY2hpZXZlZAAAAAAAAAtiZXN0X3N0cmVhawAAAAAEAAAAKUN1cnJlbnQgd2lubmluZyBzdHJlYWsgKGNvbnNlY3V0aXZlIHdpbnMpAAAAAAAADmN1cnJlbnRfc3RyZWFrAAAAAAAEAAAAG1RvdGFsIG51bWJlciBvZiByb3VuZHMgbG9zdAAAAAAMdG90YWxfbG9zc2VzAAAABAAAABpUb3RhbCBudW1iZXIgb2Ygcm91bmRzIHdvbgAAAAAACnRvdGFsX3dpbnMAAAAAAAQ=",
        "AAAAAQAAACpTdG9yZXMgYW4gaW5kaXZpZHVhbCB1c2VyJ3MgYmV0IGluIGEgcm91bmQAAAAAAAAAAAAMVXNlclBvc2l0aW9uAAAAAgAAABpIb3cgbXVjaCB2WExNIHRoZSB1c2VyIGJldAAAAAAABmFtb3VudAAAAAAACwAAABZXaGljaCBzaWRlIHRoZXkgYmV0IG9uAAAAAAAEc2lkZQAAB9AAAAAHQmV0U2lkZQA=",
        "AAAABAAAAG9DdXN0b20gZXJyb3IgdHlwZXMgZm9yIHRoZSBjb250cmFjdApVc2luZyBleHBsaWNpdCBlcnJvciBjb2RlcyBoZWxwcyB3aXRoIGRlYnVnZ2luZyBhbmQgcHJvdmlkZXMgY2xlYXIgZmVlZGJhY2sAAAAAAAAAAA1Db250cmFjdEVycm9yAAAAAAAADQAAACVDb250cmFjdCBoYXMgYWxyZWFkeSBiZWVuIGluaXRpYWxpemVkAAAAAAAAEkFscmVhZHlJbml0aWFsaXplZAAAAAAAAQAAAC1BZG1pbiBhZGRyZXNzIG5vdCBzZXQgLSBjYWxsIGluaXRpYWxpemUgZmlyc3QAAAAAAAALQWRtaW5Ob3RTZXQAAAAAAgAAAC5PcmFjbGUgYWRkcmVzcyBub3Qgc2V0IC0gY2FsbCBpbml0aWFsaXplIGZpcnN0AAAAAAAMT3JhY2xlTm90U2V0AAAAAwAAACJPbmx5IGFkbWluIGNhbiBwZXJmb3JtIHRoaXMgYWN0aW9uAAAAAAARVW5hdXRob3JpemVkQWRtaW4AAAAAAAAEAAAAI09ubHkgb3JhY2xlIGNhbiBwZXJmb3JtIHRoaXMgYWN0aW9uAAAAABJVbmF1dGhvcml6ZWRPcmFjbGUAAAAAAAUAAAAkQmV0IGFtb3VudCBtdXN0IGJlIGdyZWF0ZXIgdGhhbiB6ZXJvAAAAEEludmFsaWRCZXRBbW91bnQAAAAGAAAAFk5vIGFjdGl2ZSByb3VuZCBleGlzdHMAAAAAAA1Ob0FjdGl2ZVJvdW5kAAAAAAAABwAAABdSb3VuZCBoYXMgYWxyZWFkeSBlbmRlZAAAAAAKUm91bmRFbmRlZAAAAAAACAAAAB1Vc2VyIGhhcyBpbnN1ZmZpY2llbnQgYmFsYW5jZQAAAAAAABNJbnN1ZmZpY2llbnRCYWxhbmNlAAAAAAkAAAArVXNlciBoYXMgYWxyZWFkeSBwbGFjZWQgYSBiZXQgaW4gdGhpcyByb3VuZAAAAAAKQWxyZWFkeUJldAAAAAAACgAAABxBcml0aG1ldGljIG92ZXJmbG93IG9jY3VycmVkAAAACE92ZXJmbG93AAAACwAAABNJbnZhbGlkIHByaWNlIHZhbHVlAAAAAAxJbnZhbGlkUHJpY2UAAAAMAAAAFkludmFsaWQgZHVyYXRpb24gdmFsdWUAAAAAAA9JbnZhbGlkRHVyYXRpb24AAAAADQ==",
        "AAAAAAAAARFRdWVyaWVzIChyZWFkcykgdGhlIGN1cnJlbnQgdlhMTSBiYWxhbmNlIGZvciBhIHVzZXIKCiMgUGFyYW1ldGVycwoqIGBlbnZgIC0gVGhlIGNvbnRyYWN0IGVudmlyb25tZW50CiogYHVzZXJgIC0gVGhlIGFkZHJlc3Mgb2YgdGhlIHVzZXIgd2hvc2UgYmFsYW5jZSB3ZSB3YW50IHRvIGNoZWNrCgojIFJldHVybnMKVGhlIHVzZXIncyBiYWxhbmNlIGFzIGFuIGkxMjggKDEyOC1iaXQgaW50ZWdlcikKUmV0dXJucyAwIGlmIHRoZSB1c2VyIGhhcyBuZXZlciByZWNlaXZlZCB0b2tlbnMAAAAAAAAHYmFsYW5jZQAAAAABAAAAAAAAAAR1c2VyAAAAEwAAAAEAAAAL",
        "AAAAAAAAAF9HZXRzIHRoZSBhZG1pbiBhZGRyZXNzCgojIFJldHVybnMKT3B0aW9uPEFkZHJlc3M+IC0gU29tZShhZG1pbikgaWYgc2V0LCBOb25lIGlmIG5vdCBpbml0aWFsaXplZAAAAAAJZ2V0X2FkbWluAAAAAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAA7dQbGFjZXMgYSBiZXQgb24gdGhlIGFjdGl2ZSByb3VuZAoKIyBQYXJhbWV0ZXJzCiogYGVudmAgLSBUaGUgY29udHJhY3QgZW52aXJvbm1lbnQKKiBgdXNlcmAgLSBUaGUgYWRkcmVzcyBvZiB0aGUgdXNlciBwbGFjaW5nIHRoZSBiZXQKKiBgYW1vdW50YCAtIEFtb3VudCBvZiB2WExNIHRvIGJldCAobXVzdCBiZSA+IDApCiogYHNpZGVgIC0gV2hpY2ggc2lkZSB0byBiZXQgb24gKFVwIG9yIERvd24pCgojIFNlY3VyaXR5Ci0gUmVxdWlyZXMgdXNlciBhdXRob3JpemF0aW9uIChwcmV2ZW50cyB1bmF1dGhvcml6ZWQgYmV0dGluZykKLSBWYWxpZGF0ZXMgYmV0IGFtb3VudCBpcyBwb3NpdGl2ZQotIENoZWNrcyByb3VuZCBpcyBzdGlsbCBhY3RpdmUgKHByZXZlbnRzIGxhdGUgYmV0cykKLSBWZXJpZmllcyBzdWZmaWNpZW50IGJhbGFuY2UgKHByZXZlbnRzIG5lZ2F0aXZlIGJhbGFuY2VzKQotIFByZXZlbnRzIGRvdWJsZSBiZXR0aW5nIGluIHNhbWUgcm91bmQKLSBVc2VzIGNoZWNrZWQgYXJpdGhtZXRpYyB0byBwcmV2ZW50IG92ZXJmbG93Ci0gTm8gcmVlbnRyYW5jeSByaXNrOiBzdGF0ZSB1cGRhdGVzIGJlZm9yZSBleHRlcm5hbCBjYWxscyAoQ0VJIHBhdHRlcm4pCgojIEVycm9ycwotIGBDb250cmFjdEVycm9yOjpJbnZhbGlkQmV0QW1vdW50YCBpZiBhbW91bnQgPD0gMAotIGBDb250cmFjdEVycm9yOjpOb0FjdGl2ZVJvdW5kYCBpZiBubyByb3VuZCBleGlzdHMKLSBgQ29udHJhY3RFcnJvcjo6Um91bmRFbmRlZGAgaWYgcm91bmQgaGFzIGVuZGVkCi0gYENvbnRyYWN0RXJyb3I6Okluc3VmZmljaWVudEJhbGFuY2VgIGlmIHVzZXIgYmFsYW5jZSB0b28gbG93Ci0gYENvbnRyYWN0RXJyb3I6OkFscmVhZHlCZXRgIGlmIHVzZXIgYWxyZWFkeSBiZXQgaW4gdGhpcyByb3VuZAotIGBDb250cmFjdEVycm9yOjpPdmVyZmxvd2AgaWYgcG9vbCBjYWxjdWxhdGlvbiBvdmVyZmxvd3MAAAAACXBsYWNlX2JldAAAAAAAAAMAAAAAAAAABHVzZXIAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABHNpZGUAAAfQAAAAB0JldFNpZGUAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAGFHZXRzIHRoZSBvcmFjbGUgYWRkcmVzcwoKIyBSZXR1cm5zCk9wdGlvbjxBZGRyZXNzPiAtIFNvbWUob3JhY2xlKSBpZiBzZXQsIE5vbmUgaWYgbm90IGluaXRpYWxpemVkAAAAAAAACmdldF9vcmFjbGUAAAAAAAAAAAABAAAD6AAAABM=",
        "AAAAAAAAAhhJbml0aWFsaXplcyB0aGUgY29udHJhY3QgYnkgc2V0dGluZyB0aGUgYWRtaW4gYW5kIG9yYWNsZQpUaGlzIHNob3VsZCBiZSBjYWxsZWQgb25jZSB3aGVuIGRlcGxveWluZyB0aGUgY29udHJhY3QKCiMgUGFyYW1ldGVycwoqIGBlbnZgIC0gVGhlIGNvbnRyYWN0IGVudmlyb25tZW50CiogYGFkbWluYCAtIFRoZSBhZGRyZXNzIHRoYXQgd2lsbCBoYXZlIGFkbWluIHByaXZpbGVnZXMgKGNyZWF0ZXMgcm91bmRzKQoqIGBvcmFjbGVgIC0gVGhlIGFkZHJlc3MgdGhhdCBwcm92aWRlcyBwcmljZSBkYXRhIGFuZCByZXNvbHZlcyByb3VuZHMKCiMgU2VjdXJpdHkKLSBQcmV2ZW50cyByZS1pbml0aWFsaXphdGlvbiBhdHRhY2tzCi0gUmVxdWlyZXMgYWRtaW4gYXV0aG9yaXphdGlvbgotIEFkbWluIGFuZCBvcmFjbGUgY2Fubm90IGJlIHRoZSBzYW1lIChzZXBhcmF0aW9uIG9mIGNvbmNlcm5zKQoKIyBFcnJvcnMKUmV0dXJucyBgQ29udHJhY3RFcnJvcjo6QWxyZWFkeUluaXRpYWxpemVkYCBpZiBjb250cmFjdCB3YXMgYWxyZWFkeSBpbml0aWFsaXplZAAAAAppbml0aWFsaXplAAAAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAABm9yYWNsZQAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAyFDcmVhdGVzIGEgbmV3IHByZWRpY3Rpb24gcm91bmQKT25seSB0aGUgYWRtaW4gY2FuIGNhbGwgdGhpcyBmdW5jdGlvbgoKIyBQYXJhbWV0ZXJzCiogYGVudmAgLSBUaGUgY29udHJhY3QgZW52aXJvbm1lbnQKKiBgc3RhcnRfcHJpY2VgIC0gVGhlIGN1cnJlbnQgWExNIHByaWNlIGluIHN0cm9vcHMgKGUuZy4sIDEgWExNID0gMTAsMDAwLDAwMCBzdHJvb3BzKQoqIGBkdXJhdGlvbl9sZWRnZXJzYCAtIEhvdyBtYW55IGxlZGdlcnMgKGJsb2NrcykgdGhlIHJvdW5kIHNob3VsZCBsYXN0CkV4YW1wbGU6IDYwIGxlZGdlcnMg4omIIDUgbWludXRlcyAoc2luY2UgbGVkZ2VycyBhcmUgfjUgc2Vjb25kcykKCiMgU2VjdXJpdHkKLSBPbmx5IGFkbWluIGNhbiBjcmVhdGUgcm91bmRzIChwcmV2ZW50cyB1bmF1dGhvcml6ZWQgcm91bmQgY3JlYXRpb24pCi0gVmFsaWRhdGVzIHByaWNlIGlzIG5vbi16ZXJvCi0gVmFsaWRhdGVzIGR1cmF0aW9uIGlzIHJlYXNvbmFibGUgKHByZXZlbnRzIERvUykKLSBDaGVja3MgZm9yIG92ZXJmbG93IHdoZW4gY2FsY3VsYXRpbmcgZW5kX2xlZGdlcgoKIyBFcnJvcnMKLSBgQ29udHJhY3RFcnJvcjo6QWRtaW5Ob3RTZXRgIGlmIGNvbnRyYWN0IG5vdCBpbml0aWFsaXplZAotIGBDb250cmFjdEVycm9yOjpJbnZhbGlkUHJpY2VgIGlmIHN0YXJ0X3ByaWNlIGlzIDAKLSBgQ29udHJhY3RFcnJvcjo6SW52YWxpZER1cmF0aW9uYCBpZiBkdXJhdGlvbiBpcyAwIG9yIHRvbyBsYXJnZQotIGBDb250cmFjdEVycm9yOjpPdmVyZmxvd2AgaWYgZW5kX2xlZGdlciBjYWxjdWxhdGlvbiBvdmVyZmxvd3MAAAAAAAAMY3JlYXRlX3JvdW5kAAAAAgAAAAAAAAALc3RhcnRfcHJpY2UAAAAACgAAAAAAAAAQZHVyYXRpb25fbGVkZ2VycwAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAZNNaW50cyAoY3JlYXRlcykgaW5pdGlhbCB2WExNIHRva2VucyBmb3IgYSB1c2VyIG9uIHRoZWlyIGZpcnN0IGludGVyYWN0aW9uCgojIFBhcmFtZXRlcnMKKiBgZW52YCAtIFRoZSBjb250cmFjdCBlbnZpcm9ubWVudCAocHJvdmlkZWQgYnkgU29yb2JhbiwgZ2l2ZXMgYWNjZXNzIHRvIHN0b3JhZ2UsIGV0Yy4pCiogYHVzZXJgIC0gVGhlIGFkZHJlc3Mgb2YgdGhlIHVzZXIgd2hvIHdpbGwgcmVjZWl2ZSB0b2tlbnMKCiMgSG93IGl0IHdvcmtzCjEuIENoZWNrcyBpZiB1c2VyIGFscmVhZHkgaGFzIGEgYmFsYW5jZQoyLiBJZiBub3QsIGdpdmVzIHRoZW0gMTAwMCB2WExNIGFzIGEgc3RhcnRpbmcgYW1vdW50CjMuIFN0b3JlcyB0aGlzIGJhbGFuY2UgaW4gdGhlIGNvbnRyYWN0J3MgcGVyc2lzdGVudCBzdG9yYWdlAAAAAAxtaW50X2luaXRpYWwAAAABAAAAAAAAAAR1c2VyAAAAEwAAAAEAAAAL",
        "AAAAAAAAA2FSZXNvbHZlcyBhIHJvdW5kIHdpdGggdGhlIGZpbmFsIHByaWNlIGFuZCBjYWxjdWxhdGVzIHdpbm5pbmdzCk9ubHkgdGhlIG9yYWNsZSBjYW4gY2FsbCB0aGlzIGZ1bmN0aW9uCgojIFBhcmFtZXRlcnMKKiBgZW52YCAtIFRoZSBjb250cmFjdCBlbnZpcm9ubWVudAoqIGBmaW5hbF9wcmljZWAgLSBUaGUgWExNIHByaWNlIGF0IHJvdW5kIGVuZCAoaW4gc3Ryb29wcykKCiMgU2VjdXJpdHkKLSBPbmx5IG9yYWNsZSBjYW4gcmVzb2x2ZSAocHJldmVudHMgdW5hdXRob3JpemVkIHJlc29sdXRpb24pCi0gVmFsaWRhdGVzIGZpbmFsIHByaWNlIGlzIG5vbi16ZXJvCi0gVXNlcyBjaGVja2VkIGFyaXRobWV0aWMgaW4gcGF5b3V0IGNhbGN1bGF0aW9ucwotIE5vIHJlZW50cmFuY3k6IHN0YXRlIGNsZWFyZWQgYWZ0ZXIgYWxsIGNhbGN1bGF0aW9ucwotIFByb3BvcnRpb25hbCBkaXN0cmlidXRpb24gcHJldmVudHMgbWFuaXB1bGF0aW9uCgojIEVycm9ycwotIGBDb250cmFjdEVycm9yOjpPcmFjbGVOb3RTZXRgIGlmIG9yYWNsZSBub3QgY29uZmlndXJlZAotIGBDb250cmFjdEVycm9yOjpOb0FjdGl2ZVJvdW5kYCBpZiBubyByb3VuZCB0byByZXNvbHZlCi0gYENvbnRyYWN0RXJyb3I6OkludmFsaWRQcmljZWAgaWYgZmluYWxfcHJpY2UgaXMgMAoKIyBQYXlvdXQgbG9naWMKLSBJZiBwcmljZSB3ZW50IFVQOiBVUCBiZXR0b3JzIHNwbGl0IHRoZSBET1dOIHBvb2wgcHJvcG9ydGlvbmFsbHkKLSBJZiBwcmljZSB3ZW50IERPV046IERPV04gYmV0dG9ycyBzcGxpdCB0aGUgVVAgcG9vbCBwcm9wb3J0aW9uYWxseQotIElmIHByaWNlIFVOQ0hBTkdFRDogRXZlcnlvbmUgZ2V0cyB0aGVpciBiZXQgYmFjayAobm8gd2lubmVycy9sb3NlcnMpAAAAAAAADXJlc29sdmVfcm91bmQAAAAAAAABAAAAAAAAAAtmaW5hbF9wcmljZQAAAAAKAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAASNDbGFpbXMgcGVuZGluZyB3aW5uaW5ncyBmb3IgYSB1c2VyCgojIFBhcmFtZXRlcnMKKiBgZW52YCAtIFRoZSBjb250cmFjdCBlbnZpcm9ubWVudAoqIGB1c2VyYCAtIFRoZSBhZGRyZXNzIGNsYWltaW5nIHdpbm5pbmdzCgojIEhvdyBpdCB3b3JrcwoxLiBDaGVjayBpZiB1c2VyIGhhcyBwZW5kaW5nIHdpbm5pbmdzCjIuIEFkZCB3aW5uaW5ncyB0byB1c2VyJ3MgYmFsYW5jZQozLiBDbGVhciBwZW5kaW5nIHdpbm5pbmdzCgojIFJldHVybnMKQW1vdW50IGNsYWltZWQgKDAgaWYgbm8gcGVuZGluZyB3aW5uaW5ncykAAAAADmNsYWltX3dpbm5pbmdzAAAAAAABAAAAAAAAAAR1c2VyAAAAEwAAAAEAAAAL",
        "AAAAAAAAANRHZXRzIGEgdXNlcidzIHN0YXRpc3RpY3MgKHdpbnMsIGxvc3Nlcywgc3RyZWFrcykKCiMgUGFyYW1ldGVycwoqIGBlbnZgIC0gVGhlIGNvbnRyYWN0IGVudmlyb25tZW50CiogYHVzZXJgIC0gVGhlIGFkZHJlc3Mgb2YgdGhlIHVzZXIKCiMgUmV0dXJucwpVc2VyU3RhdHMgaWYgdGhlIHVzZXIgaGFzIHBhcnRpY2lwYXRlZCwgb3IgZGVmYXVsdCBzdGF0cyAoYWxsIHplcm9zKQAAAA5nZXRfdXNlcl9zdGF0cwAAAAAAAQAAAAAAAAAEdXNlcgAAABMAAAABAAAH0AAAAAlVc2VyU3RhdHMAAAA=",
        "AAAAAAAAALhHZXRzIHRoZSBjdXJyZW50bHkgYWN0aXZlIHJvdW5kCgojIFJldHVybnMKT3B0aW9uPFJvdW5kPiAtIFNvbWUocm91bmQpIGlmIHRoZXJlJ3MgYW4gYWN0aXZlIHJvdW5kLCBOb25lIGlmIG5vdAoKIyBVc2UgY2FzZQpGcm9udGVuZCBjYW4gY2FsbCB0aGlzIHRvIGRpc3BsYXkgY3VycmVudCByb3VuZCBpbmZvIHRvIHVzZXJzAAAAEGdldF9hY3RpdmVfcm91bmQAAAAAAAAAAQAAA+gAAAfQAAAABVJvdW5kAAAA",
        "AAAAAAAAAM1HZXRzIGEgdXNlcidzIHBvc2l0aW9uIGluIHRoZSBjdXJyZW50IHJvdW5kCgojIFBhcmFtZXRlcnMKKiBgZW52YCAtIFRoZSBjb250cmFjdCBlbnZpcm9ubWVudAoqIGB1c2VyYCAtIFRoZSBhZGRyZXNzIG9mIHRoZSB1c2VyCgojIFJldHVybnMKT3B0aW9uPFVzZXJQb3NpdGlvbj4gLSBTb21lKHBvc2l0aW9uKSBpZiB1c2VyIGhhcyBiZXQsIE5vbmUgaWYgbm90AAAAAAAAEWdldF91c2VyX3Bvc2l0aW9uAAAAAAAAAQAAAAAAAAAEdXNlcgAAABMAAAABAAAD6AAAB9AAAAAMVXNlclBvc2l0aW9u",
        "AAAAAAAAAMNHZXRzIGEgdXNlcidzIHBlbmRpbmcgd2lubmluZ3MgKGFtb3VudCB0aGV5IGNhbiBjbGFpbSkKCiMgUGFyYW1ldGVycwoqIGBlbnZgIC0gVGhlIGNvbnRyYWN0IGVudmlyb25tZW50CiogYHVzZXJgIC0gVGhlIGFkZHJlc3Mgb2YgdGhlIHVzZXIKCiMgUmV0dXJucwpBbW91bnQgb2YgdlhMTSB0aGUgdXNlciBjYW4gY2xhaW0gKDAgaWYgbm9uZSkAAAAAFGdldF9wZW5kaW5nX3dpbm5pbmdzAAAAAQAAAAAAAAAEdXNlcgAAABMAAAABAAAACw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    balance: this.txFromJSON<i128>,
        get_admin: this.txFromJSON<Option<string>>,
        place_bet: this.txFromJSON<Result<void>>,
        get_oracle: this.txFromJSON<Option<string>>,
        initialize: this.txFromJSON<Result<void>>,
        create_round: this.txFromJSON<Result<void>>,
        mint_initial: this.txFromJSON<i128>,
        resolve_round: this.txFromJSON<Result<void>>,
        claim_winnings: this.txFromJSON<i128>,
        get_user_stats: this.txFromJSON<UserStats>,
        get_active_round: this.txFromJSON<Option<Round>>,
        get_user_position: this.txFromJSON<Option<UserPosition>>,
        get_pending_winnings: this.txFromJSON<i128>
  }
}