"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.WalletManager = void 0;
const proto_signing_1 = require("@cosmjs/proto-signing");
const cosmwasm_stargate_1 = require("@cosmjs/cosmwasm-stargate");
const stargate_1 = require("@cosmjs/stargate");
const crypto_1 = require("@cosmjs/crypto");
const config_1 = require("./config");
class WalletManager {
    constructor(logger) {
        this.deployerWallet = null;
        this.testWallets = new Map();
        this.logger = logger;
    }
    async getDeployerWallet() {
        if (!this.deployerWallet) {
            this.logger.info("Creating deployer wallet...");
            const wallet = await proto_signing_1.DirectSecp256k1HdWallet.fromMnemonic(config_1.CONFIG.deployerMnemonic, {
                prefix: "neutron",
            });
            const accounts = await wallet.getAccounts();
            const address = accounts[0].address;
            const client = await cosmwasm_stargate_1.SigningCosmWasmClient.connectWithSigner(config_1.CONFIG.rpcEndpoint, wallet, {
                gasPrice: stargate_1.GasPrice.fromString(config_1.CONFIG.gasPrice),
            });
            this.deployerWallet = {
                address,
                wallet,
                client,
                mnemonic: config_1.CONFIG.deployerMnemonic,
            };
            this.logger.info(`Deployer wallet created: ${address}`);
        }
        return this.deployerWallet;
    }
    async createTestWallet(userId) {
        if (this.testWallets.has(userId)) {
            return this.testWallets.get(userId);
        }
        this.logger.info(`Creating test wallet for user: ${userId}`);
        // Generate a random mnemonic for the test wallet
        const mnemonic = this.generateMnemonic();
        const wallet = await proto_signing_1.DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
            prefix: "neutron",
        });
        const accounts = await wallet.getAccounts();
        const address = accounts[0].address;
        const client = await cosmwasm_stargate_1.SigningCosmWasmClient.connectWithSigner(config_1.CONFIG.rpcEndpoint, wallet, {
            gasPrice: stargate_1.GasPrice.fromString(config_1.CONFIG.gasPrice),
        });
        const testWallet = {
            address,
            wallet,
            client,
            mnemonic,
        };
        this.testWallets.set(userId, testWallet);
        this.logger.info(`Test wallet created for ${userId}: ${address}`);
        return testWallet;
    }
    async fundTestWallet(userId, amounts) {
        const deployerWallet = await this.getDeployerWallet();
        const testWallet = await this.createTestWallet(userId);
        this.logger.info(`Funding test wallet ${userId} (${testWallet.address})...`);
        for (const [denom, amount] of Object.entries(amounts)) {
            try {
                const result = await deployerWallet.client.sendTokens(deployerWallet.address, testWallet.address, [{ denom, amount }], "auto");
                this.logger.info(`Sent ${amount}${denom} to ${userId}: ${result.transactionHash}`);
            }
            catch (error) {
                this.logger.error(`Failed to send ${amount}${denom} to ${userId}: ${error}`);
                throw error;
            }
        }
    }
    async fundAllTestWallets(userIds) {
        this.logger.info("Funding all test wallets with required tokens...");
        // Standard funding amounts for testing
        const fundingAmounts = {
            [config_1.CONFIG.tokenDenoms.NTRN]: "1000000000", // 1000 NTRN
            [config_1.CONFIG.tokenDenoms.DATOM]: "1000000000", // 1000 dATOM
            [config_1.CONFIG.tokenDenoms.STATOM]: "1000000000", // 1000 stATOM
            "uusdc": "1000000000", // 1000 USDC
        };
        for (const userId of userIds) {
            await this.fundTestWallet(userId, fundingAmounts);
        }
        this.logger.info("All test wallets funded successfully");
    }
    getTestWallet(userId) {
        return this.testWallets.get(userId);
    }
    getAllTestWallets() {
        return new Map(this.testWallets);
    }
    async getBalance(userId, denom) {
        const wallet = this.testWallets.get(userId);
        if (!wallet) {
            throw new Error(`Test wallet not found for user: ${userId}`);
        }
        const balance = await wallet.client.getBalance(wallet.address, denom);
        return balance.amount;
    }
    generateMnemonic() {
        // Generate 24 random words for mnemonic (simplified approach)
        // In production, you'd use a proper BIP39 library
        const entropy = crypto_1.Random.getBytes(32);
        const hash = (0, crypto_1.sha256)(entropy);
        // This is a simplified mnemonic generation - in production use proper BIP39
        const words = [
            "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract",
            "absurd", "abuse", "access", "accident", "account", "accuse", "achieve", "acid",
            "acoustic", "acquire", "across", "act", "action", "actor", "actress", "actual"
        ];
        const mnemonic = [];
        for (let i = 0; i < 24; i++) {
            const index = hash[i] % words.length;
            mnemonic.push(words[index]);
        }
        return mnemonic.join(" ");
    }
    async cleanup() {
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
exports.WalletManager = WalletManager;
//# sourceMappingURL=wallet-manager.js.map