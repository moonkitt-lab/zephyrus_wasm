"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RewardsValidator = void 0;
const test_utils_1 = require("./test-utils");
// Import contract clients
const ZephyrusMain_client_1 = require("./contracts/ZephyrusMain.client");
const HydroBase_client_1 = require("./contracts/HydroBase.client");
const TributeBase_client_1 = require("./contracts/TributeBase.client");
class RewardsValidator {
    constructor(logger, walletManager, contractAddresses) {
        this.clients = {};
        this.logger = logger;
        this.walletManager = walletManager;
        this.contractAddresses = contractAddresses;
    }
    async initializeClients() {
        this.logger.info("Initializing validator contract clients...");
        const deployerWallet = await this.walletManager.getDeployerWallet();
        this.clients.hydro = new HydroBase_client_1.HydroBaseClient(deployerWallet.client, deployerWallet.address, this.contractAddresses.hydro);
        this.clients.tribute = new TributeBase_client_1.TributeBaseClient(deployerWallet.client, deployerWallet.address, this.contractAddresses.tribute);
        this.clients.zephyrus = new ZephyrusMain_client_1.ZephyrusMainClient(deployerWallet.client, deployerWallet.address, this.contractAddresses.zephyrus);
        this.logger.info("Validator contract clients initialized successfully");
    }
    async validateRewards(expectedRewards, executionResult) {
        try {
            this.logger.section("Validating Rewards");
            // Initialize clients
            await this.initializeClients();
            // Query actual rewards from contracts
            const actualRewards = await this.queryActualRewards(executionResult);
            // Compare expected vs actual
            const report = test_utils_1.ReportGenerator.generateTestReport("Rewards Validation", {}, // scenario will be passed separately
            expectedRewards, actualRewards, 0 // execution time will be calculated elsewhere
            );
            return {
                success: report.success,
                actualRewards,
                discrepancies: report.discrepancies,
            };
        }
        catch (error) {
            this.logger.error("Rewards validation failed", error);
            return {
                success: false,
                actualRewards: {
                    protocol_rewards: {},
                    hydromancer_rewards: {},
                    user_rewards: {}
                },
                discrepancies: [],
                error: error instanceof Error ? error.message : String(error)
            };
        }
    }
    async queryActualRewards(executionResult) {
        this.logger.info("Querying actual rewards from contracts...");
        // Get protocol rewards (these would be in the contract's balance)
        const protocolRewards = await this.queryProtocolRewards();
        // Get hydromancer rewards  
        const hydromancerRewards = await this.queryHydromancerRewards();
        // Get user rewards
        const userRewards = await this.queryUserRewards(executionResult.vesselIds);
        return {
            protocol_rewards: protocolRewards,
            hydromancer_rewards: hydromancerRewards,
            user_rewards: userRewards
        };
    }
    async queryProtocolRewards() {
        this.logger.info("Querying protocol rewards...");
        try {
            // Protocol rewards are typically held in the contract's balance
            // Query the contract's balance for each token type
            const deployerWallet = await this.walletManager.getDeployerWallet();
            const protocolRewards = {};
            // Check common token balances
            const commonDenoms = [
                "untrn",
                "uusdc",
                // Add other token denoms as needed
            ];
            for (const denom of commonDenoms) {
                try {
                    const balance = await deployerWallet.client.getBalance(this.contractAddresses.zephyrus, // Protocol rewards go to Zephyrus
                    denom);
                    if (parseFloat(balance.amount) > 0) {
                        protocolRewards[denom] = (parseFloat(balance.amount) / 1000000).toFixed(2);
                    }
                }
                catch (error) {
                    // Token not found or no balance, skip
                }
            }
            return protocolRewards;
        }
        catch (error) {
            this.logger.error("Failed to query protocol rewards", error);
            return {};
        }
    }
    async queryHydromancerRewards() {
        this.logger.info("Querying hydromancer rewards...");
        try {
            // Hydromancer rewards are also tracked in Zephyrus contract
            const deployerWallet = await this.walletManager.getDeployerWallet();
            // Query hydromancer's claimable rewards
            // This would require a specific query method in the contract
            const hydromancerRewards = {};
            // For now, simulate by checking contract balance
            // In real implementation, there would be specific query methods
            return hydromancerRewards;
        }
        catch (error) {
            this.logger.error("Failed to query hydromancer rewards", error);
            return {};
        }
    }
    async queryUserRewards(vesselIds) {
        this.logger.info("Querying user rewards...");
        const userRewards = {};
        try {
            for (const [userId, userVesselIds] of Object.entries(vesselIds)) {
                userRewards[userId] = {};
                // Query claimable rewards for this user's vessels
                const claimableRewards = await this.queryUserClaimableRewards(userId, userVesselIds);
                // Aggregate rewards by token denomination
                for (const reward of claimableRewards) {
                    if (userRewards[userId][reward.denom]) {
                        const existing = parseFloat(userRewards[userId][reward.denom]);
                        const additional = parseFloat(reward.amount);
                        userRewards[userId][reward.denom] = (existing + additional).toFixed(2);
                    }
                    else {
                        userRewards[userId][reward.denom] = parseFloat(reward.amount).toFixed(2);
                    }
                }
                this.logger.info(`Queried rewards for user ${userId}: ${Object.keys(userRewards[userId]).length} tokens`);
            }
            return userRewards;
        }
        catch (error) {
            this.logger.error("Failed to query user rewards", error);
            return {};
        }
    }
    async queryUserClaimableRewards(userId, vesselIds) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            this.logger.warn(`Test wallet not found for user: ${userId}`);
            return [];
        }
        try {
            // Create Zephyrus client for this user
            const zephyrusClient = new ZephyrusMain_client_1.ZephyrusMainClient(userWallet.client, userWallet.address, this.contractAddresses.zephyrus);
            // Query claimable rewards for the current round
            const currentRound = await this.clients.hydro.currentRound();
            const roundId = currentRound.round_id > 0 ? currentRound.round_id - 1 : 0; // Previous round
            const claimableRewards = [];
            // Query rewards for each vessel
            for (const vesselId of vesselIds) {
                try {
                    // Query claimable rewards for this specific vessel
                    const rewardsQuery = {
                        claimable_rewards: {
                            round_id: roundId,
                            tranche_id: 1,
                            vessel_id: vesselId
                        }
                    };
                    const rewards = await zephyrusClient.vesselsRewards({
                        roundId: roundId,
                        trancheId: 1,
                        vesselIds: [vesselId],
                        userAddress: userWallet.address
                    });
                    // Parse rewards response
                    if (rewards && rewards.rewards) {
                        for (const reward of rewards.rewards) {
                            claimableRewards.push({
                                denom: reward.coin.denom,
                                amount: (parseFloat(reward.coin.amount) / 1000000).toFixed(2) // Convert from micro units
                            });
                        }
                    }
                }
                catch (error) {
                    // Vessel might not have claimable rewards, continue
                    this.logger.debug(`No claimable rewards for vessel ${vesselId}: ${error}`);
                }
            }
            return claimableRewards;
        }
        catch (error) {
            this.logger.error(`Failed to query claimable rewards for user ${userId}`, error);
            return [];
        }
    }
    async claimAllRewards(executionResult) {
        this.logger.info("Claiming all rewards to verify actual amounts...");
        try {
            const currentRound = await this.clients.hydro.currentRound();
            const roundId = currentRound.round_id > 0 ? currentRound.round_id - 1 : 0;
            // Claim rewards for each user
            for (const [userId, vesselIds] of Object.entries(executionResult.vesselIds)) {
                await this.claimUserRewards(userId, vesselIds, roundId);
            }
            // Claim hydromancer rewards
            await this.claimHydromancerRewards(roundId);
            this.logger.info("All rewards claimed successfully");
            return true;
        }
        catch (error) {
            this.logger.error("Failed to claim rewards", error);
            return false;
        }
    }
    async claimUserRewards(userId, vesselIds, roundId) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            this.logger.warn(`Test wallet not found for user: ${userId}`);
            return;
        }
        try {
            // Create Zephyrus client for this user
            const zephyrusClient = new ZephyrusMain_client_1.ZephyrusMainClient(userWallet.client, userWallet.address, this.contractAddresses.zephyrus);
            // Record balance before claiming
            const balancesBefore = await this.getUserTokenBalances(userId);
            // Claim rewards
            const claimMsg = {
                claim: {
                    round_id: roundId,
                    tranche_id: 1,
                    vessel_ids: vesselIds
                }
            };
            const result = await zephyrusClient.claim({
                roundId: roundId,
                trancheId: 1,
                vesselIds: vesselIds
            });
            // Record balance after claiming
            const balancesAfter = await this.getUserTokenBalances(userId);
            // Log the difference
            this.logBalanceChanges(userId, balancesBefore, balancesAfter);
            this.logger.info(`Claimed rewards for user ${userId} vessels: ${vesselIds.join(", ")}`);
        }
        catch (error) {
            this.logger.error(`Failed to claim rewards for user ${userId}`, error);
        }
    }
    async claimHydromancerRewards(roundId) {
        try {
            // Hydromancer rewards are typically claimed by the protocol admin
            const deployerWallet = await this.walletManager.getDeployerWallet();
            // This would depend on the specific hydromancer rewards claiming mechanism
            // For now, we'll log that this step would be performed
            this.logger.info(`Hydromancer rewards claim simulated for round ${roundId}`);
        }
        catch (error) {
            this.logger.error("Failed to claim hydromancer rewards", error);
        }
    }
    async getUserTokenBalances(userId) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            return {};
        }
        const balances = {};
        try {
            // Get balances for common tokens
            const commonDenoms = ["untrn", "uusdc"];
            for (const denom of commonDenoms) {
                try {
                    const balance = await userWallet.client.getBalance(userWallet.address, denom);
                    balances[balance.denom] = balance.amount;
                }
                catch (error) {
                    // Token not found or no balance
                }
            }
        }
        catch (error) {
            this.logger.error(`Failed to get token balances for user ${userId}`, error);
        }
        return balances;
    }
    logBalanceChanges(userId, before, after) {
        const changes = [];
        const allDenoms = new Set([...Object.keys(before), ...Object.keys(after)]);
        for (const denom of allDenoms) {
            const beforeAmount = parseInt(before[denom] || "0");
            const afterAmount = parseInt(after[denom] || "0");
            const change = afterAmount - beforeAmount;
            if (change !== 0) {
                const symbol = this.getTokenSymbol(denom);
                const humanAmount = (change / 1000000).toFixed(2);
                changes.push(`${symbol}: +${humanAmount}`);
            }
        }
        if (changes.length > 0) {
            this.logger.info(`Balance changes for ${userId}: ${changes.join(", ")}`);
        }
    }
    getTokenSymbol(denom) {
        // Convert token denom to readable symbol
        if (denom === "untrn")
            return "NTRN";
        if (denom === "uusdc")
            return "USDC";
        if (denom.includes("udatom"))
            return "dATOM";
        if (denom.includes("statom"))
            return "stATOM";
        return denom;
    }
}
exports.RewardsValidator = RewardsValidator;
//# sourceMappingURL=rewards-validator.js.map