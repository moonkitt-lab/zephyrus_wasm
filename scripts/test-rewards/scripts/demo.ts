#!/usr/bin/env node

import { TestRewardsIntegration } from "../src/test-rewards-integration";
import path from "path";

/**
 * Demo script showing how to use the Zephyrus Rewards Testing Framework
 */

async function runDemo() {
  console.log("🎯 Zephyrus Rewards Testing Framework Demo");
  console.log("=" .repeat(50));

  console.log("\n📋 This demo will:");
  console.log("  1. Load a test scenario from JSON");
  console.log("  2. Set up the blockchain environment");
  console.log("  3. Execute the scenario on-chain");
  console.log("  4. Calculate expected rewards");
  console.log("  5. Validate actual vs expected rewards");
  console.log("  6. Generate a detailed report");

  // Check if scenario files exist
  const scenarioFile1 = path.join(__dirname, "..", "rewards-scenario-1.json");
  const scenarioFile2 = path.join(__dirname, "..", "rewards-scenario-2.json");

  let demoScenario = scenarioFile1;

  // Try to find an existing scenario file
  const fs = require("fs");
  if (!fs.existsSync(scenarioFile1) && !fs.existsSync(scenarioFile2)) {
    console.log("\n❌ No scenario files found!");
    console.log("💡 Please generate scenario files first:");
    console.log("   cd test-rewards/");
    console.log("   python generate-json.py");
    process.exit(1);
  }

  if (!fs.existsSync(scenarioFile1) && fs.existsSync(scenarioFile2)) {
    demoScenario = scenarioFile2;
  }

  console.log(`\n🔄 Running demo with scenario: ${path.basename(demoScenario)}`);
  console.log("-".repeat(50));

  try {
    const testRunner = new TestRewardsIntegration();
    const report = await testRunner.runTest(demoScenario);

    console.log("\n🎉 Demo completed!");
    console.log(`Result: ${report.success ? "✅ PASSED" : "❌ FAILED"}`);
    
    if (report.success) {
      console.log("🎊 Congratulations! The rewards system is working correctly.");
      console.log("💰 All expected rewards matched actual rewards within tolerance.");
    } else {
      console.log("🔍 Some discrepancies were found. Check the detailed report above.");
      console.log("🐛 This might indicate issues with the rewards calculation logic.");
    }

    console.log(`\n📊 Test Statistics:`);
    console.log(`   Execution Time: ${report.executionTime.toFixed(2)}s`);
    console.log(`   Transactions: ${report.transactionHashes.length}`);
    console.log(`   Discrepancies: ${report.discrepancies.length}`);

  } catch (error) {
    console.error("\n❌ Demo failed:", error);
    console.log("\n🔧 Troubleshooting tips:");
    console.log("   1. Ensure Neutron devnet is running on localhost:26657");
    console.log("   2. Check that Hydro and Zephyrus contracts are deployed");
    console.log("   3. Verify the deployer wallet has sufficient funds");
    console.log("   4. Make sure contract addresses are correctly configured");
    process.exit(1);
  }
}

if (require.main === module) {
  runDemo().catch(error => {
    console.error("Demo failed:", error);
    process.exit(1);
  });
}

export { runDemo };