import {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} from "@cosmjs/proto-signing";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { GasPrice } from "@cosmjs/stargate";
import { Secp256k1, Random, sha256 } from "@cosmjs/crypto";
import { CONFIG } from "./config";
import { TestLogger } from "./test-utils";
import * as bip39 from "bip39";

export interface TestWallet {
  address: string;
  wallet: DirectSecp256k1HdWallet;
  client: SigningCosmWasmClient;
  mnemonic: string;
}

export class WalletManager {
  private logger: TestLogger;
  private deployerWallet: TestWallet | null = null;
  private testWallets: Map<string, TestWallet> = new Map();

  constructor(logger: TestLogger) {
    this.logger = logger;
  }

  async getDeployerWallet(): Promise<TestWallet> {
    if (!this.deployerWallet) {
      this.logger.info("Restoring deployer wallet...");

      const wallet = await DirectSecp256k1HdWallet.fromMnemonic(
        CONFIG.deployerMnemonic,
        {
          prefix: "neutron",
        }
      );

      const accounts = await wallet.getAccounts();
      const address = accounts[0].address;

      const client = await SigningCosmWasmClient.connectWithSigner(
        CONFIG.rpcEndpoint,
        wallet,
        {
          gasPrice: GasPrice.fromString(CONFIG.gasPrice),
        }
      );

      this.deployerWallet = {
        address,
        wallet,
        client,
        mnemonic: CONFIG.deployerMnemonic,
      };

      this.logger.info(`Deployer wallet restored: ${address}`);
    }

    return this.deployerWallet;
  }

  async createTestWallet(userId: string): Promise<TestWallet> {
    if (this.testWallets.has(userId)) {
      return this.testWallets.get(userId)!;
    }

    this.logger.info(`Creating test wallet for user: ${userId}`);

    // Generate a random mnemonic for the test wallet
    const mnemonic = this.generateMnemonic();

    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
      prefix: "neutron",
    });

    const accounts = await wallet.getAccounts();
    const address = accounts[0].address;

    const client = await SigningCosmWasmClient.connectWithSigner(
      CONFIG.rpcEndpoint,
      wallet,
      {
        gasPrice: GasPrice.fromString(CONFIG.gasPrice),
      }
    );

    const testWallet: TestWallet = {
      address,
      wallet,
      client,
      mnemonic,
    };

    this.testWallets.set(userId, testWallet);
    this.logger.info(`Test wallet created for user ${userId}: ${address}`);

    return testWallet;
  }

  async fundTestWallet(
    userId: string,
    amounts: { [denom: string]: string }
  ): Promise<void> {
    const deployerWallet = await this.getDeployerWallet();
    const testWallet = await this.createTestWallet(userId);

    this.logger.info(
      `Funding test wallet for user ${userId} (${testWallet.address})...`
    );

    for (const [denom, amount] of Object.entries(amounts)) {
      try {
        const result = await deployerWallet.client.sendTokens(
          deployerWallet.address,
          testWallet.address,
          [{ denom, amount }],
          "auto"
        );

        this.logger.info(
          `Sent ${amount}${denom} to user ${userId}: ${result.transactionHash}`
        );
      } catch (error) {
        this.logger.error(
          `Failed to send ${amount}${denom} to user ${userId}: ${error}`
        );
        throw error;
      }
    }
  }

  async fundAllTestWallets(userIds: string[]): Promise<void> {
    this.logger.info("Funding all test wallets with required tokens...");

    // Standard funding amounts for testing
    // Increased amounts to ensure users have enough tokens for locking and claiming
    const fundingAmounts = {
      [CONFIG.tokenDenoms.NTRN]: "5000000000", // 5000 NTRN
      [CONFIG.tokenDenoms.DATOM]: "5000000000", // 5000 dATOM
      [CONFIG.tokenDenoms.STATOM]: "5000000000", // 5000 stATOM
    };

    for (const userId of userIds) {
      await this.fundTestWallet(userId, fundingAmounts);
    }

    this.logger.info("All test wallets funded successfully");
  }

  getTestWallet(userId: string): TestWallet | undefined {
    return this.testWallets.get(userId);
  }

  getAllTestWallets(): Map<string, TestWallet> {
    return new Map(this.testWallets);
  }

  async getBalance(userId: string, denom: string): Promise<string> {
    const wallet = this.testWallets.get(userId);
    if (!wallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    const balance = await wallet.client.getBalance(wallet.address, denom);
    return balance.amount;
  }

  private generateMnemonic(): string {
    // Generate unique mnemonics for each test to avoid state contamination
    // Use BIP39 library to generate valid mnemonics
    const timestamp = Date.now();
    const random = Math.floor(Math.random() * 1000000);
    const uniqueSeed = `${timestamp}-${random}-${this.testWallets.size}`;

    // Generate a unique mnemonic using the unique seed
    const mnemonic = bip39.generateMnemonic(256); // 24 words for better uniqueness

    return mnemonic;
  }

  async cleanup(): Promise<void> {
    this.logger.info("Cleaning up wallet connections...");

    // Close all client connections
    for (const wallet of this.testWallets.values()) {
      wallet.client.disconnect();
    }

    if (this.deployerWallet) {
      this.deployerWallet.client.disconnect();
    }

    this.testWallets.clear();
    this.deployerWallet = null;

    this.logger.info("Wallet cleanup completed");
  }
}
