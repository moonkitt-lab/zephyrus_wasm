"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CONFIG = void 0;
exports.getTokenDenom = getTokenDenom;
exports.getTokenSymbol = getTokenSymbol;
exports.CONFIG = {
    deployerMnemonic: "appear empty thrive panther spread mandate together possible hawk area delay artefact hockey endorse assist blood grid cheap argue capable diamond bonus abstract quarter",
    chainId: "neutron-devnet-1",
    rpcEndpoint: "http://localhost:26657",
    restEndpoint: "http://localhost:1317",
    gasPrice: "0.0053untrn",
    gasAdjustment: 1.3,
    tokenDenoms: {
        NTRN: "untrn",
        DATOM: "factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom",
        DNTRD: "factory/neutron1frc0p5czd9uaaymdkug2njz7dc7j65jxukp9apmt9260a8egujkspms2t2/udntrn",
        STATOM: "ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C",
        STOSMO: "ibc/75249A18DEFBEFE55F83B1C70CAD234DF164F174C6BC51682EE92C2C81C18C93"
    },
    // These will be loaded from deploy_scripts config files at runtime
    contractAddresses: {
        hydro: "",
        tribute: "",
        zephyrus: ""
    },
    rewardsConfig: {
        protocolCommissionBps: 1000, // 10%
        hydromancerCommissionBps: 500, // 5%
        tokenMultipliers: {
            "dATOM": 1.15,
            "stATOM": 1.6
        },
        durationMultipliers: {
            1: 1.0,
            2: 1.25,
            3: 1.5
        }
    }
};
function getTokenDenom(symbol) {
    const symbolUpper = symbol.toUpperCase();
    if (symbolUpper === "DATOM")
        return exports.CONFIG.tokenDenoms.DATOM;
    if (symbolUpper === "STATOM")
        return exports.CONFIG.tokenDenoms.STATOM;
    if (symbolUpper === "NTRN")
        return exports.CONFIG.tokenDenoms.NTRN;
    if (symbolUpper === "USDC")
        return "uusdc"; // placeholder
    throw new Error(`Unknown token symbol: ${symbol}`);
}
function getTokenSymbol(denom) {
    if (denom === exports.CONFIG.tokenDenoms.DATOM)
        return "dATOM";
    if (denom === exports.CONFIG.tokenDenoms.STATOM)
        return "stATOM";
    if (denom === exports.CONFIG.tokenDenoms.NTRN)
        return "NTRN";
    if (denom === "uusdc")
        return "USDC";
    throw new Error(`Unknown token denom: ${denom}`);
}
//# sourceMappingURL=config.js.map