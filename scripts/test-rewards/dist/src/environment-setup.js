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
            const contractAddresses = await this.runDeploymentScripts();
            // Step 2: Verify contract deployment
            await this.verifyContractsDeployed(contractAddresses);
            // Step 3: Setup test user wallets
            await this.setupTestWallets();
            this.logger.info("Environment setup completed successfully");
            return {
                contractAddresses,
                success: true,
            };
        }
        catch (error) {
            this.logger.error("Environment setup failed", error);
            return {
                contractAddresses: { hydro: "", tribute: "", zephyrus: "" },
                success: false,
                error: error instanceof Error ? error.message : String(error),
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
                    hydro: zephyrusConfig.hydro_contract_address_1 ||
                        zephyrusConfig.hydro_contract_address,
                    tribute: zephyrusConfig.tribute_contract_address_1 ||
                        zephyrusConfig.tribute_contract_address,
                    zephyrus: "", // Will be loaded from instantiate result or config
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
                whitelist_admins: [deployerWallet.address],
            };
            const instantiateResult = await deployerWallet.client.instantiate(deployerWallet.address, codeId, instantiateMsg, "Zephyrus Test", "auto", {
                admin: deployerWallet.address,
            });
            const zephyrusAddress = instantiateResult.contractAddress;
            this.logger.info(`Zephyrus contract deployed at: ${zephyrusAddress}`);
            // Save deployment result for future reference
            const deploymentResult = {
                contractAddress: zephyrusAddress,
                codeId: codeId,
                transactionHash: instantiateResult.transactionHash,
                deployedAt: new Date().toISOString(),
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
    async deployTribute(hydroAddress) {
        this.logger.info("Deploying Tribute contract...");
        try {
            const deployerWallet = await this.walletManager.getDeployerWallet();
            // Load Tribute WASM file
            const wasmPath = path_1.default.join(__dirname, "../../deploy_scripts/artifacts/tribute.wasm");
            if (!fs_1.default.existsSync(wasmPath)) {
                throw new Error(`Tribute WASM file not found: ${wasmPath}`);
            }
            const wasmCode = fs_1.default.readFileSync(wasmPath);
            // Store code
            this.logger.info("Storing Tribute code...");
            const storeResult = await deployerWallet.client.upload(deployerWallet.address, wasmCode, "auto");
            const codeId = storeResult.codeId;
            this.logger.info(`Tribute code stored with ID: ${codeId}`);
            // Instantiate contract
            this.logger.info("Instantiating Tribute contract...");
            const instantiateMsg = {
                hydro_contract: hydroAddress,
            };
            const instantiateResult = await deployerWallet.client.instantiate(deployerWallet.address, codeId, instantiateMsg, "Tribute Test", "auto", {
                admin: deployerWallet.address,
            });
            const tributeAddress = instantiateResult.contractAddress;
            this.logger.info(`Tribute contract deployed at: ${tributeAddress}`);
            // Save deployment result for future reference
            const deploymentResult = {
                contractAddress: tributeAddress,
                codeId: codeId,
                transactionHash: instantiateResult.transactionHash,
                deployedAt: new Date().toISOString(),
            };
            const resultPath = path_1.default.join(__dirname, "../../deploy_scripts/zephyrus_contract/instantiate_tribute_res.json");
            fs_1.default.writeFileSync(resultPath, JSON.stringify(deploymentResult, null, 2));
            this.logger.info(`Tribute deployment result saved to: ${resultPath}`);
            return tributeAddress;
        }
        catch (error) {
            this.logger.error("Failed to deploy Tribute contract", error);
            throw error;
        }
    }
    async deploySTTokenInfoProvider() {
        this.logger.info("Deploying ST TokenInfoProvider contract...");
        try {
            const deployerWallet = await this.walletManager.getDeployerWallet();
            // Load ST TokenInfoProvider WASM file
            const wasmPath = path_1.default.join(__dirname, "../../deploy_scripts/artifacts/st_token_info_provider.wasm");
            if (!fs_1.default.existsSync(wasmPath)) {
                throw new Error(`ST TokenInfoProvider WASM file not found: ${wasmPath}`);
            }
            const wasmCode = fs_1.default.readFileSync(wasmPath);
            // Store code
            this.logger.info("Storing ST TokenInfoProvider code...");
            const storeResult = await deployerWallet.client.upload(deployerWallet.address, wasmCode, "auto");
            const codeId = storeResult.codeId;
            this.logger.info(`ST TokenInfoProvider code stored with ID: ${codeId}`);
            // Instantiate contract
            this.logger.info("Instantiating ST TokenInfoProvider contract...");
            const instantiateMsg = {
                icq_update_period: 10000000,
                st_token_denom: "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
                stride_connection_id: "512",
                stride_host_zone_id: "",
                token_group_id: "statom",
            };
            const instantiateResult = await deployerWallet.client.instantiate(deployerWallet.address, codeId, instantiateMsg, "ST Token Info Provider statom", "auto", {
                admin: deployerWallet.address,
            });
            const stTokenInfoProviderAddress = instantiateResult.contractAddress;
            this.logger.info(`ST TokenInfoProvider contract deployed at: ${stTokenInfoProviderAddress}`);
            return codeId;
        }
        catch (error) {
            this.logger.error("Failed to deploy ST TokenInfoProvider contract", error);
            throw error;
        }
    }
    async deployDTokenInfoProvider() {
        this.logger.info("Deploying D TokenInfoProvider contract...");
        try {
            const deployerWallet = await this.walletManager.getDeployerWallet();
            // Load D TokenInfoProvider WASM file
            const wasmPath = path_1.default.join(__dirname, "../../deploy_scripts/artifacts/d_token_info_provider.wasm");
            if (!fs_1.default.existsSync(wasmPath)) {
                throw new Error(`D TokenInfoProvider WASM file not found: ${wasmPath}`);
            }
            const wasmCode = fs_1.default.readFileSync(wasmPath);
            // Store code
            this.logger.info("Storing D TokenInfoProvider code...");
            const storeResult = await deployerWallet.client.upload(deployerWallet.address, wasmCode, "auto");
            const codeId = storeResult.codeId;
            this.logger.info(`D TokenInfoProvider code stored with ID: ${codeId}`);
            // Instantiate contract
            this.logger.info("Instantiating D TokenInfoProvider contract...");
            const instantiateMsg = {
                d_token_denom: "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
                drop_staking_core_contract: "neutron16m3hjh7l04kap086jgwthduma0r5l0wh8kc6kaqk92ge9n5aqvys9q6lxr",
                token_group_id: "datom",
            };
            const instantiateResult = await deployerWallet.client.instantiate(deployerWallet.address, codeId, instantiateMsg, "D Token Info Provider datom", "auto", {
                admin: deployerWallet.address,
            });
            const dTokenInfoProviderAddress = instantiateResult.contractAddress;
            this.logger.info(`D TokenInfoProvider contract deployed at: ${dTokenInfoProviderAddress}`);
            return codeId;
        }
        catch (error) {
            this.logger.error("Failed to deploy D TokenInfoProvider contract", error);
            throw error;
        }
    }
    async deployHydro() {
        this.logger.info("Deploying Hydro contract...");
        try {
            const deployerWallet = await this.walletManager.getDeployerWallet();
            // Deploy TokenInfoProvider contracts first
            this.logger.info("Deploying TokenInfoProvider contracts...");
            const stTokenInfoProviderCodeId = await this.deploySTTokenInfoProvider();
            const dTokenInfoProviderCodeId = await this.deployDTokenInfoProvider();
            // Load Hydro WASM file
            const wasmPath = path_1.default.join(__dirname, "../../deploy_scripts/artifacts/hydro.wasm");
            if (!fs_1.default.existsSync(wasmPath)) {
                throw new Error(`Hydro WASM file not found: ${wasmPath}`);
            }
            const wasmCode = fs_1.default.readFileSync(wasmPath);
            // Store code
            this.logger.info("Storing Hydro code...");
            const storeResult = await deployerWallet.client.upload(deployerWallet.address, wasmCode, "auto");
            const codeId = storeResult.codeId;
            this.logger.info(`Hydro code stored with ID: ${codeId}`);
            // Instantiate contract
            this.logger.info("Instantiating Hydro contract...");
            const instantiateMsg = {
                round_length: "3600000000000", // 1 hour in nanoseconds (from script)
                lock_epoch_length: "3600000000000", // 1 hour in nanoseconds (from script)
                is_in_pilot_mode: true, // From script
                tranches: [
                    {
                        name: "ATOM Bucket",
                        metadata: "A bucket of ATOM to deploy as PoL",
                    },
                    {
                        name: "USDC Bucket",
                        metadata: "This is a bucket for USDC from the Cosmos Hub community pool.",
                    },
                ],
                first_round_start: Math.floor(Date.now() / 1000).toString() + "000000000", // Current timestamp in nanoseconds
                max_locked_tokens: "500000000000", // ~500k ATOM (from script)
                whitelist_admins: [deployerWallet.address],
                initial_whitelist: [deployerWallet.address],
                icq_managers: [deployerWallet.address],
                max_deployment_duration: 3, // From script
                round_lock_power_schedule: [
                    [1, "1"], // Round 1: 100% lock power
                    [2, "1.25"], // Round 2: 125% lock power
                    [3, "1.5"], // Round 3: 150% lock power
                    [6, "2"], // Round 6: 200% lock power
                    [12, "4"], // Round 12: 400% lock power
                ],
                token_info_providers: [
                    {
                        lsm: {
                            max_validator_shares_participating: 500,
                            hub_connection_id: "connection-0", // Default for tests
                            hub_transfer_channel_id: "channel-0", // Default for tests
                            icq_update_period: 10,
                        },
                    },
                    {
                        token_info_provider_contract: {
                            code_id: stTokenInfoProviderCodeId,
                            msg: {
                                icq_update_period: 10000000,
                                st_token_denom: "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
                                stride_connection_id: "512",
                                stride_host_zone_id: "",
                                token_group_id: "statom",
                            },
                            label: "ST Token Info Provider statom",
                            admin: null,
                        },
                    },
                    {
                        token_info_provider_contract: {
                            code_id: dTokenInfoProviderCodeId,
                            msg: {
                                d_token_denom: "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
                                drop_staking_core_contract: "neutron16m3hjh7l04kap086jgwthduma0r5l0wh8kc6kaqk92ge9n5aqvys9q6lxr",
                                token_group_id: "datom",
                            },
                            label: "D Token Info Provider datom",
                            admin: null,
                        },
                    },
                ],
                gatekeeper: null, // No gatekeeper for tests
                cw721_collection_info: {
                    name: "Hydro Lockups",
                    symbol: "hydro-lockups",
                },
                lock_expiry_duration_seconds: 31536000, // 1 year
                lock_depth_limit: 10,
                slash_percentage_threshold: "0.1", // 10%
                slash_tokens_receiver_addr: deployerWallet.address,
            };
            const instantiateResult = await deployerWallet.client.instantiate(deployerWallet.address, codeId, instantiateMsg, "Hydro Test", "auto", {
                admin: deployerWallet.address,
            });
            const hydroAddress = instantiateResult.contractAddress;
            this.logger.info(`Hydro contract deployed at: ${hydroAddress}`);
            // Save deployment result for future reference
            const deploymentResult = {
                contractAddress: hydroAddress,
                codeId: codeId,
                transactionHash: instantiateResult.transactionHash,
                deployedAt: new Date().toISOString(),
            };
            const resultPath = path_1.default.join(__dirname, "../../deploy_scripts/zephyrus_contract/instantiate_hydro_res.json");
            fs_1.default.writeFileSync(resultPath, JSON.stringify(deploymentResult, null, 2));
            this.logger.info(`Hydro deployment result saved to: ${resultPath}`);
            return hydroAddress;
        }
        catch (error) {
            this.logger.error("Failed to deploy Hydro contract", error);
            throw error;
        }
    }
    async runDeploymentScripts() {
        this.logger.info("Running deployment scripts...");
        try {
            // Check if contracts are already deployed
            const contractAddresses = {
                hydro: "",
                tribute: "",
                zephyrus: "",
            };
            this.logger.info("Hydro contract not found, deploying...");
            contractAddresses.hydro = await this.deployHydro();
            this.logger.info("Tribute contract not found, deploying...");
            contractAddresses.tribute = await this.deployTribute(contractAddresses.hydro);
            this.logger.info("Zephyrus contract not found, deploying...");
            contractAddresses.zephyrus = await this.deployZephyrus(contractAddresses);
            this.logger.info("All contracts deployed successfully");
            return contractAddresses;
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