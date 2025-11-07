#!/usr/bin/env node

import fs from "fs";
import path from "path";
import { WalletManager } from "./wallet-manager";
import {
  TestLogger,
  ReportGenerator,
  TestReport,
  TokenUtils,
} from "./test-utils";
import { RewardsCalculator, Scenario } from "./calculate-rewards";
import { getTokenDenom } from "./config";
import { EnvironmentSetup } from "./environment-setup";
import { ScenarioExecutor } from "./scenario-executor";
import { RewardsValidator } from "./rewards-validator";

class TestRewardsIntegration {
  private logger: TestLogger;
  private walletManager: WalletManager;
  private rewardsCalculator: RewardsCalculator;
  private environmentSetup?: EnvironmentSetup;
  private scenarioExecutor?: ScenarioExecutor;
  private rewardsValidator?: RewardsValidator;

  constructor() {
    this.logger = new TestLogger();
    this.walletManager = new WalletManager(this.logger);
    this.rewardsCalculator = new RewardsCalculator();
  }

  async runTest(scenarioFile: string): Promise<TestReport> {
    const startTime = Date.now();
    let scenario: Scenario;

    try {
      this.logger.section(`Zephyrus Rewards Integration Test`);
      this.logger.info(`Scenario file: ${scenarioFile}`);

      // Step 1: Load and validate scenario
      scenario = await this.loadScenario(scenarioFile);
      this.logger.info(
        `Loaded scenario with ${scenario.users.length} users and ${scenario.proposals.length} proposals`
      );

      // Step 1.5: Create EnvironmentSetup with full scenario
      this.environmentSetup = new EnvironmentSetup(
        this.logger,
        this.walletManager,
        scenario
      );

      // Step 2: Setup environment
      this.logger.info("Setting up test environment...");
      const setupResult = await this.environmentSetup.setupEnvironment();
      if (!setupResult.success) {
        throw new Error(`Environment setup failed: ${setupResult.error}`);
      }
      this.rewardsValidator = new RewardsValidator(
        this.logger,
        this.walletManager,
        setupResult.contractAddresses,
        setupResult.commissionRecipientAddress
      );
      // Initialize executor and validator with contract addresses
      this.scenarioExecutor = new ScenarioExecutor(
        this.logger,
        this.walletManager,
        setupResult.contractAddresses,
        setupResult.commissionRecipientAddress,
        scenario.protocol_config.round_length,
        this.rewardsValidator
      );

      // Step 3: Calculate expected rewards
      this.logger.info("Calculating expected rewards...");
      const expectedRewards =
        this.calculateExpectedRewardsForAllRounds(scenario);
      this.logger.info("Expected rewards calculation completed");

      // Step 4: Capture initial balances BEFORE scenario execution
      this.logger.info(
        "Capturing initial balances before scenario execution..."
      );
      const initialBalances = await this.captureInitialBalances(scenario);
      this.logger.info("Initial balances captured successfully");

      // Step 5: Execute scenario on blockchain
      this.logger.info(
        "🎯 TEST MAIN: Step 5 - About to execute scenario on blockchain..."
      );
      const executionResult = await this.scenarioExecutor.executeScenario(
        scenario,
        expectedRewards,
        initialBalances
      );
      this.logger.info("🎯 TEST MAIN: Step 5 - Scenario execution completed");
      if (!executionResult.success) {
        this.logger.error("🎯 TEST MAIN: Step 5 - Scenario execution failed!");
        throw new Error(`Scenario execution failed: ${executionResult.error}`);
      }
      this.logger.info("🎯 TEST MAIN: Step 5 - Scenario execution successful!");

      // Step 6: Rewards are already claimable after liquidity deployment
      this.logger.info(
        "Rewards are already claimable after liquidity deployment"
      );

      // // Step 6: Validate rewards
      // this.logger.info("Validating rewards...");
      // const validationResult = await this.rewardsValidator.validateRewards(
      //   expectedRewards,
      //   executionResult,
      //   scenario.protocol_config.total_rounds
      // );

      // // Step 7.5: Make additional claim for first user to test duplicate claim behavior
      // this.logger.info("Making additional claim for first user...");
      // await this.rewardsValidator.claimFirstUserAgain(executionResult);

      // Step 7: Calculate actual rewards from claims (using RewardsValidator)
      this.logger.info("Calculating actual rewards from claims...");
      await this.rewardsValidator.initializeClients();
      const actualRewards =
        await this.rewardsValidator.calculateActualRewardsFromClaims(
          initialBalances
        );
      this.logger.info("Actual rewards calculation completed");

      // Step 9: Generate test report
      const executionTime = (Date.now() - startTime) / 1000;
      this.logger.info(
        `🎯 GENERATE TEST REPORT: Expected rewards: ${JSON.stringify(expectedRewards)}`
      );
      this.logger.info(
        `🎯 GENERATE TEST REPORT: Actual rewards: ${JSON.stringify(actualRewards)}`
      );
      const testReport = ReportGenerator.generateTestReport(
        `Scenario Test: ${path.basename(scenarioFile)}`,
        scenario,
        expectedRewards,
        actualRewards,
        executionTime,
        executionResult.transactionHashes
      );

      // Step 9: Display results
      ReportGenerator.printReport(testReport);

      // Step 10: Save detailed report
      const reportFileName = `test-report-${Date.now()}.json`;
      const reportPath = path.join(__dirname, "..", "reports", reportFileName);
      await this.ensureReportsDirectory();
      ReportGenerator.saveReportToFile(testReport, reportPath);

      return testReport;
    } catch (error) {
      this.logger.error("Test execution failed", error);

      const executionTime = (Date.now() - startTime) / 1000;
      const errorReport = ReportGenerator.generateTestReport(
        `FAILED: ${path.basename(scenarioFile)}`,
        {}, // Empty scenario on error
        {},
        {},
        executionTime,
        []
      );

      errorReport.success = false;
      errorReport.discrepancies.push({
        type: "protocol",
        token: "ERROR",
        expected: "SUCCESS",
        actual: error instanceof Error ? error.message : String(error),
        difference: "FAILED",
      });

      ReportGenerator.printReport(errorReport);
      return errorReport;
    } finally {
      await this.cleanup();
    }
  }

