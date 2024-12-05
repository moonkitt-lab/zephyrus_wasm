export const GAIA_GAS_PRICE = {
  amount: "0.03",
  denom: "stake",
};

export const NEUTRON_GAS_PRICE = {
  amount: "0.03",
  denom: "untrn",
};

export const GAS_PRICES = {
  neutron: NEUTRON_GAS_PRICE,
  gaia: GAIA_GAS_PRICE,
};

export const WALLET_KEYS = [
  "master",
  "hermes",
  "neutronqueryrelayer",
  "demowallet1",
  "demo1",
  "demo2",
  "demo3",
  "relayer_0",
  "relayer_1",
] as const;

export const WALLET_MNEMONIC_WORD_COUNT = 12;

export const GENESIS_ALLOCATION = 1_000_000_000_000;
