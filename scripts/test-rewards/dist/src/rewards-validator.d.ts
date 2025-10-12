import { WalletManager } from "./wallet-manager";
import { TestLogger } from "./test-utils";
import { RewardsResult } from "./calculate-rewards";
import { ExecutionResult, ContractAddresses } from "./scenario-executor";
export interface ValidationResult {
    success: boolean;
    actualRewards: RewardsResult;
    discrepancies: any[];
    error?: string;
}
export interface ClaimableReward {
    denom: string;
    amount: string;
}
export interface UserClaimableRewards {
    [userId: string]: ClaimableReward[];
}
export declare class RewardsValidator {
    private logger;
    private walletManager;
    private contractAddresses;
    private clients;
    constructor(logger: TestLogger, walletManager: WalletManager, contractAddresses: ContractAddresses);
    initializeClients(): Promise<void>;
    validateRewards(expectedRewards: RewardsResult, executionResult: ExecutionResult): Promise<ValidationResult>;
    private queryActualRewards;
    private queryProtocolRewards;
    private queryHydromancerRewards;
    private queryUserRewards;
    private queryUserClaimableRewards;
    claimAllRewards(executionResult: ExecutionResult): Promise<boolean>;
    private claimUserRewards;
    private claimHydromancerRewards;
    private getUserTokenBalances;
    private logBalanceChanges;
    private getTokenSymbol;
}
//# sourceMappingURL=rewards-validator.d.ts.map