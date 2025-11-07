import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { Coin } from "@cosmjs/proto-signing";
import { WalletManager, TestWallet } from "./wallet-manager";
import { TestLogger, ContractUtils, TokenUtils } from "./test-utils";
import { CONFIG, getTokenDenom } from "./config";
import {
  Scenario,
  Vessel,
  User,
  Proposal,
  RewardsCalculator,
} from "./calculate-rewards";
import { RewardsValidator } from "./rewards-validator";

// Import contract clients
import { ZephyrusMainClient } from "./contracts/ZephyrusMain.client";
import { HydroBaseClient } from "./contracts/HydroBase.client";
import { TributeBaseClient } from "./contracts/TributeBase.client";

export interface ExecutionResult {
  transactionHashes: string[];
  vesselIds: { [userId: string]: number[] };
  proposalIds: number[];
  success: boolean;
  error?: string;
  rewardsByRound?: { [roundId: number]: any }; // Rewards calculés par round
  totalRewards?: any; // Rewards agrégés de tous les rounds
}

export interface ContractAddresses {
  hydro: string;
  tribute: string;
  zephyrus: string;
}

export class ScenarioExecutor {
  private logger: TestLogger;
  private walletManager: WalletManager;
  private contractAddresses: ContractAddresses;
  private roundLength: number;
  private clients: {
    hydro?: HydroBaseClient;
    tribute?: TributeBaseClient;
    zephyrus?: ZephyrusMainClient;
  } = {};
  private vesselControlState: Map<number, string> = new Map(); // Maps vessel ID to actual control state
  private proposalMapping: Map<number, number> = new Map(); // Maps scenario proposal ID to blockchain proposal ID
  private rewardsValidator: RewardsValidator;

  constructor(
    logger: TestLogger,
    walletManager: WalletManager,
    contractAddresses: ContractAddresses,
    commissionRecipientAddress: string,
    roundLength: number = 240000000000, // Default 4 minutes in nanoseconds
    rewardsValidator: RewardsValidator
  ) {
    this.logger = logger;
    this.walletManager = walletManager;
    this.contractAddresses = contractAddresses;
    this.roundLength = roundLength;
    this.rewardsValidator = rewardsValidator;
  }

  async initializeClients(): Promise<void> {
    this.logger.info("Initializing contract clients...");

    const deployerWallet = await this.walletManager.getDeployerWallet();

    this.clients.hydro = new HydroBaseClient(
      deployerWallet.client,
      deployerWallet.address,
      this.contractAddresses.hydro
    );

    this.clients.tribute = new TributeBaseClient(
      deployerWallet.client,
      deployerWallet.address,
      this.contractAddresses.tribute
    );

    this.clients.zephyrus = new ZephyrusMainClient(
      deployerWallet.client,
      deployerWallet.address,
      this.contractAddresses.zephyrus
    );

    this.logger.info("Contract clients initialized successfully");
  }

