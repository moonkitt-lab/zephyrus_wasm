import { WalletManager } from "./wallet-manager";
import {
  TestLogger,
  ReportGenerator,
  TestReport,
  ContractUtils,
} from "./test-utils";
import { RewardsResult } from "./calculate-rewards";
import { ExecutionResult, ContractAddresses } from "./scenario-executor";
import { CONFIG } from "./config";

// Import contract clients
import { ZephyrusMainClient } from "./contracts/ZephyrusMain.client";
import { HydroBaseClient } from "./contracts/HydroBase.client";
import { TributeBaseClient } from "./contracts/TributeBase.client";

export interface ValidationResult {
  success: boolean;
  actualRewards: RewardsResult;
  discrepancies: any[];
  error?: string;
}

export interface ParticipantBalance {
  before: { [token: string]: string };
  after: { [token: string]: string };
  difference: { [token: string]: string };
  expected: { [token: string]: string };
}

export interface BalanceSummary {
  users: { [userId: string]: ParticipantBalance };
  hydromancer: ParticipantBalance;
  commissionRecipient: ParticipantBalance;
  protocol: ParticipantBalance;
}

export interface ClaimableReward {
  denom: string;
  amount: string;
}

export interface UserClaimableRewards {
  [userId: string]: ClaimableReward[];
}

export class RewardsValidator {
  private logger: TestLogger;
  private walletManager: WalletManager;
  private contractAddresses: ContractAddresses;
  private clients: {
    hydro?: HydroBaseClient;
    tribute?: TributeBaseClient;
    zephyrus?: ZephyrusMainClient;
  } = {};
  private actualClaimedRewards: {
    [userId: string]: { [denom: string]: string };
  } = {};
  private roundClaimedRewards: {
    [roundId: number]: { [userId: string]: { [denom: string]: string } };
  } = {};
  private commissionRecipientAddress: string;
  private balanceSummary: BalanceSummary = {
    users: {},
    hydromancer: { before: {}, after: {}, difference: {}, expected: {} },
    commissionRecipient: {
      before: {},
      after: {},
      difference: {},
      expected: {},
    },
    protocol: { before: {}, after: {}, difference: {}, expected: {} },
  };

  private readonly commonDenoms = [
    "untrn",
    "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
    "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
    "ibc/B559A80D62249C8AA07A380E2A2BEA6E5CA9A6F079C912C3A9E9B494105E4F81",
  ];

  constructor(
    logger: TestLogger,
    walletManager: WalletManager,
    contractAddresses: ContractAddresses,
    commissionRecipientAddress: string
  ) {
    this.logger = logger;
    this.walletManager = walletManager;
    this.contractAddresses = contractAddresses;
    this.commissionRecipientAddress = commissionRecipientAddress;
  }

  async initializeClients(): Promise<void> {
    this.logger.info("Initializing validator contract clients...");

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

    this.logger.info("Validator contract clients initialized successfully");
  }

  private mergeAllRoundRewards(): {
    [userId: string]: { [denom: string]: string };
  } {
    this.logger.info("Merging rewards from all rounds...");

    const mergedRewards: { [userId: string]: { [denom: string]: string } } = {};

    // Iterate through all rounds
    for (const [roundIdStr, roundRewards] of Object.entries(
      this.roundClaimedRewards
    )) {
      const roundId = parseInt(roundIdStr);
      this.logger.info(`Merging rewards from round ${roundId}...`);

      // Iterate through all users in this round
      for (const [userId, userRewards] of Object.entries(roundRewards)) {
        if (!mergedRewards[userId]) {
          mergedRewards[userId] = {};
        }

        // Iterate through all tokens for this user in this round
        for (const [denom, amount] of Object.entries(userRewards)) {
          if (!mergedRewards[userId][denom]) {
            mergedRewards[userId][denom] = "0";
          }

          // Add the amount from this round to the total
          const currentTotal = parseFloat(mergedRewards[userId][denom]);
          const roundAmount = parseFloat(amount);
          mergedRewards[userId][denom] = (currentTotal + roundAmount).toFixed(
            2
          );
        }
      }
    }

    // Log the merged results
    for (const [userId, userRewards] of Object.entries(mergedRewards)) {
      this.logger.info(
        `Merged rewards for user ${userId}: ${Object.keys(userRewards).length} tokens`
      );
      for (const [denom, amount] of Object.entries(userRewards)) {
        this.logger.info(`  ${denom}: ${amount}`);
      }
    }

    return mergedRewards;
  }

  async calculateActualRewardsFromClaims(initialBalances: {
    [userId: string]: { [denom: string]: string };
  }): Promise<RewardsResult> {
    this.logger.info("Calculating actual rewards from claimed amounts...");

    // Calculate user rewards by comparing current balances with initial balances
    const userRewards: { [userId: string]: { [denom: string]: string } } = {};

    for (const [userId, initialUserBalances] of Object.entries(
      initialBalances
    )) {
      userRewards[userId] = {};

      // Get current balances for this user
      const currentBalances = await this.getUserTokenBalances(userId);

      // Calculate differences for each token
      for (const [denom, initialAmount] of Object.entries(
        initialUserBalances
      )) {
        const currentAmount = currentBalances[denom] || "0";
        const difference =
          parseFloat(currentAmount) - parseFloat(initialAmount);

        if (difference > 0) {
          const symbol = this.getTokenSymbol(denom);
          userRewards[userId][symbol] = this.formatBalance(
            difference.toString()
          );
        }
      }
    }

    this.logger.info(
      `🔍 DEBUG: userRewards calculated from balance differences: ${JSON.stringify(userRewards)}`
    );

    // Calculate protocol rewards by checking commission recipient balance
    const protocolRewards = await this.calculateProtocolRewards();

    // Calculate hydromancer rewards by comparing current vs initial balance
    const hydromancerRewards: { [denom: string]: string } = {};
    const hydromancerWallet = this.walletManager.getTestWallet("hydromancer");
    if (hydromancerWallet && initialBalances.hydromancer) {
      const currentHydromancerBalances =
        await this.getUserTokenBalances("hydromancer");
      const initialHydromancerBalances = initialBalances.hydromancer;

      // Calculate differences for each token
      for (const [denom, initialAmount] of Object.entries(
        initialHydromancerBalances
      )) {
        const currentAmount = currentHydromancerBalances[denom] || "0";
        const difference =
          parseFloat(currentAmount) - parseFloat(initialAmount);

        if (difference > 0) {
          const symbol = this.getTokenSymbol(denom);
          hydromancerRewards[symbol] = this.formatBalance(
            difference.toString()
          );
        }
      }
    }

    this.logger.info(
      `🔍 DEBUG: hydromancerRewards calculated from balance differences: ${JSON.stringify(hydromancerRewards)}`
    );

    return {
      protocol_rewards: protocolRewards,
      hydromancer_rewards: hydromancerRewards,
      user_rewards: userRewards,
    };
  }

