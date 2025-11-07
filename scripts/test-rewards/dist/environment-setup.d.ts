import { WalletManager } from "./wallet-manager";
import { TestLogger } from "./test-utils";
import { ContractAddresses } from "./scenario-executor";
export interface SetupResult {
    contractAddresses: ContractAddresses;
    success: boolean;
    error?: string;
}
export declare class EnvironmentSetup {
    private logger;
    private walletManager;
    constructor(logger: TestLogger, walletManager: WalletManager);
    setupEnvironment(): Promise<SetupResult>;
    private loadContractAddresses;
    private verifyContractsDeployed;
    private setupTestWallets;
    deployZephyrus(contractAddresses: ContractAddresses): Promise<string>;
    runDeploymentScripts(): Promise<ContractAddresses>;
    waitForContractsReady(contractAddresses: ContractAddresses): Promise<void>;
    cleanup(): Promise<void>;
}
//# sourceMappingURL=environment-setup.d.ts.map