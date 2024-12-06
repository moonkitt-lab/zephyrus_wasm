import * as path from "node:path";
import { promises as fs } from "fs";
import {
  ExecuteResult,
  JsonObject,
  SigningCosmWasmClient,
} from "@cosmjs/cosmwasm-stargate";
import { TestSuite, WalletKeys } from "./test-suite";
import { requiredEnvVar } from "./test-suite/utils";
import { coin, Coin, DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { cwd } from "node:process";
import { calculateFee } from "@cosmjs/stargate";

export function artifact(name: string): string {
  return `${__dirname}/../../artifacts/${name}.wasm`;
}

const dateToNanoseconds = (dateString: string): bigint => {
  const date = new Date(dateString);
  const fractionalSeconds = dateString.split(".")[1]?.replace("Z", "") || "";
  let nanoseconds = BigInt(date.getTime()) * 1_000_000n;

  if (fractionalSeconds) {
    const fractionalNano = BigInt(fractionalSeconds.padEnd(9, "0").slice(0, 9)); // Ensure 9 digits
    nanoseconds += fractionalNano;
  }

  return nanoseconds;
};

function hydroArtifact(name: string): string {
  return path.resolve(
    cwd(),
    "target/test-suite/hydro",
    requiredEnvVar("HYDRO_VERSION"),
    "artifacts",
    `${name}.wasm`,
  );
}

export async function readContractFileBytes(
  filePath: string,
): Promise<Uint8Array> {
  try {
    await fs.access(filePath);
    const contents = await fs.readFile(filePath);
    return new Uint8Array(contents);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw new Error(`Contract file ${filePath} does not exist`);
    }

    throw error;
  }
}

export const getFileNameWithoutExtension = (filePath: string) =>
  path.basename(filePath, path.extname(filePath));

export const snakeCaseToKebabCase = (str: string) => str.replace(/_/g, "-");

export type HydroContracts = {
  hydro: string;
  tribute: string;
};

export type HydroConfig = {
  admin: string;
  roundLengthSecs: number;
  maxDeploymentDuration?: number;
};

export type HydroProposal = {
  trancheId: number;
  title: string;
  description: string;
  deploymentDuration: number;
  minAtomLiquidityRequest: number;
};

export type HydroTribute = {
  amount: number;
  denom: string;
  roundId: number;
  trancheId: number;
  proposalId: number;
};

export type ExecWasmOpts = {
  gas?: number;
  funds?: [Coin];
};

class Wallet {
  signer: DirectSecp256k1HdWallet;

  constructor(signer: DirectSecp256k1HdWallet) {
    this.signer = signer;
  }

  async address(): Promise<string> {
    const account = (await this.signer.getAccounts())[0];
    return account.address;
  }
}

export class NeutronWallet extends Wallet {
  client: SigningCosmWasmClient;
  gasPrice: string;

  constructor(
    suite: TestSuite,
    client: SigningCosmWasmClient,
    signer: DirectSecp256k1HdWallet,
  ) {
    super(signer);
    this.client = client;
    this.gasPrice = suite.getNetworkGasPrices("neutron");
  }

  static async connect(
    suite: TestSuite,
    walletKey: WalletKeys,
  ): Promise<NeutronWallet> {
    const mnemonic = suite.getWalletMnemonics()[walletKey];
    const signer = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
      prefix: suite.getNetworkPrefix("neutron"),
    });
    const client = await SigningCosmWasmClient.connectWithSigner(
      `http://${suite.getNetworkRpc("neutron")}`,
      signer,
    );

    return new NeutronWallet(suite, client, signer);
  }

  async uploadWasm(path: string, gas: number = 500_000): Promise<number> {
    const bytes = await readContractFileBytes(path);
    const fee = calculateFee(gas, this.gasPrice);
    const sender = await this.address();
    const res = await this.client.upload(sender, bytes, fee);
    return res.codeId;
  }

  async initWasm(
    codeId: number,
    msg: JsonObject,
    label: string,
    gas: number = 500_000,
  ): Promise<string> {
    const fee = calculateFee(gas, this.gasPrice);
    const sender = await this.address();
    const res = await this.client.instantiate(sender, codeId, msg, label, fee);
    return res.contractAddress;
  }

  async execWasm(
    contract: string,
    msg: JsonObject,
    opts?: ExecWasmOpts,
  ): Promise<ExecuteResult> {
    const fee = calculateFee(opts?.gas || 500_000, this.gasPrice);
    const sender = await this.address();
    const res = await this.client.execute(
      sender,
      contract,
      msg,
      fee,
      "",
      opts?.funds || [],
    );
    return res;
  }

  async queryWasm(contract: string, msg: JsonObject): Promise<JsonObject> {
    return this.client.queryContractSmart(contract, msg);
  }

  async deployHydro(config: HydroConfig): Promise<HydroContracts> {
    const hydroCodeId = await this.uploadWasm(
      hydroArtifact("hydro"),
      15_000_000,
    );
    const tributeCodeId = await this.uploadWasm(
      hydroArtifact("tribute"),
      15_000_000,
    );

    const admin = await this.address();
    const round_length = config.roundLengthSecs * 10 ** 9;
    const max_deployment_duration = config.maxDeploymentDuration || 3;
    const lastBlockTime = (await this.client.getBlock()).header.time;
    const first_round_start = String(dateToNanoseconds(lastBlockTime));

    const hydroInitMsg = {
      round_length,
      lock_epoch_length: round_length,
      tranches: [
        { name: "ATOM Bucket", metadata: "A bucket of ATOM to deploy as PoL" },
      ],
      first_round_start,
      is_in_pilot_mode: true,
      max_locked_tokens: "20000000000",
      whitelist_admins: [admin],
      initial_whitelist: [admin],
      max_validator_shares_participating: 500,
      hub_connection_id: "connection-0",
      hub_transfer_channel_id: "channel-0",
      icq_update_period: 109000,
      icq_managers: [admin],
      round_lock_power_schedule: [
        [1, "1"],
        [2, "1.25"],
        [3, "1.5"],
        [6, "2"],
        [12, "4"],
      ],
      max_deployment_duration,
    };

    const hydro = await this.initWasm(
      hydroCodeId,
      hydroInitMsg,
      "hydro-main",
      1_000_000,
    );

    const tributeInitMsg = { hydro_contract: hydro };

    const tribute = await this.initWasm(
      tributeCodeId,
      tributeInitMsg,
      "hydro-tribute",
      1_000_000,
    );

    return { hydro, tribute };
  }

  async createHydroProposal(
    hydro: HydroContracts,
    proposal: HydroProposal,
  ): Promise<ExecuteResult> {
    const msg = {
      create_proposal: {
        tranche_id: proposal.trancheId,
        title: proposal.title,
        description: proposal.description,
        deployment_duration: proposal.deploymentDuration,
        minimum_atom_liquidity_request: String(
          proposal.minAtomLiquidityRequest,
        ),
      },
    };

    return this.execWasm(hydro.hydro, msg);
  }

  async addHydroTribute(
    hydro: HydroContracts,
    tribute: HydroTribute,
  ): Promise<ExecuteResult> {
    const msg = {
      add_tribute: {
        round_id: tribute.roundId,
        tranche_id: tribute.trancheId,
        proposal_id: tribute.proposalId,
      },
    };

    return this.execWasm(hydro.tribute, msg, {
      funds: [coin(tribute.amount, tribute.denom)],
    });
  }
}