  private async calculateProtocolRewards(): Promise<{
    [denom: string]: string;
  }> {
    this.logger.info(
      "Calculating protocol rewards from commission recipient balance..."
    );

    try {
      const commissionRecipientWallet = this.walletManager.getTestWallet(
        "commission-recipient"
      );
      if (!commissionRecipientWallet) {
        this.logger.error("Commission recipient wallet not found");
        return {};
      }

      const protocolRewards: { [denom: string]: string } = {};

      this.logger.info(
        `Checking commission recipient balance: ${commissionRecipientWallet.address}`
      );

      for (const denom of this.commonDenoms) {
        try {
          const balance = await commissionRecipientWallet.client.getBalance(
            commissionRecipientWallet.address,
            denom
          );
          if (parseFloat(balance.amount) > 0) {
            const symbol = this.getTokenSymbol(denom);
            const humanAmount = (parseFloat(balance.amount) / 1000000).toFixed(
              2
            );
            protocolRewards[symbol] = humanAmount;
            this.logger.info(
              `Protocol reward found in commission recipient: ${symbol}: ${humanAmount}`
            );
          }
        } catch (error) {
          // Token not found or no balance
        }
      }

      // If no protocol rewards found in commission recipient, they might be 0
      if (Object.keys(protocolRewards).length === 0) {
        this.logger.info(
          "No protocol rewards found in commission recipient - commissions may not be taken in this test"
        );
      }

      return protocolRewards;
    } catch (error) {
      this.logger.error("Failed to calculate protocol rewards", error);
      return {};
    }
  }

