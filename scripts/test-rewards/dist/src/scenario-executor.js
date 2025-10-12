"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.ScenarioExecutor = void 0;
const test_utils_1 = require("./test-utils");
const config_1 = require("./config");
// Import contract clients
const ZephyrusMain_client_1 = require("./contracts/ZephyrusMain.client");
const HydroBase_client_1 = require("./contracts/HydroBase.client");
const TributeBase_client_1 = require("./contracts/TributeBase.client");
class ScenarioExecutor {
    constructor(logger, walletManager, contractAddresses) {
        this.clients = {};
        this.logger = logger;
        this.walletManager = walletManager;
        this.contractAddresses = contractAddresses;
    }
    async initializeClients() {
        this.logger.info("Initializing contract clients...");
        const deployerWallet = await this.walletManager.getDeployerWallet();
        this.clients.hydro = new HydroBase_client_1.HydroBaseClient(deployerWallet.client, deployerWallet.address, this.contractAddresses.hydro);
        this.clients.tribute = new TributeBase_client_1.TributeBaseClient(deployerWallet.client, deployerWallet.address, this.contractAddresses.tribute);
        this.clients.zephyrus = new ZephyrusMain_client_1.ZephyrusMainClient(deployerWallet.client, deployerWallet.address, this.contractAddresses.zephyrus);
        this.logger.info("Contract clients initialized successfully");
    }
    async executeScenario(scenario) {
        try {
            this.logger.section("Executing Scenario");
            // Initialize clients
            await this.initializeClients();
            // Step 1: Create tribute proposals
            const proposalIds = await this.createTributeProposals(scenario.proposals);
            // Step 2: Create vessels for all users
            const vesselIds = await this.createVesselsForUsers(scenario.users);
            // Step 3: Execute votes according to scenario
            await this.executeVotes(scenario, vesselIds, proposalIds);
            // Step 4: Wait for round progression (in real test, this would be longer)
            await this.waitForRoundProgression();
            // Step 5: Deploy liquidity (simulate end of round)
            await this.simulateLiquidityDeployment();
            const transactionHashes = []; // Collect all tx hashes during execution
            return {
                transactionHashes,
                vesselIds,
                proposalIds,
                success: true
            };
        }
        catch (error) {
            this.logger.error("Scenario execution failed", error);
            return {
                transactionHashes: [],
                vesselIds: {},
                proposalIds: [],
                success: false,
                error: error instanceof Error ? error.message : String(error)
            };
        }
    }
    async createTributeProposals(proposals) {
        this.logger.info("Creating tribute proposals...");
        const proposalIds = [];
        for (const proposal of proposals) {
            // Create proposal in Hydro contract
            const createProposalMsg = {
                create_proposal: {
                    tranche_id: 1, // Using tranche 1 as default
                    title: `Test Proposal ${proposal.id}`,
                    description: `Test proposal for ${proposal.bid_duration_months} months`,
                    deployment_duration: proposal.bid_duration_months,
                    minimum_atom_liquidity_request: "1000000000" // 1000 ATOM
                }
            };
            const result = await this.clients.hydro.createProposal({
                trancheId: 1,
                title: `Test Proposal ${proposal.id}`,
                description: `Test proposal for ${proposal.bid_duration_months} months`,
                deploymentDuration: proposal.bid_duration_months,
                minimumAtomLiquidityRequest: "1000000000"
            });
            // Extract proposal ID from transaction events
            const hydromancerProposalId = test_utils_1.ContractUtils.extractAttributeFromEvents(result, "wasm", "proposal_id");
            if (hydromancerProposalId) {
                const proposalIdNum = parseInt(hydromancerProposalId);
                proposalIds.push(proposalIdNum);
                // Add tributes to the proposal
                for (const tribute of proposal.tributes) {
                    await this.addTributeToProposal(proposalIdNum, tribute);
                }
                this.logger.info(`Created proposal ${proposalIdNum} with ${proposal.tributes.length} tributes`);
            }
            else {
                throw new Error(`Failed to extract proposal ID from transaction`);
            }
        }
        return proposalIds;
    }
    async addTributeToProposal(proposalId, tribute) {
        // Get current round info
        const currentRound = await this.clients.hydro.currentRound();
        const addTributeMsg = {
            add_tribute: {
                round_id: currentRound.round_id,
                tranche_id: 1,
                proposal_id: proposalId
            }
        };
        // Convert tribute amount and denom for blockchain
        const amount = test_utils_1.TokenUtils.parseAmount(tribute.amount);
        const denom = (0, config_1.getTokenDenom)(tribute.denom);
        const funds = [{ denom, amount }];
        await this.clients.tribute.addTribute({
            roundId: currentRound.round_id,
            trancheId: 1,
            proposalId: proposalId
        }, "auto", undefined, funds);
        this.logger.info(`Added tribute: ${amount}${denom} to proposal ${proposalId}`);
    }
    async createVesselsForUsers(users) {
        this.logger.info("Creating vessels for all users...");
        const vesselIds = {};
        for (const user of users) {
            vesselIds[user.user_id] = [];
            for (const vessel of user.vessels) {
                const vesselId = await this.createVessel(user.user_id, vessel);
                vesselIds[user.user_id].push(vesselId);
            }
            this.logger.info(`Created ${user.vessels.length} vessels for user ${user.user_id}`);
        }
        return vesselIds;
    }
    async createVessel(userId, vessel) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            throw new Error(`Test wallet not found for user: ${userId}`);
        }
        // Create vessel in Hydro contract first
        const lockDuration = vessel.lock_duration_months * 30 * 24 * 60 * 60 * 1000000000; // Convert months to nanoseconds
        const amount = test_utils_1.TokenUtils.parseAmount(vessel.locked_amount);
        const denom = (0, config_1.getTokenDenom)(vessel.locked_denom);
        const lockTokensMsg = {
            lock_tokens: {
                lock_duration: lockDuration.toString()
            }
        };
        const funds = [{ denom, amount }];
        // Create Hydro client for this user
        const hydroClient = new HydroBase_client_1.HydroBaseClient(userWallet.client, userWallet.address, this.contractAddresses.hydro);
        const result = await hydroClient.lockTokens({
            lockDuration: lockDuration
        }, "auto", undefined, funds);
        // Extract lock ID from transaction events
        const lockIdStr = test_utils_1.ContractUtils.extractAttributeFromEvents(result, "wasm", "lock_id");
        if (!lockIdStr) {
            throw new Error("Failed to extract lock ID from transaction");
        }
        const lockId = parseInt(lockIdStr);
        // If controlled by hydromancer, delegate to Zephyrus
        if (vessel.controlled_by === "hydromancer") {
            await this.delegateVesselToZephyrus(userId, [lockId]);
        }
        this.logger.info(`Created vessel ${lockId} for user ${userId} (${vessel.controlled_by} controlled)`);
        return lockId;
    }
    async delegateVesselToZephyrus(userId, vesselIds) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            throw new Error(`Test wallet not found for user: ${userId}`);
        }
        // Create Zephyrus client for this user
        const zephyrusClient = new ZephyrusMain_client_1.ZephyrusMainClient(userWallet.client, userWallet.address, this.contractAddresses.zephyrus);
        const takeControlMsg = {
            take_control: {
                vessel_ids: vesselIds
            }
        };
        await zephyrusClient.takeControl({
            vesselIds: vesselIds
        });
        this.logger.info(`Delegated vessels ${vesselIds.join(", ")} to Zephyrus for user ${userId}`);
    }
    async executeVotes(scenario, vesselIds, proposalIds) {
        this.logger.info("Executing votes according to scenario...");
        // Create a mapping of scenario vessel IDs to actual blockchain vessel IDs
        const vesselMapping = this.createVesselMapping(scenario.users, vesselIds);
        for (const user of scenario.users) {
            const userVotes = this.getUserVotes(user, vesselMapping, proposalIds);
            if (userVotes.length > 0) {
                await this.executeUserVotes(user.user_id, userVotes);
            }
        }
    }
    createVesselMapping(users, vesselIds) {
        const mapping = new Map();
        for (const user of users) {
            const userVesselIds = vesselIds[user.user_id] || [];
            for (let i = 0; i < user.vessels.length && i < userVesselIds.length; i++) {
                const scenarioVesselId = user.vessels[i].id;
                const blockchainVesselId = userVesselIds[i];
                mapping.set(scenarioVesselId, blockchainVesselId);
            }
        }
        return mapping;
    }
    getUserVotes(user, vesselMapping, proposalIds) {
        const voteGroups = new Map();
        for (const vessel of user.vessels) {
            if (vessel.voted_proposal_id !== null) {
                const blockchainVesselId = vesselMapping.get(vessel.id);
                if (!blockchainVesselId) {
                    this.logger.warn(`No blockchain vessel ID found for scenario vessel ${vessel.id}`);
                    continue;
                }
                // Map scenario proposal ID to blockchain proposal ID
                const proposalIndex = vessel.voted_proposal_id - 1; // Scenario IDs start from 1
                const blockchainProposalId = proposalIds[proposalIndex];
                if (blockchainProposalId !== undefined) {
                    if (!voteGroups.has(blockchainProposalId)) {
                        voteGroups.set(blockchainProposalId, []);
                    }
                    voteGroups.get(blockchainProposalId).push(blockchainVesselId);
                }
            }
        }
        return Array.from(voteGroups.entries()).map(([proposalId, vesselIds]) => ({
            proposalId,
            vesselIds
        }));
    }
    async executeUserVotes(userId, votes) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            throw new Error(`Test wallet not found for user: ${userId}`);
        }
        for (const vote of votes) {
            // Check if vessels are controlled by Zephyrus or user directly
            const isZephyrusControlled = await this.areVesselsZephyrusControlled(userId, vote.vesselIds);
            if (isZephyrusControlled) {
                // Vote through Zephyrus
                await this.voteViaZephyrus(userId, vote.proposalId, vote.vesselIds);
            }
            else {
                // Vote directly through Hydro
                await this.voteViaHydro(userId, vote.proposalId, vote.vesselIds);
            }
        }
        this.logger.info(`Executed ${votes.length} vote groups for user ${userId}`);
    }
    async areVesselsZephyrusControlled(userId, vesselIds) {
        // Query Zephyrus to see if it controls these vessels
        // For now, assume vessels are Zephyrus-controlled if we delegated them earlier
        // In a full implementation, we'd query the contract state
        return true; // Simplified for this example
    }
    async voteViaZephyrus(userId, proposalId, vesselIds) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            throw new Error(`Test wallet not found for user: ${userId}`);
        }
        const zephyrusClient = new ZephyrusMain_client_1.ZephyrusMainClient(userWallet.client, userWallet.address, this.contractAddresses.zephyrus);
        await zephyrusClient.userVote({
            trancheId: 1,
            vesselsHarbors: [{
                    harbor_id: proposalId,
                    vessel_ids: vesselIds
                }]
        });
        this.logger.info(`User ${userId} voted via Zephyrus: proposal ${proposalId}, vessels ${vesselIds.join(", ")}`);
    }
    async voteViaHydro(userId, proposalId, vesselIds) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
            throw new Error(`Test wallet not found for user: ${userId}`);
        }
        const hydroClient = new HydroBase_client_1.HydroBaseClient(userWallet.client, userWallet.address, this.contractAddresses.hydro);
        await hydroClient.vote({
            trancheId: 1,
            proposalsVotes: [{
                    proposal_id: proposalId,
                    lock_ids: vesselIds
                }]
        });
        this.logger.info(`User ${userId} voted via Hydro: proposal ${proposalId}, vessels ${vesselIds.join(", ")}`);
    }
    async waitForRoundProgression() {
        this.logger.info("Waiting for round progression...");
        // In a real test, we'd wait for the actual round to progress
        // For testing purposes, we'll simulate a short wait
        await test_utils_1.ContractUtils.wait(5000); // 5 seconds
        this.logger.info("Round progression simulated");
    }
    async simulateLiquidityDeployment() {
        this.logger.info("Simulating liquidity deployment...");
        // In a real test, we'd call the liquidity deployment functions
        // This would trigger the end of round and make tributes claimable
        const deployerWallet = await this.walletManager.getDeployerWallet();
        // Get current round and first proposal
        const currentRound = await this.clients.hydro.currentRound();
        const proposals = await this.clients.hydro.roundProposals({
            roundId: currentRound.round_id,
            trancheId: 1,
            startFrom: 0,
            limit: 10
        });
        if (proposals.proposals && proposals.proposals.length > 0) {
            const firstProposal = proposals.proposals[0];
            // Simulate liquidity deployment
            await this.clients.hydro.addLiquidityDeployment({
                deployedFunds: [{
                        amount: "1000000000000", // 1000 ATOM
                        denom: "uatom"
                    }],
                destinations: [],
                fundsBeforeDeployment: [],
                proposalId: firstProposal.proposal_id,
                remainingRounds: 3,
                roundId: currentRound.round_id,
                totalRounds: 3,
                trancheId: 1
            });
            this.logger.info("Liquidity deployment simulated");
        }
    }
}
exports.ScenarioExecutor = ScenarioExecutor;
//# sourceMappingURL=scenario-executor.js.map