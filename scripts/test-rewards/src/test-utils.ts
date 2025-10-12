import { ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
import { DeliverTxResponse } from "@cosmjs/stargate";
import { CONFIG } from "./config";

export class TestLogger {
  private startTime: number;

  constructor() {
    this.startTime = Date.now();
  }

  info(message: string): void {
    const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
    console.log(`[${elapsed}s] INFO: ${message}`);
  }

  error(message: string, error?: any): void {
    const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
    console.error(`[${elapsed}s] ERROR: ${message}`);
    if (error) {
      console.error(error);
    }
  }

  warn(message: string): void {
    const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
    console.warn(`[${elapsed}s] WARN: ${message}`);
  }

  debug(message: string): void {
    const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
    console.debug(`[${elapsed}s] DEBUG: ${message}`);
  }

  section(title: string): void {
    console.log(`\n${"=".repeat(50)}`);
    console.log(`${title}`);
    console.log(`${"=".repeat(50)}`);
  }
}

export interface ContractInfo {
  address: string;
  codeId: number;
}

export class ContractUtils {
  static extractContractAddress(result: InstantiateResult): string {
    return result.contractAddress;
  }

  static extractCodeId(result: ExecuteResult): number | null {
    // Extract code ID from store_code events
    for (const event of result.events) {
      if (event.type === "store_code") {
        for (const attr of event.attributes) {
          if (attr.key === "code_id") {
            return parseInt(attr.value);
          }
        }
      }
    }
    return null;
  }

  static extractAttributeFromEvents(
    result: ExecuteResult | DeliverTxResponse,
    eventType: string,
    attributeKey: string
  ): string | null {
    for (const event of result.events) {
      if (event.type === eventType) {
        for (const attr of event.attributes) {
          if (attr.key === attributeKey) {
            return attr.value;
          }
        }
      }
    }
    return null;
  }

  static wait(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  static async retry<T>(
    fn: () => Promise<T>,
    maxRetries: number = 3,
    delay: number = 1000
  ): Promise<T> {
    let lastError: Error;

    for (let i = 0; i <= maxRetries; i++) {
      try {
        return await fn();
      } catch (error) {
        lastError = error as Error;
        if (i === maxRetries) {
          break;
        }
        await this.wait(delay * Math.pow(2, i)); // Exponential backoff
      }
    }

    throw lastError!;
  }
}

export class TokenUtils {
  static parseAmount(amount: string): string {
    // Remove any decimal places and return as micro units
    const decimal = parseFloat(amount);
    return Math.floor(decimal * 1_000_000).toString();
  }

  static formatAmount(amount: string, decimals: number = 6): string {
    const decimal = parseInt(amount) / Math.pow(10, decimals);
    return decimal.toFixed(decimals);
  }

  static getTokenDenomFromSymbol(symbol: string): string {
    switch (symbol.toLowerCase()) {
      case "datom":
        return CONFIG.tokenDenoms.DATOM;
      case "statom":
        return CONFIG.tokenDenoms.STATOM;
      case "ntrn":
        return CONFIG.tokenDenoms.NTRN;
      case "usdc":
        return CONFIG.tokenDenoms.USDC;
      default:
        throw new Error(`Unknown token symbol: ${symbol}`);
    }
  }

  static getTokenSymbolFromDenom(denom: string): string {
    if (denom === CONFIG.tokenDenoms.DATOM) return "dATOM";
    if (denom === CONFIG.tokenDenoms.STATOM) return "stATOM";
    if (denom === CONFIG.tokenDenoms.NTRN) return "NTRN";
    if (denom === CONFIG.tokenDenoms.USDC) return "USDC";
    throw new Error(`Unknown token denom: ${denom}`);
  }
}

export interface TestReport {
  testName: string;
  scenario: any;
  expectedRewards: any;
  actualRewards: any;
  discrepancies: RewardDiscrepancy[];
  success: boolean;
  executionTime: number;
  transactionHashes: string[];
  gasUsed: string;
}

export interface RewardDiscrepancy {
  type: "protocol" | "hydromancer" | "user";
  entity?: string; // user ID if type is "user"
  token: string;
  expected: string;
  actual: string;
  difference: string;
  percentage?: number;
}

export class ReportGenerator {
  static generateTestReport(
    testName: string,
    scenario: any,
    expectedRewards: any,
    actualRewards: any,
    executionTime: number,
    transactionHashes: string[] = [],
    gasUsed: string = "0"
  ): TestReport {
    const discrepancies = this.findDiscrepancies(
      expectedRewards,
      actualRewards
    );
    const success = discrepancies.length === 0;

    return {
      testName,
      scenario,
      expectedRewards,
      actualRewards,
      discrepancies,
      success,
      executionTime,
      transactionHashes,
      gasUsed,
    };
  }

  private static findDiscrepancies(
    expected: any,
    actual: any
  ): RewardDiscrepancy[] {
    const discrepancies: RewardDiscrepancy[] = [];
    const tolerance = 0.01; // 1% tolerance for rounding differences

    // Check protocol rewards
    this.compareRewards(
      "protocol",
      expected.protocol_rewards,
      actual.protocol_rewards,
      discrepancies,
      tolerance
    );

    // Check hydromancer rewards
    this.compareRewards(
      "hydromancer",
      expected.hydromancer_rewards,
      actual.hydromancer_rewards,
      discrepancies,
      tolerance
    );

    // Check user rewards
    const expectedUsers = Object.keys(expected.user_rewards || {});
    const actualUsers = Object.keys(actual.user_rewards || {});
    const allUsers = new Set([...expectedUsers, ...actualUsers]);

    for (const userId of allUsers) {
      const expectedUserRewards = expected.user_rewards?.[userId] || {};
      const actualUserRewards = actual.user_rewards?.[userId] || {};
      this.compareRewards(
        "user",
        expectedUserRewards,
        actualUserRewards,
        discrepancies,
        tolerance,
        userId
      );
    }

    return discrepancies;
  }

  private static compareRewards(
    type: "protocol" | "hydromancer" | "user",
    expected: { [token: string]: string },
    actual: { [token: string]: string },
    discrepancies: RewardDiscrepancy[],
    tolerance: number,
    entity?: string
  ): void {
    const expectedTokens = Object.keys(expected || {});
    const actualTokens = Object.keys(actual || {});
    const allTokens = new Set([...expectedTokens, ...actualTokens]);

    for (const token of allTokens) {
      const expectedAmount = parseFloat(expected?.[token] || "0");
      const actualAmount = parseFloat(actual?.[token] || "0");

      const difference = Math.abs(expectedAmount - actualAmount);
      const percentage =
        expectedAmount > 0 ? (difference / expectedAmount) * 100 : 0;

      if (difference > tolerance && percentage > tolerance * 100) {
        discrepancies.push({
          type,
          entity,
          token,
          expected: expectedAmount.toFixed(2),
          actual: actualAmount.toFixed(2),
          difference: difference.toFixed(2),
          percentage: parseFloat(percentage.toFixed(2)),
        });
      }
    }
  }

  static printReport(report: TestReport): void {
    console.log(`\n${"=".repeat(80)}`);
    console.log(`TEST REPORT: ${report.testName}`);
    console.log(`${"=".repeat(80)}`);

    console.log(`Status: ${report.success ? "✅ PASSED" : "❌ FAILED"}`);
    console.log(`Execution Time: ${report.executionTime.toFixed(2)}s`);
    console.log(`Gas Used: ${report.gasUsed}`);
    console.log(`Transactions: ${report.transactionHashes.length}`);

    if (report.discrepancies.length > 0) {
      console.log(`\nDISCREPANCIES (${report.discrepancies.length}):`);
      console.log(`${"─".repeat(80)}`);

      for (const discrepancy of report.discrepancies) {
        const entity = discrepancy.entity ? ` (${discrepancy.entity})` : "";
        console.log(
          `${discrepancy.type.toUpperCase()}${entity} - ${discrepancy.token}:`
        );
        console.log(`  Expected: ${discrepancy.expected}`);
        console.log(`  Actual:   ${discrepancy.actual}`);
        console.log(
          `  Diff:     ${discrepancy.difference} (${discrepancy.percentage?.toFixed(2)}%)`
        );
        console.log();
      }
    } else {
      console.log(`\n✅ All rewards match expected values within tolerance`);
    }

    if (report.transactionHashes.length > 0) {
      console.log(`\nTRANSACTION HASHES:`);
      console.log(`${"─".repeat(40)}`);
      for (const hash of report.transactionHashes) {
        console.log(`  ${hash}`);
      }
    }

    console.log(`${"=".repeat(80)}\n`);
  }

  static saveReportToFile(report: TestReport, filename: string): void {
    try {
      const fs = require("fs");
      const path = require("path");

      // Ensure directory exists
      const dir = path.dirname(filename);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }

      // Write report to file
      fs.writeFileSync(filename, JSON.stringify(report, null, 2));
      console.log(`✅ Report saved to: ${filename}`);
    } catch (error) {
      console.error(`❌ Failed to save report to ${filename}:`, error);
    }
  }
}