  private async queryActualRewards(
    executionResult: ExecutionResult
  ): Promise<RewardsResult> {
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
      user_rewards: userRewards,
    };
  }

  private async queryProtocolRewards(): Promise<{ [denom: string]: string }> {
    this.logger.info("Querying protocol rewards...");

    try {
      // Protocol rewards are typically held in the contract's balance
      // Query the contract's balance for each token type
      const deployerWallet = await this.walletManager.getDeployerWallet();

      const protocolRewards: { [denom: string]: string } = {};

      // Check common token balances
      for (const denom of this.commonDenoms) {
        try {
          const balance = await deployerWallet.client.getBalance(
            this.contractAddresses.zephyrus, // Protocol rewards go to Zephyrus
            denom
          );
          if (parseFloat(balance.amount) > 0) {
            protocolRewards[denom] = (
              parseFloat(balance.amount) / 1000000
            ).toFixed(2);
          }
        } catch (error) {
          // Token not found or no balance, skip
        }
      }

      return protocolRewards;
    } catch (error) {
      this.logger.error("Failed to query protocol rewards", error);
      return {};
    }
  }

  private async queryHydromancerRewards(): Promise<{
    [denom: string]: string;
  }> {
    this.logger.info("Querying hydromancer rewards...");

    try {
      // Hydromancer rewards are also tracked in Zephyrus contract
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Query hydromancer's claimable rewards
      // This would require a specific query method in the contract
      const hydromancerRewards: { [denom: string]: string } = {};

      // For now, simulate by checking contract balance
      // In real implementation, there would be specific query methods

      return hydromancerRewards;
    } catch (error) {
      this.logger.error("Failed to query hydromancer rewards", error);
      return {};
    }
  }

  private async queryUserRewards(vesselIds: {
    [userId: string]: number[];
  }): Promise<{ [userId: string]: { [denom: string]: string } }> {
    this.logger.info("Querying user rewards...");

    // Use the merged rewards from all rounds
    const userRewards = this.mergeAllRoundRewards();

    try {
      for (const [userId, userVesselIds] of Object.entries(vesselIds)) {
        this.logger.info(
          `Queried rewards for user ${userId}: ${Object.keys(userRewards[userId] || {}).length} tokens`
        );
      }

      return userRewards;
    } catch (error) {
      this.logger.error("Failed to query user rewards", error);
      return {};
    }
  }

  /**
   * Log les résultats de la query VesselsRewards pour chaque utilisateur avant le claim
   * Cette méthode permet de comparer les calculs théoriques avec les résultats réels du contrat
   *
   * @param roundId - ID du round pour lequel query les rewards
   * @param vesselIdsByUser - Objet avec userId -> array de vesselIds
   * @returns Les résultats de la query pour validation ultérieure
   */
  async logVesselsRewardsBeforeClaim(
    roundId: number,
    vesselIdsByUser: { [userId: string]: number[] }
  ): Promise<{ [userId: string]: { [denom: string]: number } }> {
    this.logger.info("🔍 LOGGING VESSELS REWARDS QUERY RESULTS BEFORE CLAIM");
    this.logger.info("=".repeat(60));

    const queryResults: { [userId: string]: { [denom: string]: number } } = {};

    try {
      for (const [userId, vesselIds] of Object.entries(vesselIdsByUser)) {
        const userWallet = this.walletManager.getTestWallet(userId);
        if (!userWallet) {
          this.logger.warn(`Test wallet not found for user: ${userId}`);
          continue;
        }

        this.logger.info(`\n📊 USER ${userId} - VesselsRewards Query Results:`);
        this.logger.info(`   User Address: ${userWallet.address}`);
        this.logger.info(`   Vessel IDs: [${vesselIds.join(", ")}]`);
        this.logger.info(`   Round ID: ${roundId}`);

        // Debug: Check current round status
        try {
          const currentRound = await this.clients.hydro!.currentRound();
          this.logger.info(
            `   🔍 DEBUG: Current round from Hydro: ${currentRound.round_id}`
          );
        } catch (error) {
          this.logger.warn(`   ⚠️  Could not get current round: ${error}`);
        }

        try {
          // Create Zephyrus client for this user
          const zephyrusClient = new ZephyrusMainClient(
            userWallet.client,
            userWallet.address,
            this.contractAddresses.zephyrus
          );

          // Query VesselsRewards for all vessels of this user in one call
          this.logger.info(`   🔍 DEBUG: Querying VesselsRewards with:`);
          this.logger.info(`   - roundId: ${roundId}`);
          this.logger.info(`   - trancheId: 1`);
          this.logger.info(`   - vesselIds: [${vesselIds.join(", ")}]`);
          this.logger.info(`   - userAddress: ${userWallet.address}`);

          const vesselsRewardsResponse = await zephyrusClient.vesselsRewards({
            roundId: roundId,
            trancheId: 1,
            vesselIds: vesselIds,
            userAddress: userWallet.address,
          });

          this.logger.info(`   Query Response:`);
          this.logger.info(`   - Round ID: ${vesselsRewardsResponse.round_id}`);
          this.logger.info(
            `   - Tranche ID: ${vesselsRewardsResponse.tranche_id}`
          );
          this.logger.info(
            `   - Number of rewards: ${vesselsRewardsResponse.rewards.length}`
          );

          // Debug: Log the full response for troubleshooting
          this.logger.info(
            `   🔍 DEBUG: Full response: ${JSON.stringify(vesselsRewardsResponse, null, 2)}`
          );

          if (vesselsRewardsResponse.rewards.length === 0) {
            this.logger.info(`   ⚠️  No rewards found for user ${userId}`);
            queryResults[userId] = {};
          } else {
            this.logger.info(`   💰 Rewards breakdown:`);

            // Group rewards by denom for better readability
            const rewardsByDenom: {
              [denom: string]: { total: number; details: any[] };
            } = {};

            for (const reward of vesselsRewardsResponse.rewards) {
              const denom = reward.coin.denom;
              const amount = parseFloat(reward.coin.amount) / 1000000; // Convert from micro units

              if (!rewardsByDenom[denom]) {
                rewardsByDenom[denom] = { total: 0, details: [] };
              }

              rewardsByDenom[denom].total += amount;
              rewardsByDenom[denom].details.push({
                proposal_id: reward.proposal_id,
                tribute_id: reward.tribute_id,
                amount: amount.toFixed(6),
              });
            }

            // Log summary by denom with normalized symbols
            for (const [denom, data] of Object.entries(rewardsByDenom)) {
              const normalizedSymbol = this.getTokenSymbol(denom);
              this.logger.info(
                `     ${normalizedSymbol}: ${data.total.toFixed(6)} total`
              );
              this.logger.info(
                `       Details: ${JSON.stringify(data.details, null, 8)}`
              );
            }

            // Store results for validation with normalized token symbols
            queryResults[userId] = {};
            for (const [denom, data] of Object.entries(rewardsByDenom)) {
              const normalizedSymbol = this.getTokenSymbol(denom);
              queryResults[userId][normalizedSymbol] = data.total;
            }
          }
        } catch (error) {
          this.logger.error(
            `   ❌ Error querying VesselsRewards for user ${userId}:`,
            error
          );
        }
      }

      this.logger.info("\n" + "=".repeat(60));
      this.logger.info("🔍 VESSELS REWARDS QUERY LOGGING COMPLETED");

      return queryResults;
    } catch (error) {
      this.logger.error("Failed to log VesselsRewards query results:", error);
      return queryResults;
    }
  }

  /**
   * Valide que les résultats de la query VesselsRewards correspondent aux montants réellement claimés
   *
   * @param queryResults - Résultats de la query VesselsRewards
   * @param claimedRewards - Montants réellement claimés
   * @param tolerance - Tolérance pour les différences (défaut: 0.01)
   * @returns True si les résultats correspondent, false sinon
   */
  validateVesselsRewardsQueryResults(
    queryResults: { [userId: string]: { [denom: string]: number } },
    claimedRewards: { [userId: string]: { [denom: string]: number } },
    tolerance: number = 0.01
  ): { success: boolean; discrepancies: string[] } {
    this.logger.info("🔍 VALIDATING VESSELS REWARDS QUERY RESULTS");
    this.logger.info("=".repeat(60));

    const discrepancies: string[] = [];
    let allMatch = true;

    // Vérifier chaque utilisateur
    for (const [userId, queryUserRewards] of Object.entries(queryResults)) {
      const claimedUserRewards = claimedRewards[userId] || {};

      this.logger.info(`\n📊 USER ${userId} - Validation:`);

      // Vérifier chaque denom
      for (const [denom, queryAmount] of Object.entries(queryUserRewards)) {
        const claimedAmount = claimedUserRewards[denom] || 0;
        const difference = Math.abs(queryAmount - claimedAmount);

        if (difference > tolerance) {
          allMatch = false;
          const discrepancy = `User ${userId} ${denom}: Query=${queryAmount.toFixed(6)}, Claimed=${claimedAmount.toFixed(6)}, Diff=${difference.toFixed(6)}`;
          discrepancies.push(discrepancy);
          this.logger.error(`   ❌ ${discrepancy}`);
        } else {
          this.logger.info(
            `   ✅ ${denom}: Query=${queryAmount.toFixed(6)}, Claimed=${claimedAmount.toFixed(6)} (Diff: ${difference.toFixed(6)})`
          );
        }
      }

      // Vérifier les denoms claimés qui ne sont pas dans la query
      for (const [denom, claimedAmount] of Object.entries(claimedUserRewards)) {
        if (!queryUserRewards[denom] && claimedAmount > tolerance) {
          allMatch = false;
          const discrepancy = `User ${userId} ${denom}: Query=0, Claimed=${claimedAmount.toFixed(6)} (Missing in query)`;
          discrepancies.push(discrepancy);
          this.logger.error(`   ❌ ${discrepancy}`);
        }
      }
    }

    this.logger.info("\n" + "=".repeat(60));
    if (allMatch) {
      this.logger.info("✅ VESSELS REWARDS QUERY VALIDATION PASSED");
    } else {
      this.logger.error("❌ VESSELS REWARDS QUERY VALIDATION FAILED");
      this.logger.error(`Found ${discrepancies.length} discrepancies`);
    }

    return { success: allMatch, discrepancies };
  }

  private async queryUserClaimableRewards(
    userId: string,
    vesselIds: number[]
  ): Promise<ClaimableReward[]> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      this.logger.warn(`Test wallet not found for user: ${userId}`);
      return [];
    }

    try {
      // Create Zephyrus client for this user
      const zephyrusClient = new ZephyrusMainClient(
        userWallet.client,
        userWallet.address,
        this.contractAddresses.zephyrus
      );

      // Query claimable rewards for the current round
      const currentRound = await this.clients.hydro!.currentRound();
      const roundId = currentRound.round_id > 0 ? currentRound.round_id - 1 : 0; // Previous round

      const claimableRewards: ClaimableReward[] = [];

      // Query rewards for each vessel
      for (const vesselId of vesselIds) {
        try {
          // Query claimable rewards for this specific vessel
          const rewardsQuery = {
            claimable_rewards: {
              round_id: roundId,
              tranche_id: 1,
              vessel_id: vesselId,
            },
          };

          const rewards = await zephyrusClient.vesselsRewards({
            roundId: roundId,
            trancheId: 1,
            vesselIds: [vesselId],
            userAddress: userWallet.address,
          });

          // Parse rewards response
          if (rewards && rewards.rewards) {
            for (const reward of rewards.rewards) {
              claimableRewards.push({
                denom: reward.coin.denom,
                amount: (parseFloat(reward.coin.amount) / 1000000).toFixed(2), // Convert from micro units
              });
            }
          }
        } catch (error) {
          // Vessel might not have claimable rewards, continue
          this.logger.debug(
            `No claimable rewards for vessel ${vesselId}: ${error}`
          );
        }
      }

      return claimableRewards;
    } catch (error) {
      this.logger.error(
        `Failed to query claimable rewards for user ${userId}`,
        error
      );
      return [];
    }
  }

  async claimAllRewards(
    roundId: number,
    executionResult: ExecutionResult,
    expectedRewards?: RewardsResult
  ): Promise<boolean> {
    this.logger.info("Claiming all rewards to verify actual amounts...");

    try {
      // Rewards are already claimable after liquidity deployment, no need to wait
      const currentRound = await this.clients.hydro!.currentRound();

      this.logger.info(
        `Claiming rewards for round ${roundId} (current round: ${currentRound.round_id})`
      );

      // Capture initial balances for all participants
      const allParticipants = [...Object.keys(executionResult.vesselIds)];
      await this.captureInitialBalances(allParticipants);

      // Log VesselsRewards query results before claiming
      this.logger.info(
        "🔍 Logging VesselsRewards query results before claim..."
      );
      const queryResults = await this.logVesselsRewardsBeforeClaim(
        roundId,
        executionResult.vesselIds
      );

      // Claim hydromancer rewards
      await this.claimHydromancerRewards(roundId);

      // Claim rewards for each user
      for (const [userId, vesselIds] of Object.entries(
        executionResult.vesselIds
      )) {
        await this.claimUserRewards(userId, vesselIds, roundId);
      }

      // Capture final balances and calculate differences
      await this.captureFinalBalances(allParticipants);

      // Store the claimed rewards for this round
      this.roundClaimedRewards[roundId] = { ...this.actualClaimedRewards };
      this.logger.info(
        `🔍 DEBUG: balanceSummary.users after captureFinalBalances: ${JSON.stringify(this.balanceSummary.users)}`
      );
      this.logger.info(
        `🔍 DEBUG: Stored rewards for round ${roundId}: ${JSON.stringify(this.actualClaimedRewards)}`
      );

      // Validate that query results match claimed amounts
      this.logger.info(
        "🔍 Validating VesselsRewards query results against claimed amounts..."
      );

      // Convert claimed rewards from string to number for validation
      const claimedRewardsForValidation: {
        [userId: string]: { [denom: string]: number };
      } = {};
      for (const [userId, userRewards] of Object.entries(
        this.actualClaimedRewards
      )) {
        claimedRewardsForValidation[userId] = {};
        for (const [denom, amount] of Object.entries(userRewards)) {
          claimedRewardsForValidation[userId][denom] = parseFloat(amount);
        }
      }

      const validationResult = this.validateVesselsRewardsQueryResults(
        queryResults,
        claimedRewardsForValidation
      );

      if (!validationResult.success) {
        this.logger.error("❌ VesselsRewards query validation failed!");
        this.logger.error("Discrepancies found:");
        validationResult.discrepancies.forEach((discrepancy) => {
          this.logger.error(`  - ${discrepancy}`);
        });
        return false;
      }

      this.logger.info("✅ All rewards claimed successfully and validated");
      return true;
    } catch (error) {
      this.logger.error("Failed to claim rewards", error);
      return false;
    }
  }

  async claimFirstUserAgain(
    executionResult: ExecutionResult
  ): Promise<boolean> {
    this.logger.info(
      "Making additional claim for first user to test duplicate claim behavior..."
    );

    try {
      const currentRound = await this.clients.hydro!.currentRound();
      const roundId = currentRound.round_id > 0 ? currentRound.round_id - 1 : 0;

      // Get the first user from executionResult.vesselIds
      const userIds = Object.keys(executionResult.vesselIds);
      if (userIds.length === 0) {
        this.logger.warn("No users found in execution result");
        return false;
      }

      const firstUserId = userIds[0];
      const firstUserVesselIds = executionResult.vesselIds[firstUserId];

      this.logger.info(
        `Making additional claim for first user ${firstUserId} with vessels ${firstUserVesselIds.join(", ")} for round ${roundId}`
      );

      // Log balances before the additional claim
      await this.logZephyrusContractBalance("BEFORE additional claim");
      await this.logHydroContractBalance("BEFORE additional claim");
      await this.logTributeContractBalance("BEFORE additional claim");

      // Make the additional claim (but don't recalculate rewards since they're already stored)
      await this.claimUserRewardsWithoutStoring(
        firstUserId,
        firstUserVesselIds,
        roundId
      );

      // Log balances after the additional claim
      await this.logZephyrusContractBalance("AFTER additional claim");
      await this.logHydroContractBalance("AFTER additional claim");
      await this.logTributeContractBalance("AFTER additional claim");

      this.logger.info(
        "Additional claim for first user completed successfully"
      );
      return true;
    } catch (error) {
      this.logger.error(
        "Failed to make additional claim for first user",
        error
      );
      return false;
    }
  }

  private async claimUserRewards(
    userId: string,
    vesselIds: number[],
    roundId: number
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      this.logger.warn(`Test wallet not found for user: ${userId}`);
      return;
    }

    try {
      // Create Zephyrus client for this user
      const zephyrusClient = new ZephyrusMainClient(
        userWallet.client,
        userWallet.address,
        this.contractAddresses.zephyrus
      );

      // Record balance before claiming
      const balancesBefore = await this.getUserTokenBalances(userId);
      this.logger.info(
        `User ${userId} balance BEFORE claim: ${JSON.stringify(balancesBefore)}`
      );

      // Log Zephyrus contract balance before claim
      await this.logZephyrusContractBalance("BEFORE claim");

      // Log Hydro contract balance to see tributes
      await this.logHydroContractBalance("BEFORE claim");

      // Log Tribute contract balance to see tributes
      await this.logTributeContractBalance("BEFORE claim");

      // Log Commission recipient balance to see protocol rewards
      await this.logCommissionRecipientBalance("BEFORE claim");

      // Claim rewards
      const claimMsg = {
        claim: {
          round_id: roundId,
          tranche_id: 1,
          vessel_ids: vesselIds,
        },
      };

      const result = await zephyrusClient.claim({
        roundId: roundId,
        trancheId: 1,
        vesselIds: vesselIds,
      });

      // Log transaction hash
      this.logger.info(
        `🔗 ZEPHYRUS TRANSACTION: User ${userId} claim transaction hash: ${result.transactionHash}`
      );
      this.logger.info(
        `🔗 ZEPHYRUS TRANSACTION: User ${userId} claim gas used: ${result.gasUsed}`
      );

      // Log Zephyrus contract balance after claim
      await this.logZephyrusContractBalance("AFTER claim");

      // Log Commission recipient balance after claim
      await this.logCommissionRecipientBalance("AFTER claim");

      // Record balance after claiming
      const balancesAfter = await this.getUserTokenBalances(userId);
      this.logger.info(
        `User ${userId} balance AFTER claim: ${JSON.stringify(balancesAfter)}`
      );

      // Log the difference
      this.logBalanceChanges(userId, balancesBefore, balancesAfter);

      // Calculate and store actual claimed rewards
      this.calculateAndStoreClaimedRewards(
        userId,
        balancesBefore,
        balancesAfter
      );

      this.logger.info(
        `Claimed rewards for user ${userId} vessels: ${vesselIds.join(", ")}`
      );
    } catch (error) {
      this.logger.error(
        `Failed to claim rewards for user ${userId} for round ${roundId} and vessels ${vesselIds.join(", ")}`,
        error
      );
    }
  }

  private async claimUserRewardsWithoutStoring(
    userId: string,
    vesselIds: number[],
    roundId: number
  ): Promise<void> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      this.logger.warn(`Test wallet not found for user: ${userId}`);
      return;
    }

    try {
      // Create Zephyrus client for this user
      const zephyrusClient = new ZephyrusMainClient(
        userWallet.client,
        userWallet.address,
        this.contractAddresses.zephyrus
      );

      // Record balance before claiming
      const balancesBefore = await this.getUserTokenBalances(userId);
      this.logger.info(
        `User ${userId} balance BEFORE claim: ${JSON.stringify(balancesBefore)}`
      );

      // Log Zephyrus contract balance before claim
      await this.logZephyrusContractBalance("BEFORE claim");

      // Log Hydro contract balance to see tributes
      await this.logHydroContractBalance("BEFORE claim");

      // Log Tribute contract balance to see tributes
      await this.logTributeContractBalance("BEFORE claim");

      // Claim rewards
      const result = await zephyrusClient.claim({
        roundId: roundId,
        trancheId: 1,
        vesselIds: vesselIds,
      });

      // Log transaction hash
      this.logger.info(
        `🔗 ZEPHYRUS TRANSACTION: User ${userId} claim transaction hash: ${result.transactionHash}`
      );
      this.logger.info(
        `🔗 ZEPHYRUS TRANSACTION: User ${userId} claim gas used: ${result.gasUsed}`
      );

      // Log Zephyrus contract balance after claim
      await this.logZephyrusContractBalance("AFTER claim");

      // Log Commission recipient balance after claim
      await this.logCommissionRecipientBalance("AFTER claim");

      // Record balance after claiming
      const balancesAfter = await this.getUserTokenBalances(userId);
      this.logger.info(
        `User ${userId} balance AFTER claim: ${JSON.stringify(balancesAfter)}`
      );

      // Log the difference (but don't store it)
      this.logBalanceChanges(userId, balancesBefore, balancesAfter);

      this.logger.info(
        `Claimed rewards for user ${userId} vessels: ${vesselIds.join(", ")}`
      );
    } catch (error) {
      this.logger.error(
        `Failed to claim rewards for user ${userId} for round ${roundId} and vessels ${vesselIds.join(", ")}`,
        error
      );
    }
  }

  private async claimHydromancerRewards(roundId: number): Promise<void> {
    try {
      await this.claimUserRewards("hydromancer", [], roundId);

      this.logger.info(`Hydromancer rewards claimed for round ${roundId}`);
    } catch (error) {
      this.logger.error("Failed to claim hydromancer rewards", error);
    }
  }

  private async getUserTokenBalances(
    userId: string
  ): Promise<{ [denom: string]: string }> {
    const userWallet = this.walletManager.getTestWallet(userId);
    if (!userWallet) {
      return {};
    }

    const balances: { [denom: string]: string } = {};

    try {
      // Get balances for common tokens
      for (const denom of this.commonDenoms) {
        try {
          const balance = await userWallet.client.getBalance(
            userWallet.address,
            denom
          );
          balances[balance.denom] = balance.amount;
        } catch (error) {
          // Token not found or no balance
        }
      }
    } catch (error) {
      this.logger.error(
        `Failed to get token balances for user ${userId}`,
        error
      );
    }

    return balances;
  }

  private async logZephyrusContractBalance(context: string): Promise<void> {
    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Get Zephyrus contract balance
      const balances: { [denom: string]: string } = {};

      // Check common token balances
      for (const denom of this.commonDenoms) {
        try {
          const balance = await deployerWallet.client.getBalance(
            this.contractAddresses.zephyrus,
            denom
          );
          if (parseFloat(balance.amount) > 0) {
            const symbol = this.getTokenSymbol(denom);
            balances[symbol] = (parseFloat(balance.amount) / 1000000).toFixed(
              2
            );
          }
        } catch (error) {
          // Token might not exist, continue
        }
      }

      if (Object.keys(balances).length > 0) {
        const balanceStr = Object.entries(balances)
          .map(([symbol, amount]) => `${symbol}: ${amount}`)
          .join(", ");
        this.logger.info(`Zephyrus contract balance ${context}: ${balanceStr}`);
      } else {
        this.logger.info(
          `Zephyrus contract balance ${context}: No tokens found`
        );
      }
    } catch (error) {
      this.logger.error(
        `Failed to get Zephyrus contract balance ${context}:`,
        error
      );
    }
  }

  private async logHydroContractBalance(context: string): Promise<void> {
    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Get Hydro contract balance
      const balances: { [denom: string]: string } = {};

      // Check common token balances
      for (const denom of this.commonDenoms) {
        try {
          const balance = await deployerWallet.client.getBalance(
            this.contractAddresses.hydro,
            denom
          );
          if (parseFloat(balance.amount) > 0) {
            const symbol = this.getTokenSymbol(denom);
            balances[symbol] = (parseFloat(balance.amount) / 1000000).toFixed(
              2
            );
          }
        } catch (error) {
          // Token might not exist, continue
        }
      }

      if (Object.keys(balances).length > 0) {
        const balanceStr = Object.entries(balances)
          .map(([symbol, amount]) => `${symbol}: ${amount}`)
          .join(", ");
        this.logger.info(`Hydro contract balance ${context}: ${balanceStr}`);
      } else {
        this.logger.info(`Hydro contract balance ${context}: No tokens found`);
      }
    } catch (error) {
      this.logger.error(
        `Failed to get Hydro contract balance ${context}:`,
        error
      );
    }
  }

  private async logTributeContractBalance(context: string): Promise<void> {
    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Get Tribute contract balance
      const balances: { [denom: string]: string } = {};

      // Check common token balances
      for (const denom of this.commonDenoms) {
        try {
          const balance = await deployerWallet.client.getBalance(
            this.contractAddresses.tribute,
            denom
          );
          if (parseFloat(balance.amount) > 0) {
            const symbol = this.getTokenSymbol(denom);
            balances[symbol] = (parseFloat(balance.amount) / 1000000).toFixed(
              2
            );
          }
        } catch (error) {
          // Token might not exist, continue
        }
      }

      if (Object.keys(balances).length > 0) {
        const balanceStr = Object.entries(balances)
          .map(([symbol, amount]) => `${symbol}: ${amount}`)
          .join(", ");
        this.logger.info(`Tribute contract balance ${context}: ${balanceStr}`);
      } else {
        this.logger.info(
          `Tribute contract balance ${context}: No tokens found`
        );
      }
    } catch (error) {
      this.logger.error(
        `Failed to get Tribute contract balance ${context}:`,
        error
      );
    }
  }

  private async logCommissionRecipientBalance(context: string): Promise<void> {
    try {
      const commissionRecipientWallet = this.walletManager.getTestWallet(
        "commission-recipient"
      );
      if (!commissionRecipientWallet) {
        this.logger.warn("Commission recipient wallet not found");
        return;
      }

      const balances: { [denom: string]: string } = {};

      // Check common token balances
      for (const denom of this.commonDenoms) {
        try {
          const balance = await commissionRecipientWallet.client.getBalance(
            commissionRecipientWallet.address,
            denom
          );
          if (parseFloat(balance.amount) > 0) {
            const symbol = this.getTokenSymbol(denom);
            balances[symbol] = (parseFloat(balance.amount) / 1000000).toFixed(
              2
            );
          }
        } catch (error) {
          // Token might not exist, continue
        }
      }

      if (Object.keys(balances).length > 0) {
        const balanceStr = Object.entries(balances)
          .map(([symbol, amount]) => `${symbol}: ${amount}`)
          .join(", ");
        this.logger.info(
          `Commission recipient balance ${context}: ${balanceStr}`
        );
      } else {
        this.logger.info(
          `Commission recipient balance ${context}: No tokens found`
        );
      }
    } catch (error) {
      this.logger.error(
        `Failed to get Commission recipient balance ${context}:`,
        error
      );
    }
  }

  private calculateAndStoreClaimedRewards(
    userId: string,
    balancesBefore: { [denom: string]: string },
    balancesAfter: { [denom: string]: string }
  ): void {
    this.logger.info(`🔍 DEBUG: calculateAndStoreClaimedRewards for ${userId}`);
    this.logger.info(
      `🔍 DEBUG: balancesBefore: ${JSON.stringify(balancesBefore)}`
    );
    this.logger.info(
      `🔍 DEBUG: balancesAfter: ${JSON.stringify(balancesAfter)}`
    );

    if (!this.actualClaimedRewards[userId]) {
      this.actualClaimedRewards[userId] = {};
    }

    // Calculate the difference for each token
    for (const denom of Object.keys(balancesAfter)) {
      const beforeAmount = parseFloat(balancesBefore[denom] || "0");
      const afterAmount = parseFloat(balancesAfter[denom] || "0");
      const difference = afterAmount - beforeAmount;

      if (difference > 0) {
        // Convert to human-readable format (divide by 1e6 for most tokens)
        const humanAmount = (difference / 1000000).toFixed(2);
        const symbol = this.getTokenSymbol(denom);
        this.actualClaimedRewards[userId][symbol] = humanAmount;
        this.logger.info(
          `🔍 DEBUG: Stored reward for ${userId}: ${symbol} = ${humanAmount}`
        );
      }
    }
  }

  private logBalanceChanges(
    userId: string,
    before: { [denom: string]: string },
    after: { [denom: string]: string }
  ): void {
    const changes: string[] = [];

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

  private getTokenSymbol(denom: string): string {
    // Convert token denom to readable symbol using centralized config
    if (denom === CONFIG.tokenDenoms.NTRN) return "NTRN";
    if (denom === CONFIG.tokenDenoms.USDC) return "USDC";
    if (denom === CONFIG.tokenDenoms.DATOM) return "dATOM";
    if (denom === CONFIG.tokenDenoms.STATOM) return "stATOM";

    // Fallback for partial matches (for backward compatibility)
    if (denom.includes("usdc")) return "USDC";
    if (denom.includes("udatom")) return "dATOM";
    if (denom.includes("statom")) return "stATOM";

    // Return as-is if no mapping found
    return denom;
  }

  private async captureInitialBalances(userIds: string[]): Promise<void> {
    this.logger.info("📊 Capturing initial balances for all participants...");
    this.logger.info(
      `🔍 DEBUG: userIds to capture: ${JSON.stringify(userIds)}`
    );

    // Capture user balances
    for (const userId of userIds) {
      this.balanceSummary.users[userId] = {
        before: {},
        after: {},
        difference: {},
        expected: {},
      };
      const userBalances = await this.getUserTokenBalances(userId);
      for (const denom of this.commonDenoms) {
        const symbol = this.getTokenSymbol(denom);
        this.balanceSummary.users[userId].before[symbol] = this.formatBalance(
          userBalances[denom] || "0"
        );
      }
    }

    // Capture hydromancer balance
    const hydromancerWallet = this.walletManager.getTestWallet("hydromancer");
    if (hydromancerWallet) {
      for (const denom of this.commonDenoms) {
        try {
          const balance = await hydromancerWallet.client.getBalance(
            hydromancerWallet.address,
            denom
          );
          const symbol = this.getTokenSymbol(denom);
          this.balanceSummary.hydromancer.before[symbol] = this.formatBalance(
            balance.amount
          );
        } catch (error) {
          // Token not found
        }
      }
    }

    // Capture commission recipient balance
    const commissionRecipientWallet = this.walletManager.getTestWallet(
      "commission-recipient"
    );
    if (commissionRecipientWallet) {
      for (const denom of this.commonDenoms) {
        try {
          const balance = await commissionRecipientWallet.client.getBalance(
            commissionRecipientWallet.address,
            denom
          );
          const symbol = this.getTokenSymbol(denom);
          this.balanceSummary.commissionRecipient.before[symbol] =
            this.formatBalance(balance.amount);
        } catch (error) {
          // Token not found
        }
      }
    }

    // Capture protocol (Zephyrus contract) balance
    const deployerWallet = await this.walletManager.getDeployerWallet();
    for (const denom of this.commonDenoms) {
      try {
        const balance = await deployerWallet.client.getBalance(
          this.contractAddresses.zephyrus,
          denom
        );
        const symbol = this.getTokenSymbol(denom);
        this.balanceSummary.protocol.before[symbol] = this.formatBalance(
          balance.amount
        );
      } catch (error) {
        // Token not found
      }
    }
  }

  private async captureFinalBalances(userIds: string[]): Promise<void> {
    this.logger.info("📊 Capturing final balances for all participants...");

    // Capture user balances
    for (const userId of userIds) {
      const userBalances = await this.getUserTokenBalances(userId);
      for (const denom of this.commonDenoms) {
        const symbol = this.getTokenSymbol(denom);
        this.balanceSummary.users[userId].after[symbol] = this.formatBalance(
          userBalances[denom] || "0"
        );

        // Calculate difference
        const before = parseFloat(
          this.balanceSummary.users[userId].before[symbol] || "0"
        );
        const after = parseFloat(
          this.balanceSummary.users[userId].after[symbol] || "0"
        );
        this.balanceSummary.users[userId].difference[symbol] = (
          after - before
        ).toFixed(6);
      }
    }

    // Capture hydromancer balance
    const hydromancerWallet = this.walletManager.getTestWallet("hydromancer");
    if (hydromancerWallet) {
      const deployerWallet = await this.walletManager.getDeployerWallet();
      for (const denom of this.commonDenoms) {
        try {
          const balance = await deployerWallet.client.getBalance(
            hydromancerWallet.address,
            denom
          );
          const symbol = this.getTokenSymbol(denom);
          this.balanceSummary.hydromancer.after[symbol] = this.formatBalance(
            balance.amount
          );

          // Calculate difference
          const before = parseFloat(
            this.balanceSummary.hydromancer.before[symbol] || "0"
          );
          const after = parseFloat(
            this.balanceSummary.hydromancer.after[symbol] || "0"
          );
          this.balanceSummary.hydromancer.difference[symbol] = (
            after - before
          ).toFixed(6);
        } catch (error) {
          // Token not found
        }
      }
    }

    // Capture commission recipient balance
    const commissionRecipientWallet = this.walletManager.getTestWallet(
      "commission-recipient"
    );
    if (commissionRecipientWallet) {
      for (const denom of this.commonDenoms) {
        try {
          const balance = await commissionRecipientWallet.client.getBalance(
            commissionRecipientWallet.address,
            denom
          );
          const symbol = this.getTokenSymbol(denom);
          this.balanceSummary.commissionRecipient.after[symbol] =
            this.formatBalance(balance.amount);

          // Calculate difference
          const before = parseFloat(
            this.balanceSummary.commissionRecipient.before[symbol] || "0"
          );
          const after = parseFloat(
            this.balanceSummary.commissionRecipient.after[symbol] || "0"
          );
          this.balanceSummary.commissionRecipient.difference[symbol] = (
            after - before
          ).toFixed(6);
        } catch (error) {
          // Token not found
        }
      }
    }

    // Capture protocol (Zephyrus contract) balance
    const deployerWallet = await this.walletManager.getDeployerWallet();
    for (const denom of this.commonDenoms) {
      try {
        const balance = await deployerWallet.client.getBalance(
          this.contractAddresses.zephyrus,
          denom
        );
        const symbol = this.getTokenSymbol(denom);
        this.balanceSummary.protocol.after[symbol] = this.formatBalance(
          balance.amount
        );

        // Calculate difference
        const before = parseFloat(
          this.balanceSummary.protocol.before[symbol] || "0"
        );
        const after = parseFloat(
          this.balanceSummary.protocol.after[symbol] || "0"
        );
        this.balanceSummary.protocol.difference[symbol] = (
          after - before
        ).toFixed(6);
      } catch (error) {
        // Token not found
      }
    }
  }

  private formatBalance(amount: string): string {
    return (parseFloat(amount) / 1000000).toFixed(6);
  }

  private createCumulativeDataFromAllRounds(): BalanceSummary {
    const cumulative: BalanceSummary = {
      users: {},
      hydromancer: { before: {}, after: {}, difference: {}, expected: {} },
      commissionRecipient: {
        before: {},
        after: {},
        difference: {},
        expected: {},
      },
      protocol: { before: {}, after: {}, difference: {}, expected: {} },
    };

    // Initialize with current balance summary (Round 1 data)
    cumulative.hydromancer = { ...this.balanceSummary.hydromancer };
    cumulative.commissionRecipient = {
      ...this.balanceSummary.commissionRecipient,
    };
    cumulative.protocol = { ...this.balanceSummary.protocol };
    cumulative.users = { ...this.balanceSummary.users };

    // Add data from all previous rounds (Round 0)
    for (const [roundId, roundRewards] of Object.entries(
      this.roundClaimedRewards
    )) {
      // Skip current round (Round 1) as it's already in balanceSummary
      if (parseInt(roundId) === 1) continue;

      for (const [userId, userRewards] of Object.entries(roundRewards)) {
        // Skip hydromancer as it's handled separately
        if (userId === "hydromancer") continue;

        if (!cumulative.users[userId]) {
          cumulative.users[userId] = {
            before: {},
            after: {},
            difference: {},
            expected: {},
          };
        }

        // Add rewards from this round to cumulative
        for (const [denom, amount] of Object.entries(userRewards)) {
          const currentAmount = parseFloat(
            cumulative.users[userId].difference[denom] || "0"
          );
          const roundAmount = parseFloat(amount);
          cumulative.users[userId].difference[denom] = (
            currentAmount + roundAmount
          ).toFixed(6);
        }
      }
    }

    return cumulative;
  }

  public generateBalanceSummaryTable(expectedRewards: RewardsResult): void {
    this.logger.info("\n" + "=".repeat(120));
    this.logger.info("📊 BALANCE SUMMARY TABLE (CUMULATIVE)");
    this.logger.info("=".repeat(120));

    // Set expected rewards in balance summary
    this.setExpectedBalances(expectedRewards);

    // Create cumulative data from all rounds
    const cumulativeData = this.createCumulativeDataFromAllRounds();

    const tokens = ["NTRN", "dATOM", "stATOM", "USDC"];
    const participants = [
      { name: "Protocol (Zephyrus)", data: cumulativeData.protocol },
      {
        name: "Commission Recipient",
        data: cumulativeData.commissionRecipient,
      },
      { name: "Hydromancer", data: cumulativeData.hydromancer },
      ...Object.entries(cumulativeData.users).map(([userId, data]) => ({
        name: `User ${userId}`,
        data,
      })),
    ];

    for (const token of tokens) {
      const hasData = participants.some(
        (p) =>
          parseFloat(p.data.before[token] || "0") > 0 ||
          parseFloat(p.data.after[token] || "0") > 0 ||
          parseFloat(p.data.expected[token] || "0") > 0
      );

      if (!hasData) continue;

      this.logger.info(`\n🪙 ${token} Token:`);
      this.logger.info(
        "┌──────────────────────┬──────────────┬──────────────┬──────────────┬──────────────┬─────────────┐"
      );
      this.logger.info(
        "│ Participant          │ Before       │ After        │ Actual Diff  │ Expected Diff│ Status      │"
      );
      this.logger.info(
        "├──────────────────────┼──────────────┼──────────────┼──────────────┼──────────────┼─────────────┤"
      );

      for (const participant of participants) {
        const { name, data } = participant;
        const before = data.before[token] || "0.000000";
        const after = data.after[token] || "0.000000";
        const actualDiff = data.difference[token] || "0.000000";
        const expectedDiff = data.expected[token] || "0.000000";

        const actualFloat = parseFloat(actualDiff);
        const expectedFloat = parseFloat(expectedDiff);
        const tolerance = 0.01; // Tolerance for rounding errors

        let status = "✅ Match";
        if (Math.abs(actualFloat - expectedFloat) > tolerance) {
          status = actualFloat > expectedFloat ? "⚠️ Higher" : "❌ Lower";
        }

        this.logger.info(
          `│ ${name.padEnd(20)} │ ${before.padStart(12)} │ ${after.padStart(12)} │ ${actualDiff.padStart(12)} │ ${expectedDiff.padStart(12)} │ ${status.padEnd(11)} │`
        );
      }

      this.logger.info(
        "└──────────────────────┴──────────────┴──────────────┴──────────────┴──────────────┴─────────────┘"
      );
    }

    this.logger.info("\n" + "=".repeat(120));
  }

  private setExpectedBalances(expectedRewards: RewardsResult): void {
    // Set expected protocol rewards (should drain to 0, so expected diff = -initial balance)
    const tokens = ["NTRN", "dATOM", "stATOM", "USDC"];
    for (const token of tokens) {
      const initialBalance = parseFloat(
        this.balanceSummary.protocol.before[token] || "0"
      );
      this.balanceSummary.protocol.expected[token] = (-initialBalance).toFixed(
        6
      );
    }

    // Set expected commission recipient rewards
    if (expectedRewards.protocol_rewards) {
      for (const [token, amount] of Object.entries(
        expectedRewards.protocol_rewards
      )) {
        this.balanceSummary.commissionRecipient.expected[token] = amount;
      }
    }

    // Set expected hydromancer rewards
    if (expectedRewards.hydromancer_rewards) {
      for (const [token, amount] of Object.entries(
        expectedRewards.hydromancer_rewards
      )) {
        this.balanceSummary.hydromancer.expected[token] = amount;
      }
    }

    // Set expected user rewards
    if (expectedRewards.user_rewards) {
      for (const [userId, userRewards] of Object.entries(
        expectedRewards.user_rewards
      )) {
        if (!this.balanceSummary.users[userId]) {
          this.balanceSummary.users[userId] = {
            before: {},
            after: {},
            difference: {},
            expected: {},
          };
        }
        for (const [token, amount] of Object.entries(userRewards)) {
          this.balanceSummary.users[userId].expected[token] = amount;
        }
      }
    }
  }
}
