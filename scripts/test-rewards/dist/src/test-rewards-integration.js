#!/usr/bin/env node
"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.TestRewardsIntegration = void 0;
exports.runBatchTests = runBatchTests;
const fs_1 = __importDefault(require("fs"));
const path_1 = __importDefault(require("path"));
const wallet_manager_1 = require("./wallet-manager");
const test_utils_1 = require("./test-utils");
const calculate_rewards_1 = require("./calculate-rewards");
const environment_setup_1 = require("./environment-setup");
const scenario_executor_1 = require("./scenario-executor");
const rewards_validator_1 = require("./rewards-validator");
class TestRewardsIntegration {
    constructor() {
        this.logger = new test_utils_1.TestLogger();
        this.walletManager = new wallet_manager_1.WalletManager(this.logger);
        this.rewardsCalculator = new calculate_rewards_1.RewardsCalculator();
        this.environmentSetup = new environment_setup_1.EnvironmentSetup(this.logger, this.walletManager);
    }
    async runTest(scenarioFile) {
        const startTime = Date.now();
        let scenario;
        try {
            this.logger.section(`Zephyrus Rewards Integration Test`);
            this.logger.info(`Scenario file: ${scenarioFile}`);
            // Step 1: Load and validate scenario
            scenario = await this.loadScenario(scenarioFile);
            this.logger.info(`Loaded scenario with ${scenario.users.length} users and ${scenario.proposals.length} proposals`);
            // Step 2: Setup environment
            this.logger.info("Setting up test environment...");
            const setupResult = await this.environmentSetup.setupEnvironment();
            if (!setupResult.success) {
                throw new Error(`Environment setup failed: ${setupResult.error}`);
            }
            // Initialize executor and validator with contract addresses
            this.scenarioExecutor = new scenario_executor_1.ScenarioExecutor(this.logger, this.walletManager, setupResult.contractAddresses);
            this.rewardsValidator = new rewards_validator_1.RewardsValidator(this.logger, this.walletManager, setupResult.contractAddresses);
            // Step 3: Calculate expected rewards
            this.logger.info("Calculating expected rewards...");
            const expectedRewards = this.rewardsCalculator.calculateAllRewards(scenario);
            this.logger.info("Expected rewards calculation completed");
            // Step 4: Execute scenario on blockchain
            this.logger.info("Executing scenario on blockchain...");
            const executionResult = await this.scenarioExecutor.executeScenario(scenario);
            if (!executionResult.success) {
                throw new Error(`Scenario execution failed: ${executionResult.error}`);
            }
            // Step 5: Wait for rewards to be claimable
            this.logger.info("Waiting for rewards to be claimable...");
            await this.waitForRewardsClaimable();
            // Step 6: Validate rewards
            this.logger.info("Validating rewards...");
            const validationResult = await this.rewardsValidator.validateRewards(expectedRewards, executionResult);
            // Step 7: Claim rewards to verify amounts
            this.logger.info("Claiming rewards for verification...");
            await this.rewardsValidator.claimAllRewards(executionResult);
            // Step 8: Generate test report
            const executionTime = (Date.now() - startTime) / 1000;
            const testReport = test_utils_1.ReportGenerator.generateTestReport(`Scenario Test: ${path_1.default.basename(scenarioFile)}`, scenario, expectedRewards, validationResult.actualRewards, executionTime, executionResult.transactionHashes);
            // Step 9: Display results
            test_utils_1.ReportGenerator.printReport(testReport);
            // Step 10: Save detailed report
            const reportFileName = `test-report-${Date.now()}.json`;
            const reportPath = path_1.default.join(__dirname, "..", "reports", reportFileName);
            await this.ensureReportsDirectory();
            test_utils_1.ReportGenerator.saveReportToFile(testReport, reportPath);
            return testReport;
        }
        catch (error) {
            this.logger.error("Test execution failed", error);
            const executionTime = (Date.now() - startTime) / 1000;
            const errorReport = test_utils_1.ReportGenerator.generateTestReport(`FAILED: ${path_1.default.basename(scenarioFile)}`, {}, // Empty scenario on error
            {}, {}, executionTime, []);
            errorReport.success = false;
            errorReport.discrepancies.push({
                type: "protocol",
                token: "ERROR",
                expected: "SUCCESS",
                actual: error instanceof Error ? error.message : String(error),
                difference: "FAILED"
            });
            test_utils_1.ReportGenerator.printReport(errorReport);
            return errorReport;
        }
        finally {
            await this.cleanup();
        }
    }
    async loadScenario(scenarioFile) {
        try {
            if (!fs_1.default.existsSync(scenarioFile)) {
                throw new Error(`Scenario file not found: ${scenarioFile}`);
            }
            const scenarioContent = fs_1.default.readFileSync(scenarioFile, "utf8");
            const scenario = JSON.parse(scenarioContent);
            // Validate scenario structure
            this.validateScenario(scenario);
            return scenario;
        }
        catch (error) {
            throw new Error(`Failed to load scenario: ${error}`);
        }
    }
    validateScenario(scenario) {
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
            if (!proposal.id || !proposal.tributes || !Array.isArray(proposal.tributes)) {
                throw new Error(`Invalid proposal structure: ${JSON.stringify(proposal)}`);
            }
        }
    }
    async waitForRewardsClaimable() {
        // In a real test environment, this would wait for:
        // 1. Round progression
        // 2. Liquidity deployment
        // 3. Rewards to become claimable
        this.logger.info("Simulating wait for rewards to become claimable...");
        // For testing purposes, simulate a brief wait
        await new Promise(resolve => setTimeout(resolve, 5000)); // 5 seconds
        this.logger.info("Rewards should now be claimable");
    }
    async ensureReportsDirectory() {
        const reportsDir = path_1.default.join(__dirname, "..", "reports");
        if (!fs_1.default.existsSync(reportsDir)) {
            fs_1.default.mkdirSync(reportsDir, { recursive: true });
        }
    }
    async cleanup() {
        this.logger.info("Cleaning up test resources...");
        try {
            await this.environmentSetup.cleanup();
        }
        catch (error) {
            this.logger.error("Cleanup failed", error);
        }
        this.logger.info("Test cleanup completed");
    }
}
exports.TestRewardsIntegration = TestRewardsIntegration;
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
    const fullScenarioPath = path_1.default.isAbsolute(scenarioFile)
        ? scenarioFile
        : path_1.default.join(process.cwd(), scenarioFile);
    const testRunner = new TestRewardsIntegration();
    try {
        const report = await testRunner.runTest(fullScenarioPath);
        // Exit with error code if test failed
        if (!report.success) {
            process.exit(1);
        }
        console.log("✅ Test completed successfully!");
        process.exit(0);
    }
    catch (error) {
        console.error("❌ Test runner failed:", error);
        process.exit(1);
    }
}
// Batch Testing Function
async function runBatchTests(scenarioFiles) {
    const testRunner = new TestRewardsIntegration();
    const reports = [];
    console.log(`Running batch tests on ${scenarioFiles.length} scenarios...`);
    for (const scenarioFile of scenarioFiles) {
        console.log(`\n${"=".repeat(80)}`);
        console.log(`Running test: ${scenarioFile}`);
        console.log(`${"=".repeat(80)}`);
        try {
            const report = await testRunner.runTest(scenarioFile);
            reports.push(report);
        }
        catch (error) {
            console.error(`Test failed for ${scenarioFile}:`, error);
            // Continue with next test
        }
    }
    // Generate batch summary
    console.log(`\n${"=".repeat(80)}`);
    console.log("BATCH TEST SUMMARY");
    console.log(`${"=".repeat(80)}`);
    const successful = reports.filter(r => r.success).length;
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
    main().catch(error => {
        console.error("Unhandled error:", error);
        process.exit(1);
    });
}
//# sourceMappingURL=test-rewards-integration.js.map