import { Keypair, Networks, TransactionBuilder, Operation, Asset, Horizon } from '@stellar/stellar-sdk';
import { Client } from '../src/index.js';

export const TESTNET_RPC_URL = 'https://soroban-testnet.stellar.org';
export const TESTNET_HORIZON_URL = 'https://horizon-testnet.stellar.org';
export const FRIENDBOT_URL = 'https://friendbot.stellar.org';

export interface TestAccounts {
  admin: Keypair;
  oracle: Keypair;
  user1: Keypair;
  user2: Keypair;
}

export function generateTestAccounts(): TestAccounts {
  return {
    admin: Keypair.random(),
    oracle: Keypair.random(),
    user1: Keypair.random(),
    user2: Keypair.random(),
  };
}

export async function fundAccount(publicKey: string): Promise<void> {
  const response = await fetch(`${FRIENDBOT_URL}?addr=${publicKey}`);
  if (!response.ok) {
    const text = await response.text();
    if (!text.includes('createAccountAlreadyExist')) {
      throw new Error(`Failed to fund account ${publicKey}: ${text}`);
    }
  }
}

export async function fundAllAccounts(accounts: TestAccounts): Promise<void> {
  await Promise.all([
    fundAccount(accounts.admin.publicKey()),
    fundAccount(accounts.oracle.publicKey()),
    fundAccount(accounts.user1.publicKey()),
    fundAccount(accounts.user2.publicKey()),
  ]);
}

export function createClient(contractId: string, keypair: Keypair): Client {
  return new Client({
    contractId,
    networkPassphrase: Networks.TESTNET,
    rpcUrl: TESTNET_RPC_URL,
    publicKey: keypair.publicKey(),
    signTransaction: async (xdr: string) => {
      const tx = TransactionBuilder.fromXDR(xdr, Networks.TESTNET);
      tx.sign(keypair);
      return tx.toXDR();
    },
  });
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export const INITIAL_BALANCE = 10000000000n;
export const START_PRICE = 1000000n;
export const FINAL_PRICE_UP = 1500000n;
export const FINAL_PRICE_DOWN = 500000n;
export const BET_AMOUNT = 1000000000n;
export const ROUND_DURATION = 60;
