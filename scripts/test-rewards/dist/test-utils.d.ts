import { ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
import { DeliverTxResponse } from "@cosmjs/stargate";
export declare class TestLogger {
    private startTime;
    constructor();
    info(message: string): void;
    error(message: string, error?: any): void;
    warn(message: string): void;
    debug(message: string): void;
    section(title: string): void;
}
export interface ContractInfo {
    address: string;
    codeId: number;
}
export declare class ContractUtils {
    static extractContractAddress(result: InstantiateResult): string;
    static extractCodeId(result: ExecuteResult): number | null;
    static extractAttributeFromEvents(result: ExecuteResult | DeliverTxResponse, eventType: string, attributeKey: string): string | null;
    static wait(ms: number): Promise<void>;
    static retry<T>(fn: () => Promise<T>, maxRetries?: number, delay?: number): Promise<T>;
}
export declare class TokenUtils {
    static parseAmount(amount: string): string;
    static formatAmount(amount: string, decimals?: number): string;
    static getTokenDenomFromSymbol(symbol: string): string;
    static getTokenSymbolFromDenom(denom: string): string;
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
    entity?: string;
    token: string;
    expected: string;
    actual: string;
    difference: string;
    percentage?: number;
}
export declare class ReportGenerator {
    static generateTestReport(testName: string, scenario: any, expectedRewards: any, actualRewards: any, executionTime: number, transactionHashes?: string[], gasUsed?: string): TestReport;
    private static findDiscrepancies;
    private static compareRewards;
    static printReport(report: TestReport): void;
    static saveReportToFile(report: TestReport, filename: string): void;
}
//# sourceMappingURL=test-utils.d.ts.map