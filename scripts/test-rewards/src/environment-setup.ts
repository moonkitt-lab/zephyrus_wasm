import fs from "fs";
import path from "path";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { WalletManager } from "./wallet-manager";
import { TestLogger, ContractUtils } from "./test-utils";
import { CONFIG } from "./config";
import { ContractAddresses } from "./scenario-executor";
import { Scenario } from "./calculate-rewards";

export interface SetupResult {
  contractAddresses: ContractAddresses;
  commissionRecipientAddress: string;
  success: boolean;
  error?: string;
}

export class EnvironmentSetup {
  private logger: TestLogger;
  private walletManager: WalletManager;
  private commissionRecipientAddress: string = "";
  private scenario?: Scenario;

  constructor(
    logger: TestLogger,
    walletManager: WalletManager,
    scenario?: Scenario
  ) {
    this.logger = logger;
    this.walletManager = walletManager;
    this.scenario = scenario;
  }

  async setupEnvironment(): Promise<SetupResult> {
    try {
      this.logger.section("Environment Setup");

      // Step 1: Setup test user + hydromancer + commission-recipient wallets
      await this.setupTestWallets();

      // Step 2: Load contract addresses from existing deployment
      const deploymentResult = await this.runDeploymentScripts();
      const contractAddresses = deploymentResult.contractAddresses;
      const commissionRecipientAddress =
        deploymentResult.commissionRecipientAddress;

      // Step 3: Verify contract deployment
      await this.verifyContractsDeployed(contractAddresses);

      this.logger.info("Environment setup completed successfully");

      return {
        contractAddresses,
        commissionRecipientAddress,
        success: true,
      };
    } catch (error) {
      this.logger.error("Environment setup failed", error);
      return {
        contractAddresses: { hydro: "", tribute: "", zephyrus: "" },
        commissionRecipientAddress: "",
        success: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  private async verifyContractsDeployed(
    contractAddresses: ContractAddresses
  ): Promise<void> {
    this.logger.info("Verifying contract deployment...");

    const deployerWallet = await this.walletManager.getDeployerWallet();

    // Verify Hydro contract
    try {
      const hydroCodeInfo = await deployerWallet.client.getContract(
        contractAddresses.hydro
      );
      this.logger.info(`Hydro contract verified: ${hydroCodeInfo.address}`);
    } catch (error) {
      throw new Error(
        `Hydro contract not found at ${contractAddresses.hydro}: ${error}`
      );
    }

    // Verify Tribute contract
    try {
      const tributeCodeInfo = await deployerWallet.client.getContract(
        contractAddresses.tribute
      );
      this.logger.info(`Tribute contract verified: ${tributeCodeInfo.address}`);
    } catch (error) {
      throw new Error(
        `Tribute contract not found at ${contractAddresses.tribute}: ${error}`
      );
    }

    // Verify Zephyrus contract (if deployed)
    if (contractAddresses.zephyrus) {
      try {
        const zephyrusCodeInfo = await deployerWallet.client.getContract(
          contractAddresses.zephyrus
        );
        this.logger.info(
          `Zephyrus contract verified: ${zephyrusCodeInfo.address}`
        );
      } catch (error) {
        this.logger.warn(
          `Zephyrus contract not found at ${contractAddresses.zephyrus}: ${error}`
        );
        // Don't throw error as Zephyrus might need to be deployed
      }
    }

    this.logger.info("Contract verification completed");
  }

  private async setupTestWallets(): Promise<void> {
    this.logger.info("Setting up test wallets...");

    // Extract user IDs from scenario - error if no scenario provided
    if (!this.scenario?.users) {
      throw new Error("No scenario provided or scenario missing users");
    }

    const testUserIds = this.scenario.users.map((user) => user.user_id);

    // Create and fund hydromancer wallet
    await this.setupHydromancerWallet();

    // Create commission recipient wallet (but don't fund it - we want to start with 0 balance)
    await this.setupCommissionRecipientWallet();

    // Fund test wallets with exact amounts based on their vessels
    await this.fundTestWalletsFromScenario();

    this.logger.info(
      `Test wallets setup completed for users: ${testUserIds.join(", ")}`
    );
  }

  private async setupHydromancerWallet(): Promise<void> {
    this.logger.info("Setting up hydromancer wallet...");

    // Create hydromancer wallet with temporary mnemonic
    const hydromancerWallet =
      await this.walletManager.createTestWallet("hydromancer");

    // Fund hydromancer with 1 NTRN for transactions
    await this.walletManager.fundTestWallet("hydromancer", {
      untrn: "1000000", // 1 NTRN (1,000,000 untrn)
    });

    this.logger.info(
      `Hydromancer wallet created and funded: ${hydromancerWallet.address}`
    );
  }

  private async setupCommissionRecipientWallet(): Promise<void> {
    this.logger.info("Setting up commission recipient wallet...");

    // Create commission recipient wallet with temporary mnemonic (but don't fund it)
    const commissionRecipientWallet = await this.walletManager.createTestWallet(
      "commission-recipient"
    );

    // Store the address for use in contract instantiation
    this.commissionRecipientAddress = commissionRecipientWallet.address;

    this.logger.info(
      `Commission recipient wallet created with 0 balance: ${commissionRecipientWallet.address}`
    );
  }

  async deployZephyrus(contractAddresses: ContractAddresses): Promise<string> {
    this.logger.info("Deploying Zephyrus contract...");

    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Load Zephyrus WASM file
      const wasmPath = path.join(
        __dirname,
        "../../deploy_scripts/artifacts/zephyrus-main.wasm"
      );

      if (!fs.existsSync(wasmPath)) {
        throw new Error(`Zephyrus WASM file not found: ${wasmPath}`);
      }

      const wasmCode = fs.readFileSync(wasmPath);

      // Store code
      this.logger.info("Storing Zephyrus code...");
      const storeResult = await deployerWallet.client.upload(
        deployerWallet.address,
        wasmCode,
        "auto"
      );

      const codeId = storeResult.codeId;
      this.logger.info(`Zephyrus code stored with ID: ${codeId}`);

      // Get commission recipient wallet address (created in setupCommissionRecipientWallet)
      const commissionRecipientWallet = this.walletManager.getTestWallet(
        "commission-recipient"
      );
      if (!commissionRecipientWallet) {
        throw new Error(
          "Commission recipient wallet not found. Make sure setupTestWallets() was called first."
        );
      }

      // Get hydromancer wallet address
      const hydromancerWallet = this.walletManager.getTestWallet("hydromancer");
      if (!hydromancerWallet) {
        throw new Error(
          "Hydromancer wallet not found. Make sure setupTestWallets() was called first."
        );
      }

      // Calculate commission rates from scenario config (BPS to decimal)
      const protocolCommissionRate = this.scenario?.protocol_config
        ? (
            this.scenario.protocol_config.protocol_commission_bps / 10000
          ).toFixed(4)
        : "0.05"; // Default 5% (500 BPS)

      const hydromancerCommissionRate = this.scenario?.protocol_config
        ? (
            this.scenario.protocol_config.hydromancer_commission_bps / 10000
          ).toFixed(4)
        : "0.01"; // Default 1% (100 BPS)

      this.logger.info(
        `Using commission rates from scenario config: Protocol ${protocolCommissionRate}, Hydromancer ${hydromancerCommissionRate}`
      );

      // Instantiate contract
      this.logger.info("Instantiating Zephyrus contract...");
      const instantiateMsg = {
        commission_rate: protocolCommissionRate,
        commission_recipient: commissionRecipientWallet.address,
        default_hydromancer_address: hydromancerWallet.address,
        default_hydromancer_commission_rate: hydromancerCommissionRate,
        default_hydromancer_name: "Default Hydromancer",
        hydro_contract_address: contractAddresses.hydro,
        tribute_contract_address: contractAddresses.tribute,
        whitelist_admins: [deployerWallet.address],
      };

      const instantiateResult = await deployerWallet.client.instantiate(
        deployerWallet.address,
        codeId,
        instantiateMsg,
        "Zephyrus Test",
        "auto",
        {
          admin: deployerWallet.address,
        }
      );

      const zephyrusAddress = instantiateResult.contractAddress;
      this.logger.info(`Zephyrus contract deployed at: ${zephyrusAddress}`);

      // Save deployment result for future reference
      const deploymentResult = {
        contractAddress: zephyrusAddress,
        codeId: codeId,
        transactionHash: instantiateResult.transactionHash,
        deployedAt: new Date().toISOString(),
      };

      const resultPath = path.join(
        __dirname,
        "../../deploy_scripts/zephyrus_contract/instantiate_zephyrus_res.json"
      );

      fs.writeFileSync(resultPath, JSON.stringify(deploymentResult, null, 2));

      return zephyrusAddress;
    } catch (error) {
      this.logger.error("Failed to deploy Zephyrus contract", error);
      throw error;
    }
  }

  async deployTribute(hydroAddress: string): Promise<string> {
    this.logger.info("Deploying Tribute contract...");

    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Load Tribute WASM file
      const wasmPath = path.join(
        __dirname,
        "../../deploy_scripts/artifacts/tribute.wasm"
      );

      if (!fs.existsSync(wasmPath)) {
        throw new Error(`Tribute WASM file not found: ${wasmPath}`);
      }

      const wasmCode = fs.readFileSync(wasmPath);

      // Store code
      this.logger.info("Storing Tribute code...");
      const storeResult = await deployerWallet.client.upload(
        deployerWallet.address,
        wasmCode,
        "auto"
      );

      const codeId = storeResult.codeId;
      this.logger.info(`Tribute code stored with ID: ${codeId}`);

      // Instantiate contract
      this.logger.info("Instantiating Tribute contract...");
      const instantiateMsg = {
        hydro_contract: hydroAddress,
      };

      const instantiateResult = await deployerWallet.client.instantiate(
        deployerWallet.address,
        codeId,
        instantiateMsg,
        "Tribute Test",
        "auto",
        {
          admin: deployerWallet.address,
        }
      );

      const tributeAddress = instantiateResult.contractAddress;
      this.logger.info(`Tribute contract deployed at: ${tributeAddress}`);

      // Save deployment result for future reference
      const deploymentResult = {
        contractAddress: tributeAddress,
        codeId: codeId,
        transactionHash: instantiateResult.transactionHash,
        deployedAt: new Date().toISOString(),
      };

      const resultPath = path.join(
        __dirname,
        "../../deploy_scripts/zephyrus_contract/instantiate_tribute_res.json"
      );

      fs.writeFileSync(resultPath, JSON.stringify(deploymentResult, null, 2));
      this.logger.info(`Tribute deployment result saved to: ${resultPath}`);

      return tributeAddress;
    } catch (error) {
      this.logger.error("Failed to deploy Tribute contract", error);
      throw error;
    }
  }

  async deploySTTokenInfoProvider(): Promise<number> {
    this.logger.info("Deploying ST TokenInfoProvider contract...");

    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Load ST TokenInfoProvider WASM file
      const wasmPath = path.join(
        __dirname,
        "../../deploy_scripts/artifacts/st_token_info_provider.wasm"
      );

      if (!fs.existsSync(wasmPath)) {
        throw new Error(
          `ST TokenInfoProvider WASM file not found: ${wasmPath}`
        );
      }

      const wasmCode = fs.readFileSync(wasmPath);

      // Store code
      this.logger.info("Storing ST TokenInfoProvider code...");
      const storeResult = await deployerWallet.client.upload(
        deployerWallet.address,
        wasmCode,
        "auto"
      );

      const codeId = storeResult.codeId;
      this.logger.info(`ST TokenInfoProvider code stored with ID: ${codeId}`);

      // Instantiate contract
      this.logger.info("Instantiating ST TokenInfoProvider contract...");
      const instantiateMsg = {
        icq_update_period: 10000000,
        st_token_denom:
          "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
        stride_connection_id: "512",
        stride_host_zone_id: "",
        token_group_id: "statom",
      };

      const instantiateResult = await deployerWallet.client.instantiate(
        deployerWallet.address,
        codeId,
        instantiateMsg,
        "ST Token Info Provider statom",
        "auto",
        {
          admin: deployerWallet.address,
        }
      );

      const stTokenInfoProviderAddress = instantiateResult.contractAddress;
      this.logger.info(
        `ST TokenInfoProvider contract deployed at: ${stTokenInfoProviderAddress}`
      );

      return codeId;
    } catch (error) {
      this.logger.error(
        "Failed to deploy ST TokenInfoProvider contract",
        error
      );
      throw error;
    }
  }

