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
        DNTRD: string;
        STATOM: string;
        STOSMO: string;
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
export declare const CONFIG: TestConfig;
export declare function getTokenDenom(symbol: string): string;
export declare function getTokenSymbol(denom: string): string;
//# sourceMappingURL=config.d.ts.map