  private async loadScenario(scenarioFile: string): Promise<Scenario> {
    try {
      if (!fs.existsSync(scenarioFile)) {
        throw new Error(`Scenario file not found: ${scenarioFile}`);
      }

      const scenarioContent = fs.readFileSync(scenarioFile, "utf8");
      const scenario: Scenario = JSON.parse(scenarioContent);

      // Validate scenario structure
      this.validateScenario(scenario);

      return scenario;
    } catch (error) {
      throw new Error(`Failed to load scenario: ${error}`);
    }
  }

  private validateScenario(scenario: Scenario): void {
    if (!scenario.protocol_config) {
      throw new Error("Scenario missing protocol_config");
    }

    if (!scenario.users || !Array.isArray(scenario.users)) {
      throw new Error("Scenario missing users array");
    }

    if (!scenario.proposals || !Array.isArray(scenario.proposals)) {
      throw new Error("Scenario missing proposals array");
    }

    // Validate each user has vessels
    for (const user of scenario.users) {
      if (!user.user_id || !user.vessels || !Array.isArray(user.vessels)) {
        throw new Error(`Invalid user structure: ${JSON.stringify(user)}`);
      }
    }

    // Validate each proposal has tributes
    for (const proposal of scenario.proposals) {
      if (
        proposal.id === undefined ||
        proposal.id === null ||
        !proposal.tributes ||
        !Array.isArray(proposal.tributes)
      ) {
        throw new Error(
          `Invalid proposal structure: ${JSON.stringify(proposal)}`
        );
      }
    }
  }

