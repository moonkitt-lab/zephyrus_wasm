"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.ReportGenerator = exports.TokenUtils = exports.ContractUtils = exports.TestLogger = void 0;
const config_1 = require("./config");
class TestLogger {
    constructor() {
        this.startTime = Date.now();
    }
    info(message) {
        const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
        console.log(`[${elapsed}s] INFO: ${message}`);
    }
    error(message, error) {
        const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
        console.error(`[${elapsed}s] ERROR: ${message}`);
        if (error) {
            console.error(error);
        }
    }
    warn(message) {
        const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
        console.warn(`[${elapsed}s] WARN: ${message}`);
    }
    debug(message) {
        const elapsed = ((Date.now() - this.startTime) / 1000).toFixed(1);
        console.debug(`[${elapsed}s] DEBUG: ${message}`);
    }
    section(title) {
        console.log(`\n${"=".repeat(50)}`);
        console.log(`${title}`);
        console.log(`${"=".repeat(50)}`);
    }
}
exports.TestLogger = TestLogger;
class ContractUtils {
    static extractContractAddress(result) {
        return result.contractAddress;
    }
    static extractCodeId(result) {
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
    static extractAttributeFromEvents(result, eventType, attributeKey) {
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
    static wait(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
    static async retry(fn, maxRetries = 3, delay = 1000) {
        let lastError;
        for (let i = 0; i <= maxRetries; i++) {
            try {
                return await fn();
            }
            catch (error) {
                lastError = error;
                if (i === maxRetries) {
                    break;
                }
                await this.wait(delay * Math.pow(2, i)); // Exponential backoff
            }
        }
        throw lastError;
    }
}
exports.ContractUtils = ContractUtils;
class TokenUtils {
    static parseAmount(amount) {
        // Remove any decimal places and return as micro units
        const decimal = parseFloat(amount);
        return Math.floor(decimal * 1000000).toString();
    }
    static formatAmount(amount, decimals = 6) {
        const decimal = parseInt(amount) / Math.pow(10, decimals);
        return decimal.toFixed(decimals);
    }
    static getTokenDenomFromSymbol(symbol) {
        switch (symbol.toLowerCase()) {
            case "datom":
                return config_1.CONFIG.tokenDenoms.DATOM;
            case "statom":
                return config_1.CONFIG.tokenDenoms.STATOM;
            case "ntrn":
                return config_1.CONFIG.tokenDenoms.NTRN;
            case "usdc":
                return "uusdc"; // Placeholder
            default:
                throw new Error(`Unknown token symbol: ${symbol}`);
        }
    }
    static getTokenSymbolFromDenom(denom) {
        if (denom === config_1.CONFIG.tokenDenoms.DATOM)
            return "dATOM";
        if (denom === config_1.CONFIG.tokenDenoms.STATOM)
            return "stATOM";
        if (denom === config_1.CONFIG.tokenDenoms.NTRN)
            return "NTRN";
        if (denom === "uusdc")
            return "USDC";
        throw new Error(`Unknown token denom: ${denom}`);
    }
}
exports.TokenUtils = TokenUtils;
class ReportGenerator {
    static generateTestReport(testName, scenario, expectedRewards, actualRewards, executionTime, transactionHashes = [], gasUsed = "0") {
        const discrepancies = this.findDiscrepancies(expectedRewards, actualRewards);
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
            gasUsed
        };
    }
    static findDiscrepancies(expected, actual) {
        const discrepancies = [];
        const tolerance = 0.01; // 1% tolerance for rounding differences
        // Check protocol rewards
        this.compareRewards("protocol", expected.protocol_rewards, actual.protocol_rewards, discrepancies, tolerance);
        // Check hydromancer rewards  
        this.compareRewards("hydromancer", expected.hydromancer_rewards, actual.hydromancer_rewards, discrepancies, tolerance);
        // Check user rewards
        const expectedUsers = Object.keys(expected.user_rewards || {});
        const actualUsers = Object.keys(actual.user_rewards || {});
        const allUsers = new Set([...expectedUsers, ...actualUsers]);
        for (const userId of allUsers) {
            const expectedUserRewards = expected.user_rewards?.[userId] || {};
            const actualUserRewards = actual.user_rewards?.[userId] || {};
            this.compareRewards("user", expectedUserRewards, actualUserRewards, discrepancies, tolerance, userId);
        }
        return discrepancies;
    }
    static compareRewards(type, expected, actual, discrepancies, tolerance, entity) {
        const expectedTokens = Object.keys(expected || {});
        const actualTokens = Object.keys(actual || {});
        const allTokens = new Set([...expectedTokens, ...actualTokens]);
        for (const token of allTokens) {
            const expectedAmount = parseFloat(expected?.[token] || "0");
            const actualAmount = parseFloat(actual?.[token] || "0");
            const difference = Math.abs(expectedAmount - actualAmount);
            const percentage = expectedAmount > 0 ? (difference / expectedAmount) * 100 : 0;
            if (difference > tolerance && percentage > tolerance * 100) {
                discrepancies.push({
                    type,
                    entity,
                    token,
                    expected: expectedAmount.toFixed(2),
                    actual: actualAmount.toFixed(2),
                    difference: difference.toFixed(2),
                    percentage: parseFloat(percentage.toFixed(2))
                });
            }
        }
    }
    static printReport(report) {
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
                console.log(`${discrepancy.type.toUpperCase()}${entity} - ${discrepancy.token}:`);
                console.log(`  Expected: ${discrepancy.expected}`);
                console.log(`  Actual:   ${discrepancy.actual}`);
                console.log(`  Diff:     ${discrepancy.difference} (${discrepancy.percentage?.toFixed(2)}%)`);
                console.log();
            }
        }
        else {
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
    static saveReportToFile(report, filename) {
        // In a real implementation, save to file system
        // For now, just log the JSON representation
        console.log(`Saving report to ${filename}:`);
        console.log(JSON.stringify(report, null, 2));
    }
}
exports.ReportGenerator = ReportGenerator;
//# sourceMappingURL=test-utils.js.map