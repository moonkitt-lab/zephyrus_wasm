import Decimal from "decimal.js";
export interface Vessel {
    id: number;
    lock_duration_months: number;
    locked_denom: string;
    locked_amount: string;
    controlled_by: string;
    voted_proposal_id: number | null;
}
export interface User {
    user_id: string;
    vessels: Vessel[];
}
export interface Tribute {
    id: number;
    denom: string;
    amount: string;
}
export interface Proposal {
    id: number;
    bid_duration_months: number;
    tributes: Tribute[];
}
export interface ProtocolConfig {
    protocol_commission_bps: number;
    hydromancer_commission_bps: number;
}
export interface Scenario {
    protocol_config: ProtocolConfig;
    users: User[];
    proposals: Proposal[];
}
export interface VesselWithUser extends Vessel {
    user_id: string;
}
export interface RewardsResult {
    protocol_rewards: {
        [denom: string]: string;
    };
    hydromancer_rewards: {
        [denom: string]: string;
    };
    user_rewards: {
        [userId: string]: {
            [denom: string]: string;
        };
    };
}
export declare class RewardsCalculator {
    private tokenMultipliers;
    private durationMultipliers;
    constructor();
    calculateVotingPower(vessel: Vessel): Decimal;
    getVesselsByProposal(scenario: Scenario): Map<number, VesselWithUser[]>;
    calculateProtocolRewards(scenario: Scenario): {
        [denom: string]: Decimal;
    };
    calculateHydromancerVotingPowerByProposal(scenario: Scenario): Map<number, Decimal>;
    calculateTotalVotingPowerByProposal(scenario: Scenario): Map<number, Decimal>;
    calculateHydromancerRewards(scenario: Scenario): {
        [denom: string]: Decimal;
    };
    calculateUserDirectRewards(scenario: Scenario): {
        [userId: string]: {
            [denom: string]: Decimal;
        };
    };
    calculateUserDelegatedRewards(scenario: Scenario): {
        [userId: string]: {
            [denom: string]: Decimal;
        };
    };
    calculateAllRewards(scenario: Scenario): RewardsResult;
}
//# sourceMappingURL=calculate-rewards.d.ts.map