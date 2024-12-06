import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./test-suite";
import { HydroContracts, NeutronWallet } from "./test-helpers";

let suite: TestSuite;
let ntrnWallet: NeutronWallet;
let hydroContracts: HydroContracts;

describe("Hydro contracts sanity check", () => {
  beforeAll(async () => {
    console.log("intializing test suite...");
    suite = await TestSuite.create({
      networkOverrides: {
        gaia: {
          validators: 2,
          validators_balance: ["100000000", "100000000"],
        },
      },
    });
    ntrnWallet = await NeutronWallet.connect(suite, "demo1");
  });

  afterAll(async () => {
    await suite.cleanup();
  });

  it("should deploy the hydro contracts", async () => {
    console.log("deploying hydro contracts...");

    hydroContracts = await ntrnWallet.deployHydro({
      admin: await ntrnWallet.address(),
      roundLengthSecs: 60,
    });

    const hydroConstantsRes = await ntrnWallet.queryWasm(hydroContracts.hydro, {
      constants: {},
    });

    expect(hydroConstantsRes.constants.round_length).toBe(60 * 10 ** 9);

    const tributeConfigRes = await ntrnWallet.queryWasm(
      hydroContracts.tribute,
      {
        config: {},
      },
    );

    expect(tributeConfigRes.config.hydro_contract).toBe(hydroContracts.hydro);
  });

  it("should add a proposal to hydro", async () => {
    console.log("adding a proposal to hydro...");

    await ntrnWallet.createHydroProposal(hydroContracts, {
      trancheId: 1,
      title: "Test Proposal",
      description: "Testing...",
      deploymentDuration: 1,
      minAtomLiquidityRequest: 1_000_000,
    });

    const roundProposalRes = await ntrnWallet.queryWasm(hydroContracts.hydro, {
      round_proposals: { round_id: 0, tranche_id: 1, start_from: 0, limit: 10 },
    });

    expect(roundProposalRes.proposals.length).toBe(1);
    expect(roundProposalRes.proposals[0].title).toBe("Test Proposal");
    expect(roundProposalRes.proposals[0].proposal_id).toBe(0);
  });

  it("should add a tribute to hydro", async () => {
    console.log("adding a tribute to hydro...");

    await ntrnWallet.addHydroTribute(hydroContracts, {
      amount: 1_000_000,
      denom: "untrn",
      trancheId: 1,
      proposalId: 0,
      roundId: 0,
    });

    const roundTributesRes = await ntrnWallet.queryWasm(
      hydroContracts.tribute,
      {
        round_tributes: {
          round_id: 0,
          start_from: 0,
          limit: 10,
        },
      },
    );

    expect(roundTributesRes.tributes.length).toBe(1);
    expect(roundTributesRes.tributes[0].funds.amount).toBe("1000000");
  });
});