  private async ensureReportsDirectory(): Promise<void> {
    const reportsDir = path.join(__dirname, "..", "reports");
    if (!fs.existsSync(reportsDir)) {
      fs.mkdirSync(reportsDir, { recursive: true });
    }
  }

  private async cleanup(): Promise<void> {
    this.logger.info("Cleaning up test resources...");

    try {
      if (this.environmentSetup) {
        await this.environmentSetup.cleanup();
      }
    } catch (error) {
      this.logger.error("Cleanup failed", error);
    }

    this.logger.info("Test cleanup completed");
  }

  private calculateExpectedRewardsForAllRounds(scenario: Scenario): any {
    const rewardsByRound: { [roundId: number]: any } = {};

    // Calculate actual rounds defined in scenario
    const totalRounds = this.calculateActualRounds(scenario);

    // Calculate rewards for each round
    for (let roundId = 0; roundId < totalRounds; roundId++) {
      const roundRewards = this.rewardsCalculator.calculateAllRewardsForRound(
        scenario,
        roundId,
        this.logger
      );
      rewardsByRound[roundId] = roundRewards;
    }

    // Aggregate rewards from all rounds (same logic as ScenarioExecutor)
    const aggregatedRewards: any = {
      protocol_rewards: {},
      hydromancer_rewards: {},
      user_rewards: {},
    };

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

  private getTokenSymbolFromDenom(denom: string): string {
    // Map full denoms to short symbols for consistency with expectedRewards
    if (denom === "untrn") return "NTRN";
    if (
      denom ===
      "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom"
    )
      return "dATOM";
    if (
      denom ===
      "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C"
    )
      return "stATOM";
    if (
      denom ===
      "ibc/B559A80D62249C8AA07A380E2A2BEA6E5CA9A6F079C912C3A9E9B494105E4F81"
    )
      return "USDC";
    return denom; // Return as-is if no mapping found
  }

  private async captureInitialBalances(
    scenario: Scenario
  ): Promise<{ [userId: string]: { [denom: string]: string } }> {
    const initialBalances: { [userId: string]: { [denom: string]: string } } =
      {};

    // Define common denoms
    const commonDenoms = [
      "untrn",
      "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
      "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
      "ibc/B559A80D62249C8AA07A380E2A2BEA6E5CA9A6F079C912C3A9E9B494105E4F81",
    ];

    // Capture balances for scenario users
    for (const user of scenario.users) {
      const userId = user.user_id;
      initialBalances[userId] = {};

      try {
        const wallet = this.walletManager.getTestWallet(userId);
        if (wallet) {
          for (const denom of commonDenoms) {
            try {
              const balance = await wallet.client.getBalance(
                wallet.address,
                denom
              );
              initialBalances[userId][denom] = balance.amount;
            } catch (error) {
              // Token not found, assume 0 balance
              initialBalances[userId][denom] = "0";
            }
          }
        }
      } catch (error) {
        this.logger.warn(
          `Failed to get initial balances for user ${userId}: ${error}`
        );
      }
    }

    // Subtract locked amounts in vessels from initial balances
    this.logger.info(
      "🔍 DEBUG: Subtracting locked vessel amounts from initial balances..."
    );
    for (const user of scenario.users) {
      const userId = user.user_id;

      // Calculate total locked amounts for this user's vessels
      let totalLockedAmounts: { [denom: string]: string } = {};

      for (const vessel of user.vessels) {
        // Get the locked amount for this vessel
        const lockedAmount = vessel.locked_amount || "0";
        const denom = vessel.locked_denom;

        if (lockedAmount !== "0" && denom) {
          // Map readable denom to technical denom
          let technicalDenom = denom;
          if (denom === "stATOM") {
            technicalDenom =
              "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C";
          } else if (denom === "dATOM") {
            technicalDenom =
              "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom";
          } else if (denom === "NTRN") {
            technicalDenom = "untrn";
          }

          const currentLocked = totalLockedAmounts[technicalDenom] || "0";
          // Convert locked amount to micro-units (multiply by 1,000,000)
          const lockedAmountMicro = (
            parseFloat(lockedAmount) * 1000000
          ).toString();
          totalLockedAmounts[technicalDenom] = (
            parseFloat(currentLocked) + parseFloat(lockedAmountMicro)
          ).toString();

          this.logger.info(
            `🔍 DEBUG: Vessel ${vessel.id} locked ${lockedAmount} ${denom} (${technicalDenom})`
          );
        }
      }

      // Subtract locked amounts from initial balances
      for (const [denom, lockedAmount] of Object.entries(totalLockedAmounts)) {
        if (initialBalances[userId][denom]) {
          const currentBalance = parseFloat(initialBalances[userId][denom]);
          const locked = parseFloat(lockedAmount);
          const adjustedBalance = Math.max(0, currentBalance - locked);
          initialBalances[userId][denom] = adjustedBalance.toString();

          this.logger.info(
            `🔍 DEBUG: User ${userId} ${denom}: ${currentBalance} - ${locked} (locked) = ${adjustedBalance}`
          );
        }
      }
    }

    // Capture balances for hydromancer
    try {
      const hydromancerWallet = this.walletManager.getTestWallet("hydromancer");
      if (hydromancerWallet) {
        initialBalances["hydromancer"] = {};
        for (const denom of commonDenoms) {
          try {
            const balance = await hydromancerWallet.client.getBalance(
              hydromancerWallet.address,
              denom
            );
            initialBalances["hydromancer"][denom] = balance.amount;
          } catch (error) {
            initialBalances["hydromancer"][denom] = "0";
          }
        }
      }
    } catch (error) {
      this.logger.warn(
        `Failed to get initial balances for hydromancer: ${error}`
      );
    }

    // Capture balances for commission recipient
    try {
      const commissionWallet = this.walletManager.getTestWallet(
        "commissionRecipient"
      );
      if (commissionWallet) {
        initialBalances["commissionRecipient"] = {};
        for (const denom of commonDenoms) {
          try {
            const balance = await commissionWallet.client.getBalance(
              commissionWallet.address,
              denom
            );
            initialBalances["commissionRecipient"][denom] = balance.amount;
          } catch (error) {
            initialBalances["commissionRecipient"][denom] = "0";
          }
        }
      }
    } catch (error) {
      this.logger.warn(
        `Failed to get initial balances for commission recipient: ${error}`
      );
    }
    this.logger.info(`Initial balances  ${JSON.stringify(initialBalances)}`);

    return initialBalances;
  }

  private async calculateActualRewardsFromBalances(
    scenario: Scenario,
    executionResult: any,
    initialBalances: { [userId: string]: { [denom: string]: string } }
  ): Promise<any> {
    try {
      this.logger.info(
        "🔍 BALANCE CHECK: Starting actual rewards calculation from real balances..."
      );

      const actualRewards = {
        protocol_rewards: {},
        hydromancer_rewards: {},
        user_rewards: {},
      };

      // Get final balances (after scenario execution)
      const finalBalances: { [userId: string]: { [denom: string]: string } } =
        {};

      // Calculate balance differences for each user
      for (const user of scenario.users) {
        const userId = user.user_id;
        finalBalances[userId] = {};

        // Get final balances from wallet
        try {
          const wallet = this.walletManager.getTestWallet(userId);
          if (wallet) {
            // Get balances for common tokens (same as rewards-validator.ts)
            const commonDenoms = [
              "untrn",
              "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
              "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
              "ibc/B559A80D62249C8AA07A380E2A2BEA6E5CA9A6F079C912C3A9E9B494105E4F81",
            ];

            for (const denom of commonDenoms) {
              try {
                const balance = await wallet.client.getBalance(
                  wallet.address,
                  denom
                );
                finalBalances[userId][denom] = balance.amount;
              } catch (error) {
                // Token not found, continue
                finalBalances[userId][denom] = "0";
              }
            }
          }
        } catch (error) {
          this.logger.warn(
            `Failed to get final balances for user ${userId}: ${error}`
          );
        }

        // Calculate rewards as the difference between final and initial balances
        (actualRewards.user_rewards as any)[userId] = {};

        // Calculate rewards as final balance minus initial balance
        for (const denom of Object.keys(finalBalances[userId])) {
          const finalAmount = parseFloat(finalBalances[userId][denom] || "0");
          const initialAmount = parseFloat(
            initialBalances[userId]?.[denom] || "0"
          );
          const reward = finalAmount - initialAmount;

          if (reward > 0) {
            // Convert from base units to human-readable units
            const rewardInHumanUnits = TokenUtils.formatAmount(
              reward.toString(),
              6
            );

            // Map full denom to short symbol for consistency with expectedRewards
            const symbol = this.getTokenSymbolFromDenom(denom);
            (actualRewards.user_rewards as any)[userId][symbol] =
              rewardInHumanUnits;
          }
        }

        this.logger.info(
          `🔍 BALANCE CHECK: User ${userId} rewards calculated: ${JSON.stringify((actualRewards.user_rewards as any)[userId])}`
        );
      }

      // Calculate hydromancer rewards
      try {
        const hydromancerWallet =
          this.walletManager.getTestWallet("hydromancer");
        if (hydromancerWallet) {
          const hydromancerBalances: { [denom: string]: string } = {};

          // Get hydromancer balances for common tokens
          const commonDenoms = [
            "untrn",
            "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
            "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
            "ibc/B559A80D62249C8AA07A380E2A2BEA6E5CA9A6F079C912C3A9E9B494105E4F81",
          ];

          for (const denom of commonDenoms) {
            try {
              const balance = await hydromancerWallet.client.getBalance(
                hydromancerWallet.address,
                denom
              );
              hydromancerBalances[denom] = balance.amount;
            } catch (error) {
              hydromancerBalances[denom] = "0";
            }
          }

          // Calculate hydromancer rewards (final balance minus initial balance)
          for (const denom of Object.keys(hydromancerBalances)) {
            const finalAmount = parseFloat(hydromancerBalances[denom] || "0");
            const initialAmount = parseFloat(
              initialBalances["hydromancer"]?.[denom] || "0"
            );
            const reward = finalAmount - initialAmount;

            if (reward > 0) {
              const symbol = this.getTokenSymbolFromDenom(denom);
              const rewardInHumanUnits = TokenUtils.formatAmount(
                reward.toString(),
                6
              );
              (actualRewards.hydromancer_rewards as any)[symbol] =
                rewardInHumanUnits;
            }
          }

          this.logger.info(
            `🔍 BALANCE CHECK: Hydromancer rewards calculated: ${JSON.stringify(actualRewards.hydromancer_rewards)}`
          );
        }
      } catch (error) {
        this.logger.warn(`Failed to calculate hydromancer rewards: ${error}`);
      }

      // Calculate protocol rewards (commission recipient)
      try {
        const commissionWallet = this.walletManager.getTestWallet(
          "commissionRecipient"
        );
        if (commissionWallet) {
          const protocolBalances: { [denom: string]: string } = {};

          // Get protocol balances for common tokens
          const commonDenoms = [
            "untrn",
            "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
            "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
            "ibc/B559A80D62249C8AA07A380E2A2BEA6E5CA9A6F079C912C3A9E9B494105E4F81",
          ];

          for (const denom of commonDenoms) {
            try {
              const balance = await commissionWallet.client.getBalance(
                commissionWallet.address,
                denom
              );
              protocolBalances[denom] = balance.amount;
            } catch (error) {
              protocolBalances[denom] = "0";
            }
          }

          // Calculate protocol rewards (final balance minus initial balance)
          for (const denom of Object.keys(protocolBalances)) {
            const finalAmount = parseFloat(protocolBalances[denom] || "0");
            const initialAmount = parseFloat(
              initialBalances["commissionRecipient"]?.[denom] || "0"
            );
            const reward = finalAmount - initialAmount;

            if (reward > 0) {
              const symbol = this.getTokenSymbolFromDenom(denom);
              const rewardInHumanUnits = TokenUtils.formatAmount(
                reward.toString(),
                6
              );
              (actualRewards.protocol_rewards as any)[symbol] =
                rewardInHumanUnits;
            }
          }

          this.logger.info(
            `🔍 BALANCE CHECK: Protocol rewards calculated: ${JSON.stringify(actualRewards.protocol_rewards)}`
          );
        }
      } catch (error) {
        this.logger.warn(`Failed to calculate protocol rewards: ${error}`);
      }

      this.logger.info(
        "🔍 BALANCE CHECK: Actual rewards calculation completed"
      );
      return actualRewards;
    } catch (error) {
      this.logger.error(
        "🔍 BALANCE CHECK: Error calculating actual rewards from balances:",
        error
      );
      // Return empty rewards on error
      return {
        protocol_rewards: {},
        hydromancer_rewards: {},
        user_rewards: {},
      };
    }
  }
}

// CLI Interface
async function main() {
  const args = process.argv.slice(2);

  if (args.length === 0) {
    console.log("Usage: npm run test:scenario <scenario-file>");
    console.log("Example: npm run test:scenario rewards-scenario-1.json");
    process.exit(1);
  }

  const scenarioFile = args[0];

  // Resolve relative paths
  const fullScenarioPath = path.isAbsolute(scenarioFile)
    ? scenarioFile
    : path.join(process.cwd(), scenarioFile);

  const testRunner = new TestRewardsIntegration();

  try {
    const report = await testRunner.runTest(fullScenarioPath);

    // Exit with error code if test failed
    if (!report.success) {
      process.exit(1);
    }

    console.log("✅ Test completed successfully!");
    process.exit(0);
  } catch (error) {
    console.error("❌ Test runner failed:", error);
    process.exit(1);
  }
}

// Batch Testing Function
export async function runBatchTests(
  scenarioFiles: string[]
): Promise<TestReport[]> {
  const testRunner = new TestRewardsIntegration();
  const reports: TestReport[] = [];

  console.log(`Running batch tests on ${scenarioFiles.length} scenarios...`);

  for (const scenarioFile of scenarioFiles) {
    console.log(`\n${"=".repeat(80)}`);
    console.log(`Running test: ${scenarioFile}`);
    console.log(`${"=".repeat(80)}`);

    try {
      const report = await testRunner.runTest(scenarioFile);
      reports.push(report);
    } catch (error) {
      console.error(`Test failed for ${scenarioFile}:`, error);
      // Continue with next test
    }
  }

  // Generate batch summary
  console.log(`\n${"=".repeat(80)}`);
  console.log("BATCH TEST SUMMARY");
  console.log(`${"=".repeat(80)}`);

  const successful = reports.filter((r) => r.success).length;
  const failed = reports.length - successful;

  console.log(`Total tests: ${reports.length}`);
  console.log(`Successful: ${successful}`);
  console.log(`Failed: ${failed}`);

  if (failed > 0) {
    console.log("\nFailed tests:");
    for (const report of reports) {
      if (!report.success) {
        console.log(`  - ${report.testName}`);
      }
    }
  }

  return reports;
}

// Run main function if this script is executed directly
if (require.main === module) {
  main().catch((error) => {
    console.error("Unhandled error:", error);
    process.exit(1);
  });
}

export { TestRewardsIntegration };