  async deployDTokenInfoProvider(): Promise<number> {
    this.logger.info("Deploying D TokenInfoProvider contract...");

    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Load D TokenInfoProvider WASM file
      const wasmPath = path.join(
        __dirname,
        "../../deploy_scripts/artifacts/d_token_info_provider.wasm"
      );

      if (!fs.existsSync(wasmPath)) {
        throw new Error(`D TokenInfoProvider WASM file not found: ${wasmPath}`);
      }

      const wasmCode = fs.readFileSync(wasmPath);

      // Store code
      this.logger.info("Storing D TokenInfoProvider code...");
      const storeResult = await deployerWallet.client.upload(
        deployerWallet.address,
        wasmCode,
        "auto"
      );

      const codeId = storeResult.codeId;
      this.logger.info(`D TokenInfoProvider code stored with ID: ${codeId}`);

      // Instantiate contract
      this.logger.info("Instantiating D TokenInfoProvider contract...");
      const instantiateMsg = {
        d_token_denom:
          "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
        drop_staking_core_contract:
          "neutron16m3hjh7l04kap086jgwthduma0r5l0wh8kc6kaqk92ge9n5aqvys9q6lxr",
        token_group_id: "datom",
      };

      const instantiateResult = await deployerWallet.client.instantiate(
        deployerWallet.address,
        codeId,
        instantiateMsg,
        "D Token Info Provider datom",
        "auto",
        {
          admin: deployerWallet.address,
        }
      );

      const dTokenInfoProviderAddress = instantiateResult.contractAddress;
      this.logger.info(
        `D TokenInfoProvider contract deployed at: ${dTokenInfoProviderAddress}`
      );

      return codeId;
    } catch (error) {
      this.logger.error("Failed to deploy D TokenInfoProvider contract", error);
      throw error;
    }
  }

  async deployHydro(): Promise<string> {
    this.logger.info("Deploying Hydro contract...");

    try {
      const deployerWallet = await this.walletManager.getDeployerWallet();

      // Deploy TokenInfoProvider contracts first
      this.logger.info("Deploying TokenInfoProvider contracts...");
      const stTokenInfoProviderCodeId = await this.deploySTTokenInfoProvider();
      const dTokenInfoProviderCodeId = await this.deployDTokenInfoProvider();

      // Load Hydro WASM file
      const wasmPath = path.join(
        __dirname,
        "../../deploy_scripts/artifacts/hydro.wasm"
      );

      if (!fs.existsSync(wasmPath)) {
        throw new Error(`Hydro WASM file not found: ${wasmPath}`);
      }

      const wasmCode = fs.readFileSync(wasmPath);

      // Store code
      this.logger.info("Storing Hydro code...");
      const storeResult = await deployerWallet.client.upload(
        deployerWallet.address,
        wasmCode,
        "auto"
      );

      const codeId = storeResult.codeId;
      this.logger.info(`Hydro code stored with ID: ${codeId}`);

      // Instantiate contract
      this.logger.info("Instantiating Hydro contract...");

      // Create timestamp in nanoseconds format like the shell script
      const currentTime = Math.floor(Date.now() / 1000);
      const firstRoundStartTime = currentTime.toString() + "000000000";

      const roundLength = this.scenario?.protocol_config?.round_length
        ? this.scenario.protocol_config.round_length.toString()
        : "240000000000"; // Default 4 minutes in nanoseconds

      this.logger.info(`Hydro round length: ${roundLength}`);

      const instantiateMsg = {
        round_length: parseInt(roundLength),
        lock_epoch_length: parseInt(roundLength),
        is_in_pilot_mode: true,
        tranches: [
          {
            name: "ATOM Bucket",
            metadata: "A bucket of ATOM to deploy as PoL",
          },
          {
            name: "USDC Bucket",
            metadata:
              "This is a bucket for USDC from the Cosmos Hub community pool.",
          },
        ],
        first_round_start: firstRoundStartTime, // String format like shell script
        max_locked_tokens: "500000000000", // ~500k ATOM
        whitelist_admins: [deployerWallet.address],
        initial_whitelist: [deployerWallet.address],
        icq_managers: [deployerWallet.address],
        round_lock_power_schedule: [
          [1, "1"], // Round 1: 100% lock power
          [2, "1.25"], // Round 2: 125% lock power
          [3, "1.5"], // Round 3: 150% lock power
          [6, "2"], // Round 6: 200% lock power
          [12, "4"], // Round 12: 400% lock power
        ],
        max_deployment_duration: 3,
        token_info_providers: [
          {
            lsm: {
              max_validator_shares_participating: 500,
              hub_connection_id: "connection-0",
              hub_transfer_channel_id: "channel-0",
              icq_update_period: 10,
            },
          },
          {
            token_info_provider_contract: {
              code_id: stTokenInfoProviderCodeId,
              msg: "eyJpY3FfdXBkYXRlX3BlcmlvZCI6MTAwMDAwMDAsInN0X3Rva2VuX2Rlbm9tIjoiaWJjL0I3ODY0QjAzRTFCOUZENEYwNDkyNDNFOTJBQkQ2OTE1ODZGNjgyMTM3MDM3QTlGM0ZDQTUyMjI4MTU2MjBCM0MiLCJzdHJpZGVfY29ubmVjdGlvbl9pZCI6IjUxMiIsInN0cmlkZV9ob3N0X3pvbmVfaWQiOiIiLCJ0b2tlbl9ncm91cF9pZCI6InN0YXRvbSJ9",
              label: "ST Token Info Provider statom",
              admin: null,
            },
          },
          {
            token_info_provider_contract: {
              code_id: dTokenInfoProviderCodeId,
              msg: "eyJkX3Rva2VuX2Rlbm9tIjoiZmFjdG9yeS9uZXV0cm9uMWs2aHIwZjgzZTd1bjJ3amYyOWNzcGs3ajY5anJuc2trNjVrM2VrMm5qOWR6dHJsenBqNnEwMHJ0c2EvdWRhdG9tIiwiZHJvcF9zdGFraW5nX2NvcmVfY29udHJhY3QiOiJuZXV0cm9uMTZtM2hqaDdsMDRrYXAwODZqZ3d0aGR1bWEwcjVsMHdoOGtjNmthcWs5MmdlOW41YXF2eXM5cTZseHIiLCJ0b2tlbl9ncm91cF9pZCI6ImRhdG9tIn0=",
              label: "D Token Info Provider datom",
              admin: null,
            },
          },
        ],
        gatekeeper: null,
        cw721_collection_info: {
          name: "Hydro Lockups",
          symbol: "hydro-lockups",
        },
        lock_expiry_duration_seconds: 31536000, // 1 year
        lock_depth_limit: 10,
        slash_percentage_threshold: "0.1", // 10%
        slash_tokens_receiver_addr: deployerWallet.address,
      };

      const instantiateResult = await deployerWallet.client.instantiate(
        deployerWallet.address,
        codeId,
        instantiateMsg,
        "Hydro Test",
        "auto",
        {
          admin: deployerWallet.address,
        }
      );

      const hydroAddress = instantiateResult.contractAddress;
      this.logger.info(`Hydro contract deployed at: ${hydroAddress}`);

      // Save deployment result for future reference
      const deploymentResult = {
        contractAddress: hydroAddress,
        codeId: codeId,
        transactionHash: instantiateResult.transactionHash,
        deployedAt: new Date().toISOString(),
      };

      const resultPath = path.join(
        __dirname,
        "../../deploy_scripts/zephyrus_contract/instantiate_hydro_res.json"
      );

      fs.writeFileSync(resultPath, JSON.stringify(deploymentResult, null, 2));
      this.logger.info(`Hydro deployment result saved to: ${resultPath}`);

      return hydroAddress;
    } catch (error) {
      this.logger.error("Failed to deploy Hydro contract", error);
      throw error;
    }
  }

  async runDeploymentScripts(): Promise<{
    contractAddresses: ContractAddresses;
    commissionRecipientAddress: string;
  }> {
    this.logger.info("Running deployment scripts...");

    try {
      // Check if contracts are already deployed
      const contractAddresses: ContractAddresses = {
        hydro: "",
        tribute: "",
        zephyrus: "",
      };

      this.logger.info("Hydro contract not found, deploying...");
      contractAddresses.hydro = await this.deployHydro();

      this.logger.info("Tribute contract not found, deploying...");
      contractAddresses.tribute = await this.deployTribute(
        contractAddresses.hydro
      );

      this.logger.info("Zephyrus contract not found, deploying...");
      contractAddresses.zephyrus = await this.deployZephyrus(contractAddresses);

      this.logger.info("All contracts deployed successfully");
      return {
        contractAddresses,
        commissionRecipientAddress: this.commissionRecipientAddress,
      };
    } catch (error) {
      this.logger.error("Failed to run deployment scripts", error);
      throw error;
    }
  }

  async waitForContractsReady(
    contractAddresses: ContractAddresses
  ): Promise<void> {
    this.logger.info("Waiting for contracts to be ready...");

    const maxRetries = 30;
    const retryDelay = 2000; // 2 seconds

    for (let i = 0; i < maxRetries; i++) {
      try {
        await this.verifyContractsDeployed(contractAddresses);
        this.logger.info("All contracts are ready");
        return;
      } catch (error) {
        if (i === maxRetries - 1) {
          throw new Error(
            `Contracts not ready after ${maxRetries} retries: ${error}`
          );
        }

        this.logger.info(
          `Contracts not ready, waiting... (${i + 1}/${maxRetries})`
        );
        await ContractUtils.wait(retryDelay);
      }
    }
  }

  async cleanup(): Promise<void> {
    this.logger.info("Cleaning up environment setup...");

    // Cleanup would include:
    // - Clearing temporary files
    // - Resetting contract states if needed
    // - Cleaning up test wallets

    await this.walletManager.cleanup();

    this.logger.info("Environment cleanup completed");
  }

  private async fundTestWalletsFromScenario(): Promise<void> {
    if (!this.scenario?.users) {
      throw new Error("No scenario provided for funding calculation");
    }

    this.logger.info(
      "Calculating funding amounts based on vessel requirements..."
    );

    // Calculate required amounts for each user based on their vessels
    for (const user of this.scenario.users) {
      const requiredAmounts: { [denom: string]: string } = {};

      // Add 1 NTRN for transaction fees
      requiredAmounts[CONFIG.tokenDenoms.NTRN] = "1"; // 1 NTRN for fees

      // Calculate total needed for each token based on vessels
      for (const vessel of user.vessels) {
        const amount = parseFloat(vessel.locked_amount);
        let denom: string;

        // Map vessel token names to actual denominations
        switch (vessel.locked_denom) {
          case "dATOM":
            denom = CONFIG.tokenDenoms.DATOM;
            break;
          case "stATOM":
            denom = CONFIG.tokenDenoms.STATOM;
            break;
          default:
            throw new Error(`Unsupported vessel token: ${vessel.locked_denom}`);
        }

        // Add to required amount (exact amount, no buffer)
        if (requiredAmounts[denom]) {
          requiredAmounts[denom] = (
            parseFloat(requiredAmounts[denom]) + amount
          ).toString();
        } else {
          requiredAmounts[denom] = amount.toString();
        }
      }

      // Convert to micro units and fund the wallet
      const fundingAmounts: { [denom: string]: string } = {};
      for (const [denom, amount] of Object.entries(requiredAmounts)) {
        // Convert to micro units (multiply by 1,000,000)
        fundingAmounts[denom] = Math.ceil(
          parseFloat(amount) * 1000000
        ).toString();
      }

      this.logger.info(
        `Funding user ${user.user_id} with calculated amounts: ${JSON.stringify(fundingAmounts)}`
      );
      await this.walletManager.fundTestWallet(user.user_id, fundingAmounts);
    }

    this.logger.info("All test wallets funded with exact amounts");
  }
}