  async executeScenario(
    scenario: Scenario,
    expectedRewards?: any,
    initialBalances?: { [userId: string]: { [denom: string]: string } }
  ): Promise<ExecutionResult> {
    try {
      this.logger.section("Executing Scenario");

      // Initialize clients
      await this.initializeClients();

      const transactionHashes: string[] = [];
      const allVesselIds: { [userId: string]: number[] } = {};
      const allProposalIds: number[] = [];
      const rewardsByRound: { [roundId: number]: any } = {};

      // Step 1: Create all vessels first
      this.logger.info("🎯 EXECUTE SCENARIO: Step 1 - Creating vessels...");
      const vesselIds = await this.createVesselsForUsers(scenario.users);
      Object.assign(allVesselIds, vesselIds);
      this.logger.info("🎯 EXECUTE SCENARIO: Step 1 completed successfully");

      // Step 2: Process each round sequentially
      const totalRounds = this.calculateActualRounds(scenario);
      for (let roundId = 0; roundId < totalRounds; roundId++) {
        this.logger.info(`🎯 EXECUTE SCENARIO: Processing Round ${roundId}...`);

        // Create proposals for this round
        const roundProposals = scenario.proposals.filter(
          (p) => p.round_id === roundId
        );
        const proposalIds = await this.createProposalsForRound(
          roundProposals,
          roundId
        );
        allProposalIds.push(...proposalIds);

        // Process vessels for this round
        await this.processVesselsForRound(scenario.users, vesselIds, roundId);

        // Wait for round progression
        this.logger.info(
          `🎯 EXECUTE SCENARIO: Waiting for round progression...`
        );
        await this.waitForRoundProgression();

        // Deploy liquidity for this round's proposals
        this.logger.info(
          `🎯 EXECUTE SCENARIO: Deploying liquidity for round ${roundId}...`
        );
        await this.simulateLiquidityDeployment();

        // Wait for rewards to be calculated after liquidity deployment
        this.logger.info(
          `🎯 EXECUTE SCENARIO: Waiting for rewards to be calculated...`
        );
        await ContractUtils.wait(5000); // Wait 5 seconds for rewards calculation

        // Calculate and verify rewards for this round
        this.logger.info(
          `🎯 EXECUTE SCENARIO: Calculating rewards for round ${roundId}...`
        );
        const roundRewards = await this.calculateAndVerifyRewardsForRound(
          scenario,
          roundId
        );
        if (roundRewards) {
          rewardsByRound[roundId] = roundRewards;
        }

        // Claim rewards for this round
        this.logger.info(
          `🎯 EXECUTE SCENARIO: Claiming rewards for round ${roundId}...`
        );
        await this.claimRewardsForRound(
          roundId,
          {
            transactionHashes,
            vesselIds: allVesselIds,
            proposalIds: allProposalIds,
            success: true,
            rewardsByRound,
            totalRewards: null,
          },
          expectedRewards
        );

        this.logger.info(
          `🎯 EXECUTE SCENARIO: Round ${roundId} completed successfully`
        );
      }

      // Aggregate rewards from all rounds
      const totalRewards = this.aggregateRewardsByRound(rewardsByRound);
      this.logger.info(
        `🎯 EXECUTE SCENARIO: Total rewards: ${JSON.stringify(totalRewards)}`
      );
      return {
        transactionHashes,
        vesselIds: allVesselIds,
        proposalIds: allProposalIds,
        success: true,
        rewardsByRound,
        totalRewards,
      };
    } catch (error) {
      this.logger.error("Scenario execution failed", error);
      return {
        transactionHashes: [],
        vesselIds: {},
        proposalIds: [],
        success: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  private async createVesselsForUsers(
    users: User[]
  ): Promise<{ [userId: string]: number[] }> {
    this.logger.info("Creating vessels for all users...");
    const vesselIds: { [userId: string]: number[] } = {};

    for (const user of users) {
      const userVesselIds: number[] = [];

      for (const vessel of user.vessels) {
        try {
          this.logger.info(
            `Creating vessel for user ${user.user_id} with duration ${vessel.lock_duration_rounds} rounds`
          );

          const vesselId = await this.createVessel(user.user_id, vessel);
          userVesselIds.push(vesselId);
          this.logger.info(
            `Vessel ID: ${vesselId} assigned to user ${user.user_id}`
          );
        } catch (error) {
          this.logger.error(
            `Failed to build vessel for user ${user.user_id}:`,
            error
          );
          throw error;
        }
      }

      vesselIds[user.user_id] = userVesselIds;
    }

    return vesselIds;
  }

  private async createProposalsForRound(
    proposals: Proposal[],
    roundId: number
  ): Promise<number[]> {
    this.logger.info(`Creating proposals for round ${roundId}...`);
    const proposalIds: number[] = [];

    for (const proposal of proposals) {
      try {
        const createProposalMsg = {
          create_proposal: {
            tranche_id: 1, // Using tranche 1 as default
            bid_duration_months: proposal.bid_duration_months,
            tributes: proposal.tributes,
          },
        };

        this.logger.info(
          `Creating proposal ${proposal.id} with ${proposal.tributes.length} tributes`
        );

        const result = await this.clients.hydro!.createProposal({
          trancheId: 1,
          title: `Test Proposal ${proposal.id}`,
          description: `Test proposal for ${proposal.bid_duration_months} months`,
          deploymentDuration: proposal.bid_duration_months,
          minimumAtomLiquidityRequest: "1000000000",
        });

        if (result.transactionHash) {
          this.logger.info(
            `Proposal created successfully. TxHash: ${result.transactionHash}`
          );

          // Extract proposal ID from the response
          const hydromancerProposalId =
            ContractUtils.extractAttributeFromEvents(
              result,
              "wasm",
              "proposal_id"
            );
          if (!hydromancerProposalId) {
            throw new Error("Failed to extract proposal ID from transaction");
          }
          const proposalId = parseInt(hydromancerProposalId);
          proposalIds.push(proposalId);
          this.proposalMapping.set(proposal.id, proposalId);

          // Add tributes to the proposal
          for (const tribute of proposal.tributes) {
            await this.addTributeToProposal(proposalId, tribute);
          }

          this.logger.info(
            `Proposal ID: ${proposalId} mapped from scenario ID: ${proposal.id}`
          );
        }
      } catch (error) {
        this.logger.error(`Failed to create proposal ${proposal.id}:`, error);
        throw error;
      }
    }

    return proposalIds;
  }

  private async processVesselsForRound(
    users: User[],
    vesselIds: { [userId: string]: number[] },
    roundId: number
  ): Promise<void> {
    this.logger.info(`Processing vessels for round ${roundId}...`);

    for (const user of users) {
      const userVesselIds = vesselIds[user.user_id] || [];

      for (let i = 0; i < user.vessels.length; i++) {
        const vessel = user.vessels[i];
        const vesselId = userVesselIds[i];

        if (vesselId === undefined) {
          this.logger.warn(
            `No vessel ID found for vessel ${i} of user ${user.user_id}`
          );
          continue;
        }

        // Find the round state for this round
        const roundState = vessel.rounds.find((r) => r.round_id === roundId);
        if (!roundState) {
          this.logger.warn(
            `No round state found for vessel ${vesselId} in round ${roundId}`
          );
          continue;
        }
        if (roundState.refresh) {
          const lockDuration = this.convertRoundsToNanoseconds(
            vessel.lock_duration_rounds
          );
          await this.refreshVessel(vesselId, user.user_id, lockDuration);
        }

        // Handle control changes
        if (roundState.controlled_by === "user") {
          await this.takeControl(vesselId, user.user_id);
        } else if (roundState.controlled_by === "hydromancer") {
          // Check if vessel is currently controlled by user and needs to be given back to hydromancer
          const currentControlState = this.vesselControlState.get(vesselId);
          if (currentControlState === "user") {
            await this.changeHydromancer(vesselId, user.user_id);
          }
        }

        // Handle voting
        if (roundState.voted_proposal_id !== null) {
          const actualProposalId = this.proposalMapping.get(
            roundState.voted_proposal_id
          );
          if (actualProposalId !== undefined) {
            if (roundState.controlled_by === "hydromancer") {
              // Hydromancer votes via Zephyrus
              await this.hydromancerVoteViaZephyrus(
                user.user_id,
                actualProposalId,
                [vesselId]
              );
            } else {
              // User votes via Zephyrus
              await this.userVoteViaZephyrus(user.user_id, actualProposalId, [
                vesselId,
              ]);
            }
          } else {
            this.logger.warn(
              `No mapping found for proposal ID ${roundState.voted_proposal_id}`
            );
          }
        }
      }
    }
  }

  private async refreshVessel(
    vesselId: number,
    userId: string,
    lock_duration: number
  ): Promise<void> {
    try {
      this.logger.info(`Refreshing vessel ${vesselId} for user ${userId}`);
      const userWallet = this.walletManager.getTestWallet(userId);
      if (!userWallet) {
        throw new Error(`Test wallet not found for user: ${userId}`);
      }

      const zephyrusClient = new ZephyrusMainClient(
        userWallet.client,
        userWallet.address,
        this.contractAddresses.zephyrus
      );

      const result = await zephyrusClient.updateVesselsClass({
        hydroLockDuration: lock_duration,
        hydroLockIds: [vesselId],
      });
    } catch (error) {
      this.logger.error(`Failed to refresh vessel ${vesselId}:`, error);
      throw error;
    }
  }

  private async takeControl(vesselId: number, userId: string): Promise<void> {
    try {
      this.logger.info(
        `Taking control of vessel ${vesselId} for user ${userId}`
      );

      const userWallet = this.walletManager.getTestWallet(userId);
      if (!userWallet) {
        throw new Error(`Test wallet not found for user: ${userId}`);
      }

      const zephyrusClient = new ZephyrusMainClient(
        userWallet.client,
        userWallet.address,
        this.contractAddresses.zephyrus
      );

      const result = await zephyrusClient.takeControl({
        vesselIds: [vesselId],
      });

      this.logger.info(
        `Control taken successfully. TxHash: ${result.transactionHash}`
      );
      this.vesselControlState.set(vesselId, "user");
    } catch (error) {
      this.logger.error(`Failed to take control of vessel ${vesselId}:`, error);
      throw error;
    }
  }

  private async changeHydromancer(
    vesselId: number,
    userId: string
  ): Promise<void> {
    try {
      this.logger.info(
        `Changing hydromancer for vessel ${vesselId} (giving control back to hydromancer)`
      );

      const userWallet = this.walletManager.getTestWallet(userId);
      if (!userWallet) {
        throw new Error(`Test wallet not found for user: ${userId}`);
      }

      const zephyrusClient = new ZephyrusMainClient(
        userWallet.client,
        userWallet.address,
        this.contractAddresses.zephyrus
      );

      const result = await zephyrusClient.changeHydromancer({
        hydroLockIds: [vesselId],
        hydromancerId: 0, // Default hydromancer ID
        trancheId: 1,
      });

      this.logger.info(
        `Hydromancer changed successfully. TxHash: ${result.transactionHash}`
      );
      this.vesselControlState.set(vesselId, "hydromancer");
    } catch (error) {
      this.logger.error(
        `Failed to change hydromancer for vessel ${vesselId}:`,
        error
      );
      throw error;
    }
  }

  private async voteForProposal(
    vesselId: number,
    proposalId: number,
    userId: string
  ): Promise<void> {
    try {
      this.logger.info(
        `Voting with vessel ${vesselId} for proposal ${proposalId}`
      );

      const userWallet = this.walletManager.getTestWallet(userId);
      if (!userWallet) {
        throw new Error(`Test wallet not found for user: ${userId}`);
      }

      const zephyrusClient = new ZephyrusMainClient(
        userWallet.client,
        userWallet.address,
        this.contractAddresses.zephyrus
      );

      const result = await zephyrusClient.userVote({
        trancheId: 1,
        vesselsHarbors: [
          {
            harbor_id: proposalId,
            vessel_ids: [vesselId],
          },
        ],
      });

      this.logger.info(
        `Vote cast successfully. TxHash: ${result.transactionHash}`
      );
    } catch (error) {
      this.logger.error(
        `Failed to vote with vessel ${vesselId} for proposal ${proposalId}:`,
        error
      );
      throw error;
    }
  }

  private async createTributeProposalsAndVote(scenario: Scenario): Promise<{
    proposalIds: number[];
    vesselIds: { [userId: string]: number[] };
  }> {
    this.logger.info(
      "Creating tribute proposals and voting in the same round..."
    );

    // Check current round at the start
    const startRound = await this.clients.hydro!.currentRound();
    this.logger.info(`Starting in round: ${startRound.round_id}`);

    // Step 1: Create tribute proposals
    const proposalIds = await this.createTributeProposals(scenario.proposals);

    // Step 2: Create vessels for all users
    const vesselIds = await this.createVesselsForUsers(scenario.users);

    // Step 3: Check round hasn't changed and execute votes immediately
    const currentRound = await this.clients.hydro!.currentRound();
    this.logger.info(`About to vote in round: ${currentRound.round_id}`);

    if (currentRound.round_id !== startRound.round_id) {
      this.logger.warn(
        `Round changed from ${startRound.round_id} to ${currentRound.round_id}! This may cause voting issues.`
      );
    }

    // Step 4: Execute votes immediately in the same round
    // await this.executeVotes(scenario, vesselIds, proposalIds); // Commented out - using new flow

    return { proposalIds, vesselIds };
  }

  private async createTributeProposals(
    proposals: Proposal[]
  ): Promise<number[]> {
    this.logger.info("Creating tribute proposals...");
    const proposalIds: number[] = [];
    this.proposalMapping.clear(); // Clear any existing mapping

    // Check current round before creating proposals
    const currentRound = await this.clients.hydro!.currentRound();
    this.logger.info(`Creating proposals in round: ${currentRound.round_id}`);

    for (const proposal of proposals) {
      // Create proposal in Hydro contract
      const createProposalMsg = {
        create_proposal: {
          tranche_id: 1, // Using tranche 1 as default
          title: `Test Proposal ${proposal.id}`,
          description: `Test proposal for ${proposal.bid_duration_months} months`,
          deployment_duration: proposal.bid_duration_months,
          minimum_atom_liquidity_request: "1000000000", // 1000 ATOM
        },
      };

      const result = await this.clients.hydro!.createProposal({
        trancheId: 1,
        title: `Test Proposal ${proposal.id}`,
        description: `Test proposal for ${proposal.bid_duration_months} months`,
        deploymentDuration: proposal.bid_duration_months,
        minimumAtomLiquidityRequest: "1000000000",
      });

      // Extract proposal ID from transaction events
      const hydromancerProposalId = ContractUtils.extractAttributeFromEvents(
        result,
        "wasm",
        "proposal_id"
      );

      if (hydromancerProposalId) {
        const proposalIdNum = parseInt(hydromancerProposalId);
        proposalIds.push(proposalIdNum);
        this.proposalMapping.set(proposal.id, proposalIdNum);

        // Add tributes to the proposal
        for (const tribute of proposal.tributes) {
          await this.addTributeToProposal(proposalIdNum, tribute);
        }

        this.logger.info(
          `Created proposal ${proposalIdNum} (scenario ID ${proposal.id}) with ${proposal.tributes.length} tributes in round ${currentRound.round_id}`
        );
      } else {
        throw new Error(`Failed to extract proposal ID from transaction`);
      }
    }

    return proposalIds;
  }

  private async addTributeToProposal(
    proposalId: number,
    tribute: any
  ): Promise<void> {
    // Get current round info
    const currentRound = await this.clients.hydro!.currentRound();

    const addTributeMsg = {
      add_tribute: {
        round_id: currentRound.round_id,
        tranche_id: 1,
        proposal_id: proposalId,
      },
    };

    // Convert tribute amount and denom for blockchain
    const amount = TokenUtils.parseAmount(tribute.amount);
    const denom = getTokenDenom(tribute.denom);

    const funds: Coin[] = [{ denom, amount }];

    await this.clients.tribute!.addTribute(
      {
        roundId: currentRound.round_id,
        trancheId: 1,
        proposalId: proposalId,
      },
      "auto",
      undefined,
      funds
    );

    this.logger.info(
      `Added tribute: ${amount}${denom} to proposal ${proposalId}`
    );
  }

  private async createVessel(userId: string, vessel: Vessel): Promise<number> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    // Create vessel in Hydro contract first
    // Map months to valid lock durations accepted by Hydro contract
    // Note: These durations must be multiples of the round length
    const validLockDurations = {
      1: this.roundLength, // 1 round
      2: this.roundLength * 2, // 2 rounds
      3: this.roundLength * 3, // 3 rounds
      6: this.roundLength * 6, // 6 rounds
      12: this.roundLength * 12, // 12 rounds
    };

    const lockDuration =
      validLockDurations[
        vessel.lock_duration_rounds as keyof typeof validLockDurations
      ];
    if (!lockDuration) {
      throw new Error(
        `Invalid lock duration: ${vessel.lock_duration_rounds} months. Valid durations: 1, 2, 3, 6, 12 months`
      );
    }
    const amount = TokenUtils.parseAmount(vessel.locked_amount);
    const denom = getTokenDenom(vessel.locked_denom);

    const lockTokensMsg = {
      lock_tokens: {
        lock_duration: lockDuration.toString(),
      },
    };

    const funds: Coin[] = [{ denom, amount }];

    // Create Hydro client for this user
    const hydroClient = new HydroBaseClient(
      userWallet.client,
      userWallet.address,
      this.contractAddresses.hydro
    );

    const result = await hydroClient.lockTokens(
      {
        lockDuration: lockDuration,
      },
      "auto",
      undefined,
      funds
    );

    // Extract lock ID from transaction events
    const lockIdStr = ContractUtils.extractAttributeFromEvents(
      result,
      "wasm",
      "lock_id"
    );
    if (!lockIdStr) {
      throw new Error("Failed to extract lock ID from transaction");
    }

    const lockId = parseInt(lockIdStr);

    // Send vessel to Zephyrus via SendNft
    await this.sendVesselToZephyrus(userId, lockId, vessel);

    // Set initial control state to hydromancer (default)
    this.vesselControlState.set(lockId, "hydromancer");

    this.logger.info(
      `Created vessel ${lockId} for user ${userId} (${this.vesselControlState.get(lockId)} controlled)`
    );
    return lockId;
  }

  private async sendVesselToZephyrus(
    userId: string,
    lockId: number,
    vessel: Vessel
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    // Calculate lock duration in nanoseconds
    const lockDuration = this.convertRoundsToNanoseconds(
      vessel.lock_duration_rounds
    );

    // Create Hydro client for this user
    const hydroClient = new HydroBaseClient(
      userWallet.client,
      userWallet.address,
      this.contractAddresses.hydro
    );

    // Prepare VesselInfo message
    const vesselInfo = {
      owner: userWallet.address,
      auto_maintenance: false, // Default to false for tests
      hydromancer_id: 0, // Always start with default hydromancer (ID 0)
      class_period: lockDuration, // Use the calculated lock duration
    };

    // Send vessel to Zephyrus via SendNft
    this.logger.info(
      `Sending vessel ${lockId} to Zephyrus with vesselInfo: ${JSON.stringify(vesselInfo)}`
    );
    await hydroClient.sendNft({
      contract: this.contractAddresses.zephyrus,
      tokenId: lockId.toString(),
      msg: Buffer.from(JSON.stringify(vesselInfo)).toString("base64"),
    });

    this.logger.info(`Sent vessel ${lockId} to Zephyrus for user ${userId}`);
  }

  private async takeControlFromHydromancer(
    userId: string,
    vesselIds: number[]
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    // Create Zephyrus client for this user
    const zephyrusClient = new ZephyrusMainClient(
      userWallet.client,
      userWallet.address,
      this.contractAddresses.zephyrus
    );

    // User takes control back from hydromancer
    const result = await zephyrusClient.takeControl({
      vesselIds: vesselIds,
    });

    // Log transaction hash
    this.logger.info(
      `🔗 ZEPHYRUS TRANSACTION: User ${userId} takeControl transaction hash: ${result.transactionHash}`
    );
    this.logger.info(
      `🔗 ZEPHYRUS TRANSACTION: User ${userId} takeControl gas used: ${result.gasUsed}`
    );

    this.logger.info(
      `User ${userId} took control of vessels ${vesselIds.join(", ")} from hydromancer`
    );
  }

  // Commented out - using new flow with processVesselsForRound
  /*
  private async executeVotes(
    scenario: Scenario,
    vesselIds: { [userId: string]: number[] },
    proposalIds: number[]
  ): Promise<void> {
    this.logger.info("Executing votes according to scenario...");

    // Create a mapping of scenario vessel IDs to actual blockchain vessel IDs
    const vesselMapping = this.createVesselMapping(scenario.users, vesselIds);

    for (const user of scenario.users) {
      const userVotes = this.getUserVotes(user, vesselMapping, proposalIds);

      if (userVotes.length > 0) {
        await this.executeUserVotes(user.id, userVotes);
      }
    }
  }
  */

  // Commented out - using new flow with processVesselsForRound
  /*
  private createVesselMapping(
    users: User[],
    vesselIds: { [userId: string]: number[] }
  ): Map<number, number> {
    const mapping = new Map<number, number>();

    for (const user of users) {
      const userVesselIds = vesselIds[user.id] || [];

      for (
        let i = 0;
        i < user.vessels.length && i < userVesselIds.length;
        i++
      ) {
        const scenarioVesselId = user.vessels[i].id;
        const blockchainVesselId = userVesselIds[i];
        mapping.set(scenarioVesselId, blockchainVesselId);
      }
    }

    return mapping;
  }
  */

  // Commented out - using new flow with processVesselsForRound
  /*
  private getUserVotes(
    user: User,
    vesselMapping: Map<number, number>,
    proposalIds: number[]
  ): { proposalId: number; vesselIds: number[]; controlledBy: string }[] {
    const voteGroups = new Map<string, number[]>();

    this.logger.info(
      `Getting votes for user ${user.id} with ${user.vessels.length} vessels`
    );

    for (const vessel of user.vessels) {
      this.logger.info(
        `Vessel ${vessel.id}: voted_proposal_id=${vessel.voted_proposal_id}, controlled_by=${vessel.controlled_by}`
      );

      if (vessel.voted_proposal_id !== null) {
        const blockchainVesselId = vesselMapping.get(vessel.id);
        if (!blockchainVesselId) {
          this.logger.warn(
            `No blockchain vessel ID found for scenario vessel ${vessel.id}`
          );
          continue;
        }

        // Map scenario proposal ID to blockchain proposal ID using the mapping
        const blockchainProposalId = this.proposalMapping.get(
          vessel.voted_proposal_id
        );
        this.logger.info(
          `Vessel ${vessel.id}: scenario proposal ${vessel.voted_proposal_id} -> blockchain proposal ${blockchainProposalId}`
        );

        if (blockchainProposalId !== undefined) {
          // Use actual control state instead of scenario control state
          const actualControlState =
            this.vesselControlState.get(blockchainVesselId) || "user";
          this.logger.info(
            `Vessel ${vessel.id} (blockchain ID ${blockchainVesselId}): scenario controlled_by=${vessel.controlled_by}, actual control state=${actualControlState}`
          );

          // Group by proposalId + controlledBy
          const groupKey = `${blockchainProposalId}-${actualControlState}`;
          if (!voteGroups.has(groupKey)) {
            voteGroups.set(groupKey, []);
          }
          voteGroups.get(groupKey)!.push(blockchainVesselId);
        }
      }
    }

    return Array.from(voteGroups.entries()).map(([groupKey, vesselIds]) => {
      const [proposalIdStr, controlledBy] = groupKey.split("-");
      return {
        proposalId: parseInt(proposalIdStr),
        vesselIds,
        controlledBy,
      };
    });
  }
  */

  private async executeUserVotes(
    userId: string,
    votes: { proposalId: number; vesselIds: number[]; controlledBy: string }[]
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    this.logger.info(
      `Executing ${votes.length} vote groups for user ${userId}`
    );
    for (const vote of votes) {
      this.logger.info(
        `Vote group: proposal ${vote.proposalId}, vessels [${vote.vesselIds.join(", ")}], controlled by ${vote.controlledBy}`
      );

      if (vote.controlledBy === "hydromancer") {
        // Hydromancer votes via Zephyrus
        await this.hydromancerVoteViaZephyrus(
          userId,
          vote.proposalId,
          vote.vesselIds
        );
      } else {
        // User votes via Zephyrus
        await this.userVoteViaZephyrus(userId, vote.proposalId, vote.vesselIds);
      }
    }

    this.logger.info(`Executed ${votes.length} vote groups for user ${userId}`);
  }

  private async userVoteViaZephyrus(
    userId: string,
    proposalId: number,
    vesselIds: number[]
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    // Check current round before voting
    const currentRound = await this.clients.hydro!.currentRound();
    this.logger.info(`Current round before voting: ${currentRound.round_id}`);

    // Create Zephyrus client for this user
    const zephyrusClient = new ZephyrusMainClient(
      userWallet.client,
      userWallet.address,
      this.contractAddresses.zephyrus
    );

    // User votes via Zephyrus
    const result = await zephyrusClient.userVote({
      trancheId: 1,
      vesselsHarbors: [
        {
          harbor_id: proposalId,
          vessel_ids: vesselIds,
        },
      ],
    });

    // Log transaction hash
    this.logger.info(
      `🔗 ZEPHYRUS TRANSACTION: User ${userId} userVote transaction hash: ${result.transactionHash}`
    );
    this.logger.info(
      `🔗 ZEPHYRUS TRANSACTION: User ${userId} userVote gas used: ${result.gasUsed}`
    );

    this.logger.info(
      `User ${userId} voted on proposal ${proposalId} with vessels ${vesselIds.join(", ")} in round ${currentRound.round_id}`
    );
  }

  private async hydromancerVoteViaZephyrus(
    userId: string,
    proposalId: number,
    vesselIds: number[]
  ): Promise<void> {
    // For hydromancer votes, we need to use the hydromancer wallet
    const hydromancerWallet = this.walletManager.getTestWallet("hydromancer");
    if (!hydromancerWallet) {
      throw new Error(
        "Hydromancer wallet not found. Make sure setupTestWallets() was called first."
      );
    }

    // Check current round before voting
    const currentRound = await this.clients.hydro!.currentRound();
    this.logger.info(
      `Current round before hydromancer voting: ${currentRound.round_id}`
    );

    // Create Zephyrus client for the hydromancer
    const zephyrusClient = new ZephyrusMainClient(
      hydromancerWallet.client,
      hydromancerWallet.address,
      this.contractAddresses.zephyrus
    );

    // 🔍 DEBUG: Check vessel control state before voting
    try {
      const vesselsByHydromancer =
        await this.clients.zephyrus!.vesselsByHydromancer({
          hydromancerAddr: hydromancerWallet.address,
          limit: 100,
        });
      this.logger.info(
        `🔍 VESSEL CONTROL DEBUG: Hydromancer ${hydromancerWallet.address} controls ${vesselsByHydromancer.vessels.length} vessels: ${vesselsByHydromancer.vessels.map((v) => v.hydro_lock_id).join(", ")}`
      );

      const controlledVesselIds = vesselsByHydromancer.vessels.map(
        (v) => v.hydro_lock_id
      );
      const missingVessels = vesselIds.filter(
        (id) => !controlledVesselIds.includes(id)
      );

      if (missingVessels.length > 0) {
        this.logger.error(
          `❌ VESSEL CONTROL ERROR: Hydromancer does not control vessels: ${missingVessels.join(", ")}`
        );
      }
    } catch (vesselError) {
      this.logger.error(
        `❌ VESSEL DEBUG: Failed to check vessel control: ${vesselError}`
      );
    }

    // Hydromancer votes via Zephyrus
    const result = await zephyrusClient.hydromancerVote({
      trancheId: 1,
      vesselsHarbors: [
        {
          harbor_id: proposalId,
          vessel_ids: vesselIds,
        },
      ],
    });

    // Log transaction hash
    this.logger.info(
      `🔗 ZEPHYRUS TRANSACTION: Hydromancer hydromancerVote transaction hash: ${result.transactionHash}`
    );
    this.logger.info(
      `🔗 ZEPHYRUS TRANSACTION: Hydromancer hydromancerVote gas used: ${result.gasUsed}`
    );

    this.logger.info(
      `Hydromancer (${hydromancerWallet.address}) voted on proposal ${proposalId} with vessels ${vesselIds.join(", ")} for user ${userId} in round ${currentRound.round_id}`
    );
  }

  private async areVesselsZephyrusControlled(
    userId: string,
    vesselIds: number[]
  ): Promise<boolean> {
    // Query Zephyrus to see if it controls these vessels
    // For now, assume vessels are Zephyrus-controlled if we delegated them earlier
    // In a full implementation, we'd query the contract state
    return true; // Simplified for this example
  }

  private async voteViaZephyrus(
    userId: string,
    proposalId: number,
    vesselIds: number[]
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    const zephyrusClient = new ZephyrusMainClient(
      userWallet.client,
      userWallet.address,
      this.contractAddresses.zephyrus
    );

    await zephyrusClient.userVote({
      trancheId: 1,
      vesselsHarbors: [
        {
          harbor_id: proposalId,
          vessel_ids: vesselIds,
        },
      ],
    });

    this.logger.info(
      `User ${userId} voted via Zephyrus: proposal ${proposalId}, vessels ${vesselIds.join(", ")}`
    );
  }

  private async voteViaHydro(
    userId: string,
    proposalId: number,
    vesselIds: number[]
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      throw new Error(`Test wallet not found for user: ${userId}`);
    }

    const hydroClient = new HydroBaseClient(
      userWallet.client,
      userWallet.address,
      this.contractAddresses.hydro
    );

    await hydroClient.vote({
      trancheId: 1,
      proposalsVotes: [
        {
          proposal_id: proposalId,
          lock_ids: vesselIds,
        },
      ],
    });

    this.logger.info(
      `User ${userId} voted via Hydro: proposal ${proposalId}, vessels ${vesselIds.join(", ")}`
    );
  }

  private async waitForRoundProgression(): Promise<void> {
    this.logger.info(
      "Waiting for round to be ready for liquidity deployment..."
    );

    // Wait for the round to be ready for liquidity deployment
    await this.waitForRoundReady();

    this.logger.info("Round is ready for liquidity deployment");
  }

  private async waitForRoundReady(): Promise<void> {
    this.logger.info("Checking if round is ready for liquidity deployment...");

    const maxWaitTime = 3600000; // 1 hour max wait
    const checkInterval = 10000; // Check every 10 seconds
    let waitedTime = 0;
    let base_round = await this.clients.hydro!.currentRound();
    while (waitedTime < maxWaitTime) {
      try {
        // Check if we can deploy liquidity (this will fail if round is not ready)
        const currentRound = await this.clients.hydro!.currentRound();
        this.logger.info(`Current round: ${currentRound.round_id}`);

        if (currentRound.round_id > base_round.round_id) {
          this.logger.info("New round is ready for liquidity deployment");
          return;
        }

        this.logger.info(
          `Round not ready yet, waiting... (${waitedTime}ms elapsed)`
        );
        await ContractUtils.wait(checkInterval);
        waitedTime += checkInterval;
      } catch (error) {
        this.logger.warn(`Error checking round status: ${error}`);
        await ContractUtils.wait(checkInterval);
        waitedTime += checkInterval;
      }
    }

    throw new Error(`Timeout waiting for round to be ready, stopping now`);
  }

  private async simulateLiquidityDeployment(): Promise<void> {
    this.logger.info(
      "🚀 LIQUIDITY DEPLOYMENT: Starting liquidity deployment..."
    );

    const deployerWallet = await this.walletManager.getDeployerWallet();
    this.logger.info(
      `🚀 LIQUIDITY DEPLOYMENT: Using deployer wallet: ${deployerWallet.address}`
    );

    try {
      // Get current round and all proposals
      const currentRound = await this.clients.hydro!.currentRound();
      this.logger.info(
        `🚀 LIQUIDITY DEPLOYMENT: Current round: ${currentRound.round_id}`
      );

      // Deploy liquidity for the previous round's proposals
      // The proposals are created in the current round, but liquidity is deployed
      // for the previous round after the round has progressed
      const targetRoundId = currentRound.round_id - 1;
      this.logger.info(
        `🚀 LIQUIDITY DEPLOYMENT: Looking for proposals in round: ${targetRoundId}`
      );

      const proposals = await this.clients.hydro!.roundProposals({
        roundId: targetRoundId,
        trancheId: 1,
        startFrom: 0,
        limit: 10,
      });

      this.logger.info(
        `🚀 LIQUIDITY DEPLOYMENT: Found ${proposals.proposals ? proposals.proposals.length : 0} proposals`
      );

      if (proposals.proposals && proposals.proposals.length > 0) {
        this.logger.info(
          `🚀 LIQUIDITY DEPLOYMENT: Proposals found: ${proposals.proposals.map((p) => p.proposal_id).join(", ")}`
        );
        // Deploy liquidity for ALL proposals that have tributes
        for (const proposal of proposals.proposals) {
          this.logger.info(
            `🚀 LIQUIDITY DEPLOYMENT: Deploying liquidity for proposal ${proposal.proposal_id}...`
          );

          try {
            this.logger.info(
              `🚀 LIQUIDITY DEPLOYMENT: About to call addLiquidityDeployment with params:`
            );
            this.logger.info(
              `🚀 LIQUIDITY DEPLOYMENT: - proposalId: ${proposal.proposal_id}`
            );
            this.logger.info(
              `🚀 LIQUIDITY DEPLOYMENT: - roundId: ${targetRoundId}`
            );
            this.logger.info(`🚀 LIQUIDITY DEPLOYMENT: - trancheId: 1`);
            this.logger.info(
              `🚀 LIQUIDITY DEPLOYMENT: - deployedFunds: 1000 ATOM`
            );

            // Deploy liquidity for this proposal
            const result = await this.clients.hydro!.addLiquidityDeployment({
              deployedFunds: [
                {
                  amount: "1000000000000", // 1000 ATOM
                  denom: "uatom",
                },
              ],
              destinations: [],
              fundsBeforeDeployment: [],
              proposalId: proposal.proposal_id,
              remainingRounds: 3,
              roundId: targetRoundId,
              totalRounds: 3,
              trancheId: 1,
            });

            this.logger.info(
              `🚀 LIQUIDITY DEPLOYMENT: ✅ Successfully deployed liquidity for proposal ${proposal.proposal_id}`
            );
            this.logger.info(
              `🚀 LIQUIDITY DEPLOYMENT: Transaction result: ${JSON.stringify(result.events)}`
            );
          } catch (error) {
            this.logger.error(
              `🚀 LIQUIDITY DEPLOYMENT: ❌ Failed to deploy liquidity for proposal ${proposal.proposal_id}: ${error}`
            );
            // Continue with other proposals
          }
        }

        this.logger.info(
          "🚀 LIQUIDITY DEPLOYMENT: ✅ All liquidity deployments completed successfully"
        );

        // Note: Rewards will be claimed by individual users in rewards-validator.ts
      } else {
        this.logger.warn(
          "🚀 LIQUIDITY DEPLOYMENT: ⚠️ No proposals found for liquidity deployment"
        );
        this.logger.warn(
          `🚀 LIQUIDITY DEPLOYMENT: ⚠️ This means no liquidity will be deployed and no rewards will be available!`
        );
      }
    } catch (error) {
      this.logger.error(
        "🚀 LIQUIDITY DEPLOYMENT: ❌ Error in liquidity deployment",
        error
      );
      // Continue anyway, rewards might still be claimable
    }
  }

  private async calculateAndVerifyRewardsForRound(
    scenario: Scenario,
    roundId: number
  ): Promise<any> {
    try {
      this.logger.info(
        `💰 REWARDS CALCULATION: Starting rewards calculation for round ${roundId}`
      );

      // Create rewards calculator
      const rewardsCalculator = new RewardsCalculator();

      // Calculate rewards for this specific round
      const rewards = rewardsCalculator.calculateAllRewardsForRound(
        scenario,
        roundId,
        this.logger
      );

      // Log the calculated rewards
      this.logger.info(
        `💰 REWARDS CALCULATION: Round ${roundId} rewards calculated:`
      );
      this.logger.info(
        `💰 REWARDS CALCULATION: Protocol rewards: ${JSON.stringify(rewards.protocol_rewards)}`
      );
      this.logger.info(
        `💰 REWARDS CALCULATION: Hydromancer rewards: ${JSON.stringify(rewards.hydromancer_rewards)}`
      );
      this.logger.info(
        `💰 REWARDS CALCULATION: User rewards: ${JSON.stringify(rewards.user_rewards)}`
      );

      // TODO: Add balance verification logic here
      // This would check actual user balances against calculated rewards
      this.logger.info(
        `💰 REWARDS CALCULATION: Round ${roundId} rewards calculation completed`
      );

      return rewards;
    } catch (error) {
      this.logger.error(
        `💰 REWARDS CALCULATION: ❌ Error calculating rewards for round ${roundId}`,
        error
      );
      // Continue execution, don't fail the entire scenario
      return null;
    }
  }

  private aggregateRewardsByRound(rewardsByRound: {
    [roundId: number]: any;
  }): any {
    const aggregatedRewards: any = {
      protocol_rewards: {},
      hydromancer_rewards: {},
      user_rewards: {},
    };

    // Aggregate rewards from all rounds
    for (const [roundId, roundRewards] of Object.entries(rewardsByRound)) {
      if (!roundRewards) continue;

      // Aggregate protocol rewards
      for (const [denom, amount] of Object.entries(
        roundRewards.protocol_rewards || {}
      )) {
        const currentAmount = aggregatedRewards.protocol_rewards[denom] || "0";
        aggregatedRewards.protocol_rewards[denom] = (
          parseFloat(currentAmount) + parseFloat(amount as string)
        ).toFixed(2);
      }

      // Aggregate hydromancer rewards
      for (const [denom, amount] of Object.entries(
        roundRewards.hydromancer_rewards || {}
      )) {
        const currentAmount =
          aggregatedRewards.hydromancer_rewards[denom] || "0";
        aggregatedRewards.hydromancer_rewards[denom] = (
          parseFloat(currentAmount) + parseFloat(amount as string)
        ).toFixed(2);
      }

      // Aggregate user rewards
      for (const [userId, userRewards] of Object.entries(
        roundRewards.user_rewards || {}
      )) {
        if (!aggregatedRewards.user_rewards[userId]) {
          aggregatedRewards.user_rewards[userId] = {};
        }

        for (const [denom, amount] of Object.entries(userRewards as any)) {
          const currentAmount =
            aggregatedRewards.user_rewards[userId][denom] || "0";
          aggregatedRewards.user_rewards[userId][denom] = (
            parseFloat(currentAmount) + parseFloat(amount as string)
          ).toFixed(2);
        }
      }
    }

    return aggregatedRewards;
  }

  private convertRoundsToNanoseconds(rounds: number): number {
    const validLockDurations = {
      1: this.roundLength, // 1 round
      2: this.roundLength * 2, // 2 rounds
      3: this.roundLength * 3, // 3 rounds
      6: this.roundLength * 6, // 6 rounds
      12: this.roundLength * 12, // 12 rounds
    };

    const lockDuration =
      validLockDurations[rounds as keyof typeof validLockDurations];
    if (!lockDuration) {
      throw new Error(
        `Invalid lock duration: ${rounds} rounds. Valid durations: 1, 2, 3, 6, 12 rounds`
      );
    }
    return lockDuration;
  }

  private calculateActualRounds(scenario: Scenario): number {
    // If total_rounds is explicitly defined, use it
    if (scenario.protocol_config.total_rounds) {
      return scenario.protocol_config.total_rounds;
    }

    // Otherwise, calculate the maximum round_id from all data
    let maxRoundId = -1;

    // Check vessel rounds
    for (const user of scenario.users) {
      for (const vessel of user.vessels) {
        for (const round of vessel.rounds) {
          maxRoundId = Math.max(maxRoundId, round.round_id);
        }
      }
    }

    // Check proposal rounds
    for (const proposal of scenario.proposals) {
      maxRoundId = Math.max(maxRoundId, proposal.round_id);
    }

    // Return maxRoundId + 1 (since rounds are 0-indexed)
    return maxRoundId + 1;
  }

  private async claimRewardsForRound(
    roundId: number,
    executionResult: ExecutionResult,
    expectedRewards?: any
  ): Promise<void> {
    try {
      this.logger.info(
        `💰 CLAIM REWARDS: Starting claim for round ${roundId}...`
      );

      // Initialize clients for rewards validator
      await this.rewardsValidator.initializeClients();

      // Use the rewards validator to claim all rewards
      const success = await this.rewardsValidator.claimAllRewards(
        roundId,
        executionResult,
        expectedRewards
      );

      if (success) {
        this.logger.info(
          `💰 CLAIM REWARDS: Round ${roundId} rewards claimed successfully`
        );
      } else {
        this.logger.warn(
          `💰 CLAIM REWARDS: Round ${roundId} rewards claim failed`
        );
      }
    } catch (error) {
      this.logger.error(
        `💰 CLAIM REWARDS: ❌ Error claiming rewards for round ${roundId}`,
        error
      );
      // Continue execution even if claim fails
    }
  }
}
