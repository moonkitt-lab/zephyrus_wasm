import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { TestLogger } from "./test-utils";
export interface TestWallet {
    address: string;
    wallet: DirectSecp256k1HdWallet;
    client: SigningCosmWasmClient;
    mnemonic: string;
}
export declare class WalletManager {
    private logger;
    private deployerWallet;
    private testWallets;
    constructor(logger: TestLogger);
    getDeployerWallet(): Promise<TestWallet>;
    createTestWallet(userId: string): Promise<TestWallet>;
    fundTestWallet(userId: string, amounts: {
        [denom: string]: string;
    }): Promise<void>;
    fundAllTestWallets(userIds: string[]): Promise<void>;
    getTestWallet(userId: string): TestWallet | undefined;
    getAllTestWallets(): Map<string, TestWallet>;
    getBalance(userId: string, denom: string): Promise<string>;
    private generateMnemonic;
    cleanup(): Promise<void>;
}
//# sourceMappingURL=wallet-manager.d.ts.map