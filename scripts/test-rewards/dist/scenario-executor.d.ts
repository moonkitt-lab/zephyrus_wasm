import { WalletManager } from "./wallet-manager";
import { TestLogger } from "./test-utils";
import { Scenario } from "./calculate-rewards";
export interface ExecutionResult {
    transactionHashes: string[];
    vesselIds: {
        [userId: string]: number[];
    };
    proposalIds: number[];
    success: boolean;
    error?: string;
}
export interface ContractAddresses {
    hydro: string;
    tribute: string;
    zephyrus: string;
}
export declare class ScenarioExecutor {
    private logger;
    private walletManager;
    private contractAddresses;
    private clients;
    constructor(logger: TestLogger, walletManager: WalletManager, contractAddresses: ContractAddresses);
    initializeClients(): Promise<void>;
    executeScenario(scenario: Scenario): Promise<ExecutionResult>;
    private createTributeProposals;
    private addTributeToProposal;
    private createVesselsForUsers;
    private createVessel;
    private delegateVesselToZephyrus;
    private executeVotes;
    private createVesselMapping;
    private getUserVotes;
    private executeUserVotes;
    private areVesselsZephyrusControlled;
    private voteViaZephyrus;
    private voteViaHydro;
    private waitForRoundProgression;
    private simulateLiquidityDeployment;
}
//# sourceMappingURL=scenario-executor.d.ts.map