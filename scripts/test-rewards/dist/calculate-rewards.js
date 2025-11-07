"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.RewardsCalculator = void 0;
const decimal_js_1 = __importDefault(require("decimal.js"));
// Set precision for Decimal calculations
decimal_js_1.default.set({ precision: 50, rounding: decimal_js_1.default.ROUND_HALF_UP });
class RewardsCalculator {
    constructor() {
        this.tokenMultipliers = {
            "dATOM": new decimal_js_1.default("1.15"),
            "stATOM": new decimal_js_1.default("1.6"),
        };
        this.durationMultipliers = {
            1: new decimal_js_1.default("1.0"),
            2: new decimal_js_1.default("1.25"),
            3: new decimal_js_1.default("1.5")
        };
    }
    calculateVotingPower(vessel) {
        const amount = new decimal_js_1.default(vessel.locked_amount);
        const tokenDenom = vessel.locked_denom;
        const lockDuration = vessel.lock_duration_months;
        // Apply token multiplier (should always exist since vessels only contain stATOM/dATOM)
        if (!(tokenDenom in this.tokenMultipliers)) {
            throw new Error(`Invalid vessel token: ${tokenDenom}. Vessels can only contain stATOM or dATOM.`);
        }
        const tokenMultiplier = this.tokenMultipliers[tokenDenom];
        const durationMultiplier = this.durationMultipliers[lockDuration];
        const votingPower = amount.mul(tokenMultiplier).mul(durationMultiplier);
        return votingPower;
    }
    getVesselsByProposal(scenario) {
        const vesselsByProposal = new Map();
        for (const user of scenario.users) {
            for (const vessel of user.vessels) {
                if (vessel.voted_proposal_id !== null) {
                    const vesselWithUser = {
                        ...vessel,
                        user_id: user.user_id
                    };
                    if (!vesselsByProposal.has(vessel.voted_proposal_id)) {
                        vesselsByProposal.set(vessel.voted_proposal_id, []);
                    }
                    vesselsByProposal.get(vessel.voted_proposal_id).push(vesselWithUser);
                }
            }
        }
        return vesselsByProposal;
    }
    calculateProtocolRewards(scenario) {
        const protocolCommissionRate = new decimal_js_1.default(scenario.protocol_config.protocol_commission_bps).div(new decimal_js_1.default("10000"));
        const protocolRewards = new Map();
        const vesselsByProposal = this.getVesselsByProposal(scenario);
        const activeProposalIds = new Set(vesselsByProposal.keys());
        for (const proposal of scenario.proposals) {
            if (activeProposalIds.has(proposal.id)) {
                for (const tribute of proposal.tributes) {
                    const tributeAmount = new decimal_js_1.default(tribute.amount);
                    const commission = tributeAmount.mul(protocolCommissionRate);
                    const currentAmount = protocolRewards.get(tribute.denom) || new decimal_js_1.default("0");
                    protocolRewards.set(tribute.denom, currentAmount.add(commission));
                }
            }
        }
        const result = {};
        for (const [denom, amount] of protocolRewards) {
            result[denom] = amount;
        }
        return result;
    }
    calculateHydromancerVotingPowerByProposal(scenario) {
        const vesselsByProposal = this.getVesselsByProposal(scenario);
        const hydromancerPowerByProposal = new Map();
        for (const [proposalId, vessels] of vesselsByProposal) {
            let hydromancerPower = new decimal_js_1.default("0");
            for (const vessel of vessels) {
                if (vessel.controlled_by === "hydromancer") {
                    hydromancerPower = hydromancerPower.add(this.calculateVotingPower(vessel));
                }
            }
            hydromancerPowerByProposal.set(proposalId, hydromancerPower);
        }
        return hydromancerPowerByProposal;
    }
    calculateTotalVotingPowerByProposal(scenario) {
        const vesselsByProposal = this.getVesselsByProposal(scenario);
        const totalPowerByProposal = new Map();
        for (const [proposalId, vessels] of vesselsByProposal) {
            let totalPower = new decimal_js_1.default("0");
            for (const vessel of vessels) {
                totalPower = totalPower.add(this.calculateVotingPower(vessel));
            }
            totalPowerByProposal.set(proposalId, totalPower);
        }
        return totalPowerByProposal;
    }
    calculateHydromancerRewards(scenario) {
        const hydromancerCommissionRate = new decimal_js_1.default(scenario.protocol_config.hydromancer_commission_bps).div(new decimal_js_1.default("10000"));
        const protocolCommissionRate = new decimal_js_1.default(scenario.protocol_config.protocol_commission_bps).div(new decimal_js_1.default("10000"));
        const hydromancerRewards = new Map();
        const hydromancerPowerByProposal = this.calculateHydromancerVotingPowerByProposal(scenario);
        const totalPowerByProposal = this.calculateTotalVotingPowerByProposal(scenario);
        for (const proposal of scenario.proposals) {
            const proposalId = proposal.id;
            if (hydromancerPowerByProposal.has(proposalId) &&
                hydromancerPowerByProposal.get(proposalId).gt(0)) {
                const hydromancerPower = hydromancerPowerByProposal.get(proposalId);
                const totalPower = totalPowerByProposal.get(proposalId);
                const hydromancerShare = hydromancerPower.div(totalPower);
                for (const tribute of proposal.tributes) {
                    const tributeAmount = new decimal_js_1.default(tribute.amount);
                    // Remove protocol commission first
                    const afterProtocolCommission = tributeAmount.mul(new decimal_js_1.default("1").sub(protocolCommissionRate));
                    // Hydromancer gets their share
                    const hydromancerTributeShare = afterProtocolCommission.mul(hydromancerShare);
                    // Hydromancer takes commission from their share
                    const hydromancerCommission = hydromancerTributeShare.mul(hydromancerCommissionRate);
                    const currentAmount = hydromancerRewards.get(tribute.denom) || new decimal_js_1.default("0");
                    hydromancerRewards.set(tribute.denom, currentAmount.add(hydromancerCommission));
                }
            }
        }
        const result = {};
        for (const [denom, amount] of hydromancerRewards) {
            result[denom] = amount;
        }
        return result;
    }
    calculateUserDirectRewards(scenario) {
        const protocolCommissionRate = new decimal_js_1.default(scenario.protocol_config.protocol_commission_bps).div(new decimal_js_1.default("10000"));
        const userRewards = new Map();
        const vesselsByProposal = this.getVesselsByProposal(scenario);
        const totalPowerByProposal = this.calculateTotalVotingPowerByProposal(scenario);
        for (const proposal of scenario.proposals) {
            const proposalId = proposal.id;
            if (vesselsByProposal.has(proposalId)) {
                const totalPower = totalPowerByProposal.get(proposalId);
                const userVessels = vesselsByProposal.get(proposalId).filter(v => v.controlled_by === "user");
                for (const vessel of userVessels) {
                    const userId = vessel.user_id;
                    const vesselPower = this.calculateVotingPower(vessel);
                    const userShare = vesselPower.div(totalPower);
                    if (!userRewards.has(userId)) {
                        userRewards.set(userId, new Map());
                    }
                    const userRewardMap = userRewards.get(userId);
                    for (const tribute of proposal.tributes) {
                        const tributeAmount = new decimal_js_1.default(tribute.amount);
                        // Remove protocol commission
                        const afterProtocolCommission = tributeAmount.mul(new decimal_js_1.default("1").sub(protocolCommissionRate));
                        const userReward = afterProtocolCommission.mul(userShare);
                        const currentAmount = userRewardMap.get(tribute.denom) || new decimal_js_1.default("0");
                        userRewardMap.set(tribute.denom, currentAmount.add(userReward));
                    }
                }
            }
        }
        const result = {};
        for (const [userId, rewardMap] of userRewards) {
            result[userId] = {};
            for (const [denom, amount] of rewardMap) {
                result[userId][denom] = amount;
            }
        }
        return result;
    }
    calculateUserDelegatedRewards(scenario) {
        const protocolCommissionRate = new decimal_js_1.default(scenario.protocol_config.protocol_commission_bps).div(new decimal_js_1.default("10000"));
        const hydromancerCommissionRate = new decimal_js_1.default(scenario.protocol_config.hydromancer_commission_bps).div(new decimal_js_1.default("10000"));
        const userRewards = new Map();
        // Get all vessels controlled by hydromancer, grouped by user and eligible proposal duration
        const userVesselsByDuration = new Map();
        for (const user of scenario.users) {
            for (const vessel of user.vessels) {
                if (vessel.controlled_by === "hydromancer") {
                    const vesselWithUser = {
                        ...vessel,
                        user_id: user.user_id
                    };
                    if (!userVesselsByDuration.has(user.user_id)) {
                        userVesselsByDuration.set(user.user_id, new Map());
                    }
                    const userMap = userVesselsByDuration.get(user.user_id);
                    if (!userMap.has(vessel.lock_duration_months)) {
                        userMap.set(vessel.lock_duration_months, []);
                    }
                    userMap.get(vessel.lock_duration_months).push(vesselWithUser);
                }
            }
        }
        // For each proposal, calculate rewards that should be shared among delegated users
        for (const proposal of scenario.proposals) {
            const proposalDuration = proposal.bid_duration_months;
            const proposalId = proposal.id;
            // Find hydromancer vessels that voted for this proposal
            const vesselsByProposal = this.getVesselsByProposal(scenario);
            if (!vesselsByProposal.has(proposalId)) {
                continue;
            }
            const hydromancerVesselsForProposal = vesselsByProposal.get(proposalId).filter(v => v.controlled_by === "hydromancer");
            if (hydromancerVesselsForProposal.length === 0) {
                continue;
            }
            // Calculate total hydromancer voting power for this proposal
            const totalHydromancerPowerForProposal = hydromancerVesselsForProposal.reduce((sum, vessel) => sum.add(this.calculateVotingPower(vessel)), new decimal_js_1.default("0"));
            // Calculate hydromancer's share of total voting power
            const totalPowerByProposal = this.calculateTotalVotingPowerByProposal(scenario);
            const totalPower = totalPowerByProposal.get(proposalId);
            const hydromancerShare = totalHydromancerPowerForProposal.div(totalPower);
            // Calculate eligible user voting power for this proposal duration
            const eligibleUserPower = new Map();
            let totalEligiblePower = new decimal_js_1.default("0");
            for (const [userId, vesselsByDuration] of userVesselsByDuration) {
                let userPower = new decimal_js_1.default("0");
                // Users can participate if their vessel duration >= proposal duration
                for (const [duration, vessels] of vesselsByDuration) {
                    if (duration >= proposalDuration) {
                        for (const vessel of vessels) {
                            userPower = userPower.add(this.calculateVotingPower(vessel));
                        }
                    }
                }
                if (userPower.gt(0)) {
                    eligibleUserPower.set(userId, userPower);
                    totalEligiblePower = totalEligiblePower.add(userPower);
                }
            }
            if (totalEligiblePower.gt(0)) {
                // Distribute hydromancer's tribute share among eligible users
                for (const tribute of proposal.tributes) {
                    const tributeAmount = new decimal_js_1.default(tribute.amount);
                    // Remove protocol commission
                    const afterProtocolCommission = tributeAmount.mul(new decimal_js_1.default("1").sub(protocolCommissionRate));
                    // Get hydromancer's share
                    const hydromancerTributeShare = afterProtocolCommission.mul(hydromancerShare);
                    // Remove hydromancer commission
                    const afterHydromancerCommission = hydromancerTributeShare.mul(new decimal_js_1.default("1").sub(hydromancerCommissionRate));
                    // Distribute among eligible users based on their voting power
                    for (const [userId, userPower] of eligibleUserPower) {
                        const userShare = userPower.div(totalEligiblePower);
                        const userReward = afterHydromancerCommission.mul(userShare);
                        if (!userRewards.has(userId)) {
                            userRewards.set(userId, new Map());
                        }
                        const userRewardMap = userRewards.get(userId);
                        const currentAmount = userRewardMap.get(tribute.denom) || new decimal_js_1.default("0");
                        userRewardMap.set(tribute.denom, currentAmount.add(userReward));
                    }
                }
            }
        }
        const result = {};
        for (const [userId, rewardMap] of userRewards) {
            result[userId] = {};
            for (const [denom, amount] of rewardMap) {
                result[userId][denom] = amount;
            }
        }
        return result;
    }
    calculateAllRewards(scenario) {
        // Calculate protocol rewards
        const protocolRewards = this.calculateProtocolRewards(scenario);
        // Calculate hydromancer rewards
        const hydromancerRewards = this.calculateHydromancerRewards(scenario);
        // Calculate user direct rewards
        const userDirectRewards = this.calculateUserDirectRewards(scenario);
        // Calculate user delegated rewards
        const userDelegatedRewards = this.calculateUserDelegatedRewards(scenario);
        // Combine user rewards
        const allUserRewards = new Map();
        // Add direct rewards
        for (const [userId, rewards] of Object.entries(userDirectRewards)) {
            if (!allUserRewards.has(userId)) {
                allUserRewards.set(userId, new Map());
            }
            const userRewardMap = allUserRewards.get(userId);
            for (const [denom, amount] of Object.entries(rewards)) {
                const currentAmount = userRewardMap.get(denom) || new decimal_js_1.default("0");
                userRewardMap.set(denom, currentAmount.add(amount));
            }
        }
        // Add delegated rewards
        for (const [userId, rewards] of Object.entries(userDelegatedRewards)) {
            if (!allUserRewards.has(userId)) {
                allUserRewards.set(userId, new Map());
            }
            const userRewardMap = allUserRewards.get(userId);
            for (const [denom, amount] of Object.entries(rewards)) {
                const currentAmount = userRewardMap.get(denom) || new decimal_js_1.default("0");
                userRewardMap.set(denom, currentAmount.add(amount));
            }
        }
        // Convert to result format with string amounts for JSON serialization
        const finalProtocolRewards = {};
        for (const [denom, amount] of Object.entries(protocolRewards)) {
            finalProtocolRewards[denom] = amount.toFixed(2);
        }
        const finalHydromancerRewards = {};
        for (const [denom, amount] of Object.entries(hydromancerRewards)) {
            finalHydromancerRewards[denom] = amount.toFixed(2);
        }
        const finalUserRewards = {};
        for (const [userId, rewardMap] of allUserRewards) {
            finalUserRewards[userId] = {};
            for (const [denom, amount] of rewardMap) {
                finalUserRewards[userId][denom] = amount.toFixed(2);
            }
        }
        return {
            protocol_rewards: finalProtocolRewards,
            hydromancer_rewards: finalHydromancerRewards,
            user_rewards: finalUserRewards
        };
    }
}
exports.RewardsCalculator = RewardsCalculator;
//# sourceMappingURL=calculate-rewards.js.map