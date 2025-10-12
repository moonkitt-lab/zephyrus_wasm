export interface TestConfig {
  deployerMnemonic: string;
  chainId: string;
  rpcEndpoint: string;
  restEndpoint: string;
  gasPrice: string;
  gasAdjustment: number;
  tokenDenoms: {
    NTRN: string;
    DATOM: string;
    DNTRN: string;
    STATOM: string;
    STOSMO: string;
    USDC: string;
  };
  contractAddresses: {
    hydro: string;
    tribute: string;
    zephyrus: string;
  };
  rewardsConfig: {
    protocolCommissionBps: number;
    hydromancerCommissionBps: number;
    tokenMultipliers: {
      [key: string]: number;
    };
    durationMultipliers: {
      [key: number]: number;
    };
  };
}

export const CONFIG: TestConfig = {
  deployerMnemonic:
    "appear empty thrive panther spread mandate together possible hawk area delay artefact hockey endorse assist blood grid cheap argue capable diamond bonus abstract quarter",

  chainId: "neutron-devnet-1",
  rpcEndpoint: "http://localhost:26657",
  restEndpoint: "http://localhost:1317",
  gasPrice: "0.0053untrn",
  gasAdjustment: 1.3,

  tokenDenoms: {
    NTRN: "untrn",
    DATOM:
      "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
    DNTRN:
      "factory/neutron1frc0p5czd9uaaymdkug2njz7dc7j65jxukp9apmt9260a8egujkspms2t2/udntrn",
    STATOM:
      "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
    STOSMO:
      "ibc/75249A18DEFBEFE55F83B1C70CAD234DF164F174C6BC51682EE92C2C81C18C93",
    USDC: "ibc/B559A80D62249C8AA07A380E2A2BEA6E5CA9A6F079C912C3A9E9B494105E4F81",
  },

  // These will be loaded from deploy_scripts config files at runtime
  contractAddresses: {
    hydro: "",
    tribute: "",
    zephyrus: "",
  },

  rewardsConfig: {
    protocolCommissionBps: 1000, // 10%
    hydromancerCommissionBps: 500, // 5%
    tokenMultipliers: {
      dATOM: 1.3,
      stATOM: 1.6,
    },
    durationMultipliers: {
      1: 1.0,
      2: 1.25,
      3: 1.5,
    },
  },
};

export function getTokenDenom(symbol: string): string {
  const symbolUpper = symbol.toUpperCase();
  if (symbolUpper === "DATOM") return CONFIG.tokenDenoms.DATOM;
  if (symbolUpper === "STATOM") return CONFIG.tokenDenoms.STATOM;
  if (symbolUpper === "NTRN") return CONFIG.tokenDenoms.NTRN;
  if (symbolUpper === "USDC") return CONFIG.tokenDenoms.USDC;
  throw new Error(`Unknown token symbol: ${symbol}`);
}

export function getTokenSymbol(denom: string): string {
  if (denom === CONFIG.tokenDenoms.DATOM) return "dATOM";
  if (denom === CONFIG.tokenDenoms.STATOM) return "stATOM";
  if (denom === CONFIG.tokenDenoms.NTRN) return "NTRN";
  if (denom === CONFIG.tokenDenoms.USDC) return "USDC";
  throw new Error(`Unknown token denom: ${denom}`);
}
