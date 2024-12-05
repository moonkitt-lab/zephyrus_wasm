import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./test-suite";
import {
  GAIA_GAS_PRICE,
  NEUTRON_GAS_PRICE,
  WALLET_KEYS,
  WALLET_MNEMONIC_WORD_COUNT,
} from "./test-suite/constants";
import { isContainerPaused } from "./test-suite/utils";
import {
  StargateClient,
  QueryClient,
  setupStakingExtension,
} from "@cosmjs/stargate";
import { Tendermint37Client } from "@cosmjs/tendermint-rpc";

let suite: TestSuite;

describe("TestSuite sanity check", () => {
  beforeAll(async () => {
    console.log("intializing test suite...");
    console.log(
      "TODO: Speed up. It waits for IBC channels to be established between the chains via Hermes, seems to take a while.",
    );
    suite = await TestSuite.create({
      networkOverrides: {
        gaia: {
          validators: 6,
          validators_balance: [
            "100000000",
            "100000000",
            "100000000",
            "100000000",
            "100000000",
            "100000000",
          ],
        },
      },
    });
  });

  afterAll(async () => {
    await suite.cleanup();
  });

  it("should get the correct neutron prefix", () => {
    const prefix = suite.getNetworkPrefix("neutron");
    expect(prefix).toBe("neutron");
  });

  it("should get the correct neutron RPC", () => {
    const rpc = suite.getNetworkRpc("neutron");
    expect(rpc).toMatch(/^127\.0\.0\.1:\d{5}$/);
  });

  it("should get the neutron gas price", () => {
    const price = suite.getNetworkGasPrices("neutron");
    const priceStr = `${NEUTRON_GAS_PRICE.amount}${NEUTRON_GAS_PRICE.denom}`;
    expect(price).toBe(priceStr);
  });

  it("should get the correct gaia prefix", () => {
    const prefix = suite.getNetworkPrefix("gaia");
    expect(prefix).toBe("cosmos");
  });

  it("should get the correct gaia RPC", () => {
    const rpc = suite.getNetworkRpc("gaia");
    expect(rpc).toMatch(/^127\.0\.0\.1:\d{5}$/);
  });

  it("should get the gaia gas price", () => {
    const price = suite.getNetworkGasPrices("gaia");
    const priceStr = `${GAIA_GAS_PRICE.amount}${GAIA_GAS_PRICE.denom}`;
    expect(price).toBe(priceStr);
  });

  it("should get the wallet mnemonics", () => {
    const mnemonics = suite.getWalletMnemonics();
    expect(Object.keys(mnemonics).length).toBe(WALLET_KEYS.length - 3);

    for (const [_, mnemonic] of Object.entries(mnemonics)) {
      expect(mnemonic.split(" ").length).toBe(WALLET_MNEMONIC_WORD_COUNT);
    }
  });

  it("should pause the ICQ relayer", async () => {
    await suite.pauseIcqRelaying();
    const isPaused = await isContainerPaused("default-relayer_neutron1-1");
    expect(isPaused).toBe(true);
  });

  it("should resume the ICQ relayer", async () => {
    await suite.resumeIcqRelaying();
    const isPaused = await isContainerPaused("default-relayer_neutron1-1");
    expect(isPaused).toBe(false);
  });

  it("should allow connections to the neutron RPC", async () => {
    const rpc = suite.getNetworkRpc("neutron");
    const client = await StargateClient.connect(`http://${rpc}`);
    const block = await client.getBlock();
    expect(block.header.height).toBeGreaterThan(0);
  });

  it("should allow connections to the gaia RPC", async () => {
    const rpc = suite.getNetworkRpc("gaia");
    const client = await StargateClient.connect(`http://${rpc}`);
    const block = await client.getBlock();
    expect(block.header.height).toBeGreaterThan(0);
  });

  it("should have the specified number of validators", async () => {
    const rpc = suite.getNetworkRpc("gaia");
    const tmClient = await Tendermint37Client.connect(`http://${rpc}`);
    const queryClient = QueryClient.withExtensions(
      tmClient,
      setupStakingExtension,
    );

    const bondedValidators =
      await queryClient.staking.validators("BOND_STATUS_BONDED");
    const unbondedValidators = await queryClient.staking.validators(
      "BOND_STATUS_UNBONDED",
    );
    const unbondingValidators = await queryClient.staking.validators(
      "BOND_STATUS_UNBONDING",
    );

    const totalValidatorCount =
      bondedValidators.validators.length +
      unbondedValidators.validators.length +
      unbondingValidators.validators.length;

    expect(totalValidatorCount).toBe(6);
  });
});
