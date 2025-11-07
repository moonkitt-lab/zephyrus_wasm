"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.EnvironmentSetup = void 0;
const fs_1 = __importDefault(require("fs"));
const path_1 = __importDefault(require("path"));
const test_utils_1 = require("./test-utils");
class EnvironmentSetup {
    constructor(logger, walletManager) {
        this.logger = logger;
        this.walletManager = walletManager;
    }
    async setupEnvironment() {
        try {
            this.logger.section("Environment Setup");
            // Step 1: Load contract addresses from existing deployment
            const contractAddresses = await this.loadContractAddresses();
            // Step 2: Verify contract deployment
            await this.verifyContractsDeployed(contractAddresses);
            // Step 3: Setup test user wallets
            await this.setupTestWallets();
            this.logger.info("Environment setup completed successfully");
            return {
                contractAddresses,
                success: true
            };
        }
        catch (error) {
            this.logger.error("Environment setup failed", error);
            return {
                contractAddresses: { hydro: "", tribute: "", zephyrus: "" },
                success: false,
                error: error instanceof Error ? error.message : String(error)
            };
        }
    }
    async loadContractAddresses() {
        this.logger.info("Loading contract addresses from deployment configs...");
        try {
            // Load Zephyrus config
            const zephyrusConfigPath = path_1.default.join(__dirname, "../../deploy_scripts/zephyrus_contract/config_devnet.json");
            if (fs_1.default.existsSync(zephyrusConfigPath)) {
                const zephyrusConfig = JSON.parse(fs_1.default.readFileSync(zephyrusConfigPath, "utf8"));
                const contractAddresses = {
                    hydro: zephyrusConfig.hydro_contract_address_1 || zephyrusConfig.hydro_contract_address,
                    tribute: zephyrusConfig.tribute_contract_address_1 || zephyrusConfig.tribute_contract_address,
                    zephyrus: "" // Will be loaded from instantiate result or config
                };
                // Try to load Zephyrus address from instantiate result
                const zephyrusInstantiateResultPath = path_1.default.join(__dirname, "../../deploy_scripts/zephyrus_contract/instantiate_zephyrus_res.json");
                if (fs_1.default.existsSync(zephyrusInstantiateResultPath)) {
                    const instantiateResult = JSON.parse(fs_1.default.readFileSync(zephyrusInstantiateResultPath, "utf8"));
                    // Extract contract address from instantiate result
                    if (instantiateResult.contractAddress) {
                        contractAddresses.zephyrus = instantiateResult.contractAddress;
                    }
                }
                // Validate that we have all required addresses
                if (!contractAddresses.hydro || !contractAddresses.tribute) {
                    throw new Error("Missing required contract addresses in config");
                }
                this.logger.info(`Loaded contract addresses:
          Hydro: ${contractAddresses.hydro}
          Tribute: ${contractAddresses.tribute}
          Zephyrus: ${contractAddresses.zephyrus || "NOT DEPLOYED"}`);
                return contractAddresses;
            }
            else {
                throw new Error(`Zephyrus config file not found: ${zephyrusConfigPath}`);
            }
        }
        catch (error) {
            this.logger.error("Failed to load contract addresses", error);
            throw error;
        }
    }
    async verifyContractsDeployed(contractAddresses) {
        this.logger.info("Verifying contract deployment...");
        const deployerWallet = await this.walletManager.getDeployerWallet();
        // Verify Hydro contract
        try {
            const hydroCodeInfo = await deployerWallet.client.getContract(contractAddresses.hydro);
            this.logger.info(`Hydro contract verified: ${hydroCodeInfo.address}`);
        }
        catch (error) {
            throw new Error(`Hydro contract not found at ${contractAddresses.hydro}: ${error}`);
        }
        // Verify Tribute contract
        try {
            const tributeCodeInfo = await deployerWallet.client.getContract(contractAddresses.tribute);
            this.logger.info(`Tribute contract verified: ${tributeCodeInfo.address}`);
        }
        catch (error) {
            throw new Error(`Tribute contract not found at ${contractAddresses.tribute}: ${error}`);
        }
        // Verify Zephyrus contract (if deployed)
        if (contractAddresses.zephyrus) {
            try {
                const zephyrusCodeInfo = await deployerWallet.client.getContract(contractAddresses.zephyrus);
                this.logger.info(`Zephyrus contract verified: ${zephyrusCodeInfo.address}`);
            }
            catch (error) {
                this.logger.warn(`Zephyrus contract not found at ${contractAddresses.zephyrus}: ${error}`);
                // Don't throw error as Zephyrus might need to be deployed
            }
        }
        this.logger.info("Contract verification completed");
    }
    async setupTestWallets() {
        this.logger.info("Setting up test wallets...");
        // Standard test user IDs from scenarios
        const testUserIds = ["A", "B", "C", "D", "E"];
        // Fund test wallets
        await this.walletManager.fundAllTestWallets(testUserIds);
        this.logger.info(`Test wallets setup completed for users: ${testUserIds.join(", ")}`);
    }
    async deployZephyrus(contractAddresses) {
        this.logger.info("Deploying Zephyrus contract...");
        try {
            const deployerWallet = await this.walletManager.getDeployerWallet();
            // Load Zephyrus WASM file
            const wasmPath = path_1.default.join(__dirname, "../../deploy_scripts/artifacts/zephyrus-main.wasm");
            if (!fs_1.default.existsSync(wasmPath)) {
                throw new Error(`Zephyrus WASM file not found: ${wasmPath}`);
            }
            const wasmCode = fs_1.default.readFileSync(wasmPath);
            // Store code
            this.logger.info("Storing Zephyrus code...");
            const storeResult = await deployerWallet.client.upload(deployerWallet.address, wasmCode, "auto");
            const codeId = storeResult.codeId;
            this.logger.info(`Zephyrus code stored with ID: ${codeId}`);
            // Instantiate contract
            this.logger.info("Instantiating Zephyrus contract...");
            const instantiateMsg = {
                commission_rate: "0.10", // 10%
                commission_recipient: deployerWallet.address,
                default_hydromancer_address: deployerWallet.address,
                default_hydromancer_commission_rate: "0.05", // 5%
                default_hydromancer_name: "Default Hydromancer",
                hydro_contract_address: contractAddresses.hydro,
                tribute_contract_address: contractAddresses.tribute,
                whitelist_admins: [deployerWallet.address]
            };
            const instantiateResult = await deployerWallet.client.instantiate(deployerWallet.address, codeId, instantiateMsg, "Zephyrus Test", "auto", {
                admin: deployerWallet.address
            });
            const zephyrusAddress = instantiateResult.contractAddress;
            this.logger.info(`Zephyrus contract deployed at: ${zephyrusAddress}`);
            // Save deployment result for future reference
            const deploymentResult = {
                contractAddress: zephyrusAddress,
                codeId: codeId,
                transactionHash: instantiateResult.transactionHash,
                deployedAt: new Date().toISOString()
            };
            const resultPath = path_1.default.join(__dirname, "../../deploy_scripts/zephyrus_contract/instantiate_zephyrus_res.json");
            fs_1.default.writeFileSync(resultPath, JSON.stringify(deploymentResult, null, 2));
            return zephyrusAddress;
        }
        catch (error) {
            this.logger.error("Failed to deploy Zephyrus contract", error);
            throw error;
        }
    }
    async runDeploymentScripts() {
        this.logger.info("Running deployment scripts...");
        try {
            // Check if contracts are already deployed
            const contractAddresses = await this.loadContractAddresses();
            if (contractAddresses.hydro && contractAddresses.tribute) {
                this.logger.info("Contracts already deployed, skipping deployment");
                // Deploy Zephyrus if not already deployed
                if (!contractAddresses.zephyrus) {
                    contractAddresses.zephyrus = await this.deployZephyrus(contractAddresses);
                }
                return contractAddresses;
            }
            // If contracts not deployed, recommend running the deployment scripts manually
            this.logger.warn("Contracts not found. Please run the deployment scripts manually:");
            this.logger.warn("1. cd deploy_scripts/hydro_contracts && ./setup_on_devnet.sh");
            this.logger.warn("2. cd ../zephyrus_contract && ./setup_on_devnet.sh");
            throw new Error("Contracts not deployed. Please run deployment scripts first.");
        }
        catch (error) {
            this.logger.error("Failed to run deployment scripts", error);
            throw error;
        }
    }
    async waitForContractsReady(contractAddresses) {
        this.logger.info("Waiting for contracts to be ready...");
        const maxRetries = 30;
        const retryDelay = 2000; // 2 seconds
        for (let i = 0; i < maxRetries; i++) {
            try {
                await this.verifyContractsDeployed(contractAddresses);
                this.logger.info("All contracts are ready");
                return;
            }
            catch (error) {
                if (i === maxRetries - 1) {
                    throw new Error(`Contracts not ready after ${maxRetries} retries: ${error}`);
                }
                this.logger.info(`Contracts not ready, waiting... (${i + 1}/${maxRetries})`);
                await test_utils_1.ContractUtils.wait(retryDelay);
            }
        }
    }
    async cleanup() {
        this.logger.info("Cleaning up environment setup...");
        // Cleanup would include:
        // - Clearing temporary files
        // - Resetting contract states if needed
        // - Cleaning up test wallets
        await this.walletManager.cleanup();
        this.logger.info("Environment cleanup completed");
    }
}
exports.EnvironmentSetup = EnvironmentSetup;
//# sourceMappingURL=environment-setup.js.map