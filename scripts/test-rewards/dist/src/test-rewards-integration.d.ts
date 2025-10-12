#!/usr/bin/env node
import { TestReport } from "./test-utils";
declare class TestRewardsIntegration {
    private logger;
    private walletManager;
    private rewardsCalculator;
    private environmentSetup;
    private scenarioExecutor?;
    private rewardsValidator?;
    constructor();
    runTest(scenarioFile: string): Promise<TestReport>;
    private loadScenario;
    private validateScenario;
    private waitForRewardsClaimable;
    private ensureReportsDirectory;
    private cleanup;
}
export declare function runBatchTests(scenarioFiles: string[]): Promise<TestReport[]>;
export { TestRewardsIntegration };
//# sourceMappingURL=test-rewards-integration.d.ts.map