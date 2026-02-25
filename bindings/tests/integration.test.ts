import { describe, it, expect, beforeAll } from "vitest";
import { Keypair, Networks } from "@stellar/stellar-sdk";
import { Client, BetSide, ContractError } from "../src/index.js";
import {
  generateTestAccounts,
  fundAllAccounts,
  createClient,
  sleep,
  TESTNET_RPC_URL,
  INITIAL_BALANCE,
  START_PRICE,
  FINAL_PRICE_UP,
  BET_AMOUNT,
  ROUND_DURATION,
  type TestAccounts,
} from "./setup.js";

describe("Xelma Integration Tests", () => {
  let accounts: TestAccounts;
  let contractId: string;
  let adminClient: Client;
  let oracleClient: Client;
  let user1Client: Client;
  let user2Client: Client;

  beforeAll(async () => {
    accounts = generateTestAccounts();

    contractId = process.env.CONTRACT_ID || "";
    if (!contractId) {
      console.log("CONTRACT_ID not set - skipping live contract tests");
      console.log(
        "To run full integration tests, deploy a contract and set CONTRACT_ID env var",
      );
      return;
    }

    try {
      console.log("Funding test accounts via Friendbot...");
      await fundAllAccounts(accounts);
      console.log("Accounts funded successfully");
    } catch (error) {
      console.log(
        "Warning: Could not fund accounts via Friendbot (network unavailable)",
      );
      console.log("Contract-dependent tests will be skipped");
      contractId = "";
      return;
    }

    adminClient = createClient(contractId, accounts.admin);
    oracleClient = createClient(contractId, accounts.oracle);
    user1Client = createClient(contractId, accounts.user1);
    user2Client = createClient(contractId, accounts.user2);
  });

  describe("Account Setup", () => {
    it("should generate valid keypairs", () => {
      expect(accounts.admin.publicKey()).toMatch(/^G[A-Z0-9]{55}$/);
      expect(accounts.oracle.publicKey()).toMatch(/^G[A-Z0-9]{55}$/);
      expect(accounts.user1.publicKey()).toMatch(/^G[A-Z0-9]{55}$/);
      expect(accounts.user2.publicKey()).toMatch(/^G[A-Z0-9]{55}$/);
    });

    it("should have unique keypairs", () => {
      const keys = [
        accounts.admin.publicKey(),
        accounts.oracle.publicKey(),
        accounts.user1.publicKey(),
        accounts.user2.publicKey(),
      ];
      const uniqueKeys = new Set(keys);
      expect(uniqueKeys.size).toBe(4);
    });
  });

  describe("Contract Initialization", () => {
    it.skipIf(!process.env.CONTRACT_ID)(
      "should initialize contract with admin and oracle",
      async () => {
        const tx = await adminClient.initialize({
          admin: accounts.admin.publicKey(),
          oracle: accounts.oracle.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeDefined();
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should return admin address after initialization",
      async () => {
        const tx = await adminClient.get_admin();
        const result = await tx.signAndSend();
        expect(result.result).toBe(accounts.admin.publicKey());
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should return oracle address after initialization",
      async () => {
        const tx = await adminClient.get_oracle();
        const result = await tx.signAndSend();
        expect(result.result).toBe(accounts.oracle.publicKey());
      },
    );
  });

  describe("Token Minting", () => {
    it.skipIf(!process.env.CONTRACT_ID)(
      "should mint initial tokens for new user",
      async () => {
        const tx = await user1Client.mint_initial({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBe(INITIAL_BALANCE);
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should return correct balance after minting",
      async () => {
        const tx = await user1Client.balance({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBe(INITIAL_BALANCE);
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should mint tokens for second user",
      async () => {
        const tx = await user2Client.mint_initial({
          user: accounts.user2.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBe(INITIAL_BALANCE);
      },
    );
  });

  describe("Round Management", () => {
    it.skipIf(!process.env.CONTRACT_ID)(
      "should create a new round",
      async () => {
        const tx = await adminClient.create_round({
          start_price: START_PRICE,
          duration_ledgers: ROUND_DURATION,
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeDefined();
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should return active round details",
      async () => {
        const tx = await adminClient.get_active_round();
        const result = await tx.signAndSend();

        expect(result.result).toBeDefined();
        expect(result.result?.price_start).toBe(START_PRICE);
      },
    );
  });

  describe("Betting", () => {
    it.skipIf(!process.env.CONTRACT_ID)(
      "should place UP bet for user1",
      async () => {
        const tx = await user1Client.place_bet({
          user: accounts.user1.publicKey(),
          amount: BET_AMOUNT,
          side: { tag: "Up", values: undefined } as BetSide,
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeDefined();
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should place DOWN bet for user2",
      async () => {
        const tx = await user2Client.place_bet({
          user: accounts.user2.publicKey(),
          amount: BET_AMOUNT,
          side: { tag: "Down", values: undefined } as BetSide,
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeDefined();
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should return user position after betting",
      async () => {
        const tx = await user1Client.get_user_position({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeDefined();
        expect(result.result?.amount).toBe(BET_AMOUNT);
        expect(result.result?.side.tag).toBe("Up");
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should update pool totals after bets",
      async () => {
        const tx = await adminClient.get_active_round();
        const result = await tx.signAndSend();

        expect(result.result?.pool_up).toBe(BET_AMOUNT);
        expect(result.result?.pool_down).toBe(BET_AMOUNT);
      },
    );
  });

  describe("Round Resolution", () => {
    it.skipIf(!process.env.CONTRACT_ID)(
      "should resolve round with final price",
      async () => {
        const tx = await oracleClient.resolve_round({
          final_price: FINAL_PRICE_UP,
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeDefined();
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should clear active round after resolution",
      async () => {
        const tx = await adminClient.get_active_round();
        const result = await tx.signAndSend();

        expect(result.result).toBeUndefined();
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should set pending winnings for winner (user1)",
      async () => {
        const tx = await user1Client.get_pending_winnings({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeGreaterThan(0n);
      },
    );
  });

  describe("Claiming Winnings", () => {
    it.skipIf(!process.env.CONTRACT_ID)(
      "should claim winnings for winner",
      async () => {
        const tx = await user1Client.claim_winnings({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeGreaterThan(0n);
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should clear pending winnings after claim",
      async () => {
        const tx = await user1Client.get_pending_winnings({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBe(0n);
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should increase winner balance after claim",
      async () => {
        const tx = await user1Client.balance({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result).toBeGreaterThan(INITIAL_BALANCE - BET_AMOUNT);
      },
    );
  });

  describe("User Statistics", () => {
    it.skipIf(!process.env.CONTRACT_ID)(
      "should track wins for winner",
      async () => {
        const tx = await user1Client.get_user_stats({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result.total_wins).toBeGreaterThanOrEqual(1);
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should track losses for loser",
      async () => {
        const tx = await user2Client.get_user_stats({
          user: accounts.user2.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result.total_losses).toBeGreaterThanOrEqual(1);
      },
    );

    it.skipIf(!process.env.CONTRACT_ID)(
      "should track winning streak",
      async () => {
        const tx = await user1Client.get_user_stats({
          user: accounts.user1.publicKey(),
        });

        const result = await tx.signAndSend();
        expect(result.result.current_streak).toBeGreaterThanOrEqual(1);
      },
    );
  });

  describe("Error Handling", () => {
    it("should have correct error codes defined", () => {
      expect(ContractError[1].message).toBe("AlreadyInitialized");
      expect(ContractError[6].message).toBe("InvalidBetAmount");
      expect(ContractError[7].message).toBe("NoActiveRound");
      expect(ContractError[9].message).toBe("InsufficientBalance");
      expect(ContractError[10].message).toBe("AlreadyBet");
    });
  });
});
