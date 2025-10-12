import Decimal from "decimal.js";
import { CONFIG } from "./config";
import { TestLogger } from "./test-utils";

// Set precision for Decimal calculations
Decimal.set({ precision: 50, rounding: Decimal.ROUND_HALF_UP });

export interface VesselRound {
  round_id: number;
  controlled_by: string;
  voted_proposal_id: number | null;
  refresh: boolean;
}

export interface Vessel {
  id: number;
  lock_duration_rounds: number;
  locked_denom: string;
  locked_amount: string;
  rounds: VesselRound[];
}

export interface User {
  user_id: string;
  address: string;
  vessels: Vessel[];
}

export interface Tribute {
  id: number;
  denom: string;
  amount: string;
}

export interface Proposal {
  id: number;
  round_id: number;
  bid_duration_months: number;
  tributes: Tribute[];
}

export interface ProtocolConfig {
  protocol_commission_bps: number;
  hydromancer_commission_bps: number;
  round_length: number;
  total_rounds: number;
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
  protocol_rewards: { [denom: string]: string };
  hydromancer_rewards: { [denom: string]: string };
  user_rewards: { [userId: string]: { [denom: string]: string } };
}

export class RewardsCalculator {
  private tokenMultipliers: { [key: string]: Decimal };
  private durationMultipliers: { [key: number]: Decimal };

  constructor() {
    this.tokenMultipliers = {
      dATOM: new Decimal("1.3"),
      stATOM: new Decimal("1.6"),
    };

    this.durationMultipliers = {
      1: new Decimal("1.0"),
      2: new Decimal("1.25"),
      3: new Decimal("1.5"),
    };
  }

  calculateDurationMultiplier(lockDuration: number): Decimal {
    // Find the largest durationMultiplier where the key is <= lockDuration
    let durationMultiplier = new Decimal("1.0"); // default fallback
    let maxValidDuration = 0;
    for (const [duration, multiplier] of Object.entries(
      this.durationMultipliers
    )) {
      const durationNum = parseInt(duration);
      if (durationNum <= lockDuration && durationNum > maxValidDuration) {
        maxValidDuration = durationNum;
        durationMultiplier = multiplier;
      }
    }
    return durationMultiplier;
  }

  getLockDuration(vessel: Vessel, roundId: number): number {
    let lockDuration = vessel.lock_duration_rounds;
    for (const round of vessel.rounds) {
      if (round.round_id <= roundId) {
        if (!round.refresh && roundId > 0) {
          lockDuration--;
        } else {
          lockDuration = vessel.lock_duration_rounds;
        }
      }
    }
    return lockDuration;
  }

  calculateVotingPower(
    roundId: number,
    vessel: Vessel,
    logger: TestLogger
  ): Decimal {
    const amount = new Decimal(vessel.locked_amount);
    const tokenDenom = vessel.locked_denom;
    let lockDuration = this.getLockDuration(vessel, roundId);
    if (lockDuration <= 0) {
      return new Decimal("0");
    }
    // Apply token multiplier (should always exist since vessels only contain stATOM/dATOM)
    if (!(tokenDenom in this.tokenMultipliers)) {
      throw new Error(
        `Invalid vessel token: ${tokenDenom}. Vessels can only contain stATOM or dATOM.`
      );
    }

    const tokenMultiplier = this.tokenMultipliers[tokenDenom];
    const durationMultiplier = this.calculateDurationMultiplier(lockDuration);

    const votingPower = amount.mul(tokenMultiplier).mul(durationMultiplier);
    logger.info(
      `Duration multiplier for vessel ${vessel.id} and round ${roundId}: ${durationMultiplier.toString()}, token multiplier: ${tokenMultiplier.toString()} vp : ${votingPower.toString()}`
    );
    return votingPower;
  }

  getVesselsByProposal(
    scenario: Scenario,
    roundId: number
  ): Map<number, VesselWithUser[]> {
    const vesselsByProposal = new Map<number, VesselWithUser[]>();

    for (const user of scenario.users) {
      for (const vessel of user.vessels) {
        for (const round of vessel.rounds) {
          if (round.round_id === roundId) {
            if (round.voted_proposal_id !== null) {
              const vesselWithUser: VesselWithUser = {
                ...vessel,
                user_id: user.user_id,
              };

              if (!vesselsByProposal.has(round.voted_proposal_id)) {
                vesselsByProposal.set(round.voted_proposal_id, []);
              }
              vesselsByProposal
                .get(round.voted_proposal_id)!
                .push(vesselWithUser);
            }
          }
        }
      }
    }

    return vesselsByProposal;
  }

  calculateProtocolRewards(
    scenario: Scenario,
    roundId: number
  ): { [denom: string]: Decimal } {
    const protocolCommissionRate = new Decimal(
      scenario.protocol_config.protocol_commission_bps
    ).div(new Decimal("10000"));
    const protocolRewards = new Map<string, Decimal>();

    const vesselsByProposal = this.getVesselsByProposal(scenario, roundId);
    const activeProposalIds = new Set(vesselsByProposal.keys());

    for (const proposal of scenario.proposals) {
      if (activeProposalIds.has(proposal.id)) {
        for (const tribute of proposal.tributes) {
          const tributeAmount = new Decimal(tribute.amount);
          const commission = tributeAmount.mul(protocolCommissionRate);

          const currentAmount =
            protocolRewards.get(tribute.denom) || new Decimal("0");
          protocolRewards.set(tribute.denom, currentAmount.add(commission));
        }
      }
    }

    const result: { [denom: string]: Decimal } = {};
    for (const [denom, amount] of protocolRewards) {
      result[denom] = amount;
    }

    return result;
  }

  calculateHydromancerVotingPowerByProposal(
    scenario: Scenario,
    roundId: number,
    logger: TestLogger
  ): Map<number, Decimal> {
    const vesselsByProposal = this.getVesselsByProposal(scenario, roundId);
    const hydromancerPowerByProposal = new Map<number, Decimal>();

    for (const [proposalId, vessels] of vesselsByProposal) {
      let hydromancerPower = new Decimal("0");
      for (const vessel of vessels) {
        for (const round of vessel.rounds) {
          if (round.round_id === roundId) {
            if (round.controlled_by === "hydromancer") {
              hydromancerPower = hydromancerPower.add(
                this.calculateVotingPower(roundId, vessel, logger)
              );
            }
          }
        }
      }
      hydromancerPowerByProposal.set(proposalId, hydromancerPower);
    }

    return hydromancerPowerByProposal;
  }

  calculateTotalVotingPowerByProposal(
    scenario: Scenario,
    roundId: number,
    logger: TestLogger
  ): Map<number, Decimal> {
    const vesselsByProposal = this.getVesselsByProposal(scenario, roundId);
    const totalPowerByProposal = new Map<number, Decimal>();

    for (const [proposalId, vessels] of vesselsByProposal) {
      let totalPower = new Decimal("0");
      for (const vessel of vessels) {
        totalPower = totalPower.add(
          this.calculateVotingPower(roundId, vessel, logger)
        );
      }
      totalPowerByProposal.set(proposalId, totalPower);
    }

    return totalPowerByProposal;
  }

  calculateHydromancerRewards(
    scenario: Scenario,
    roundId: number,
    logger: TestLogger
  ): {
    [denom: string]: Decimal;
  } {
    const hydromancerCommissionRate = new Decimal(
      scenario.protocol_config.hydromancer_commission_bps
    ).div(new Decimal("10000"));
    const protocolCommissionRate = new Decimal(
      scenario.protocol_config.protocol_commission_bps
    ).div(new Decimal("10000"));

    const hydromancerRewards = new Map<string, Decimal>();
    const hydromancerPowerByProposal =
      this.calculateHydromancerVotingPowerByProposal(scenario, roundId, logger);
    const totalPowerByProposal = this.calculateTotalVotingPowerByProposal(
      scenario,
      roundId,
      logger
    );

    for (const proposal of scenario.proposals) {
      const proposalId = proposal.id;

      if (
        hydromancerPowerByProposal.has(proposalId) &&
        hydromancerPowerByProposal.get(proposalId)!.gt(0)
      ) {
        const hydromancerPower = hydromancerPowerByProposal.get(proposalId)!;
        const totalPower = totalPowerByProposal.get(proposalId)!;
        const hydromancerShare = hydromancerPower.div(totalPower);

        for (const tribute of proposal.tributes) {
          const tributeAmount = new Decimal(tribute.amount);
          // Remove protocol commission first
          const afterProtocolCommission = tributeAmount.mul(
            new Decimal("1").sub(protocolCommissionRate)
          );
          // Hydromancer gets their share
          const hydromancerTributeShare =
            afterProtocolCommission.mul(hydromancerShare);
          // Hydromancer takes commission from their share
          const hydromancerCommission = hydromancerTributeShare.mul(
            hydromancerCommissionRate
          );

          const currentAmount =
            hydromancerRewards.get(tribute.denom) || new Decimal("0");
          hydromancerRewards.set(
            tribute.denom,
            currentAmount.add(hydromancerCommission)
          );
        }
      }
    }

    const result: { [denom: string]: Decimal } = {};
    for (const [denom, amount] of hydromancerRewards) {
      result[denom] = amount;
    }

    return result;
  }

  calculateUserDirectRewards(
    scenario: Scenario,
    roundId: number,
    logger: TestLogger
  ): {
    [userId: string]: { [denom: string]: Decimal };
  } {
    const protocolCommissionRate = new Decimal(
      scenario.protocol_config.protocol_commission_bps
    ).div(new Decimal("10000"));
    const userRewards = new Map<string, Map<string, Decimal>>();

    const vesselsByProposal = this.getVesselsByProposal(scenario, roundId);
    const totalPowerByProposal = this.calculateTotalVotingPowerByProposal(
      scenario,
      roundId,
      logger
    );

    for (const proposal of scenario.proposals) {
      const proposalId = proposal.id;

      if (vesselsByProposal.has(proposalId)) {
        const totalPower = totalPowerByProposal.get(proposalId)!;
        const userVessels = vesselsByProposal
          .get(proposalId)!
          .filter((v) =>
            v.rounds.some(
              (r) => r.round_id === roundId && r.controlled_by === "user"
            )
          );

        for (const vessel of userVessels) {
          const userId = vessel.user_id;
          const vesselPower = this.calculateVotingPower(
            roundId,
            vessel,
            logger
          );
          const userShare = vesselPower.div(totalPower);

          if (!userRewards.has(userId)) {
            userRewards.set(userId, new Map<string, Decimal>());
          }
          const userRewardMap = userRewards.get(userId)!;

          for (const tribute of proposal.tributes) {
            const tributeAmount = new Decimal(tribute.amount);
            // Remove protocol commission
            const afterProtocolCommission = tributeAmount.mul(
              new Decimal("1").sub(protocolCommissionRate)
            );
            const userReward = afterProtocolCommission.mul(userShare);

            const currentAmount =
              userRewardMap.get(tribute.denom) || new Decimal("0");
            userRewardMap.set(tribute.denom, currentAmount.add(userReward));
          }
        }
      }
    }

    const result: { [userId: string]: { [denom: string]: Decimal } } = {};
    for (const [userId, rewardMap] of userRewards) {
      result[userId] = {};
      for (const [denom, amount] of rewardMap) {
        result[userId][denom] = amount;
      }
    }

    return result;
  }

  calculateUserDelegatedRewards(
    scenario: Scenario,
    roundId: number,
    logger: TestLogger
  ): {
    [userId: string]: { [denom: string]: Decimal };
  } {
    const protocolCommissionRate = new Decimal(
      scenario.protocol_config.protocol_commission_bps
    ).div(new Decimal("10000"));
    const hydromancerCommissionRate = new Decimal(
      scenario.protocol_config.hydromancer_commission_bps
    ).div(new Decimal("10000"));

    const userRewards = new Map<string, Map<string, Decimal>>();

    // Get all vessels controlled by hydromancer, grouped by user and eligible proposal duration
    const userVesselsByDuration = new Map<
      string,
      Map<number, VesselWithUser[]>
    >();
    for (const user of scenario.users) {
      for (const vessel of user.vessels) {
        if (
          vessel.rounds.some(
            (r) => r.round_id === roundId && r.controlled_by === "hydromancer"
          )
        ) {
          const vesselWithUser: VesselWithUser = {
            ...vessel,
            user_id: user.user_id,
          };

          if (!userVesselsByDuration.has(user.user_id)) {
            userVesselsByDuration.set(
              user.user_id,
              new Map<number, VesselWithUser[]>()
            );
          }
          const userMap = userVesselsByDuration.get(user.user_id)!;
          const lockDuration = this.getLockDuration(vessel, roundId);

          if (!userMap.has(lockDuration)) {
            userMap.set(lockDuration, []);
          }
          userMap.get(lockDuration)!.push(vesselWithUser);
        }
      }
    }

    // For each proposal, calculate rewards that should be shared among delegated users
    for (const proposal of scenario.proposals) {
      const proposalDuration = proposal.bid_duration_months;
      const proposalId = proposal.id;

      // Find hydromancer vessels that voted for this proposal
      const vesselsByProposal = this.getVesselsByProposal(scenario, roundId);
      if (!vesselsByProposal.has(proposalId)) {
        continue;
      }

      const hydromancerVesselsForProposal = vesselsByProposal
        .get(proposalId)!
        .filter((v) =>
          v.rounds.some(
            (r) => r.round_id === roundId && r.controlled_by === "hydromancer"
          )
        );

      if (hydromancerVesselsForProposal.length === 0) {
        continue;
      }

      // Calculate total hydromancer voting power for this proposal
      const totalHydromancerPowerForProposal =
        hydromancerVesselsForProposal.reduce(
          (sum, vessel) =>
            sum.add(this.calculateVotingPower(roundId, vessel, logger)),
          new Decimal("0")
        );

      // Calculate hydromancer's share of total voting power
      const totalPowerByProposal = this.calculateTotalVotingPowerByProposal(
        scenario,
        roundId,
        logger
      );
      const totalPower = totalPowerByProposal.get(proposalId)!;
      const hydromancerShare = totalHydromancerPowerForProposal.div(totalPower);

      // Calculate eligible user voting power for this proposal duration
      const eligibleUserPower = new Map<string, Decimal>();
      let totalEligiblePower = new Decimal("0");

      for (const [userId, vesselsByDuration] of userVesselsByDuration) {
        let userPower = new Decimal("0");
        // Users can participate if their vessel duration >= proposal duration
        for (const [duration, vessels] of vesselsByDuration) {
          if (duration >= proposalDuration) {
            for (const vessel of vessels) {
              userPower = userPower.add(
                this.calculateVotingPower(roundId, vessel, logger)
              );
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
          const tributeAmount = new Decimal(tribute.amount);
          // Remove protocol commission
          const afterProtocolCommission = tributeAmount.mul(
            new Decimal("1").sub(protocolCommissionRate)
          );
          // Get hydromancer's share
          const hydromancerTributeShare =
            afterProtocolCommission.mul(hydromancerShare);
          // Remove hydromancer commission
          const afterHydromancerCommission = hydromancerTributeShare.mul(
            new Decimal("1").sub(hydromancerCommissionRate)
          );

          // Distribute among eligible users based on their voting power
          for (const [userId, userPower] of eligibleUserPower) {
            const userShare = userPower.div(totalEligiblePower);
            const userReward = afterHydromancerCommission.mul(userShare);

            if (!userRewards.has(userId)) {
              userRewards.set(userId, new Map<string, Decimal>());
            }
            const userRewardMap = userRewards.get(userId)!;

            const currentAmount =
              userRewardMap.get(tribute.denom) || new Decimal("0");
            userRewardMap.set(tribute.denom, currentAmount.add(userReward));
          }
        }
      }
    }

    const result: { [userId: string]: { [denom: string]: Decimal } } = {};
    for (const [userId, rewardMap] of userRewards) {
      result[userId] = {};
      for (const [denom, amount] of rewardMap) {
        result[userId][denom] = amount;
      }
    }

    return result;
  }

  calculateAllRewardsForRound(
    scenario: Scenario,
    roundId: number,
    logger: TestLogger
  ): RewardsResult {
    // Calculate protocol rewards
    const protocolRewards = this.calculateProtocolRewards(scenario, roundId);

    // Calculate hydromancer rewards
    const hydromancerRewards = this.calculateHydromancerRewards(
      scenario,
      roundId,
      logger
    );

    // Calculate user direct rewards
    const userDirectRewards = this.calculateUserDirectRewards(
      scenario,
      roundId,
      logger
    );

    // Calculate user delegated rewards
    const userDelegatedRewards = this.calculateUserDelegatedRewards(
      scenario,
      roundId,
      logger
    );

    // Combine user rewards
    const allUserRewards = new Map<string, Map<string, Decimal>>();

    // Add direct rewards
    for (const [userId, rewards] of Object.entries(userDirectRewards)) {
      if (!allUserRewards.has(userId)) {
        allUserRewards.set(userId, new Map<string, Decimal>());
      }
      const userRewardMap = allUserRewards.get(userId)!;

      for (const [denom, amount] of Object.entries(rewards)) {
        const currentAmount = userRewardMap.get(denom) || new Decimal("0");
        userRewardMap.set(denom, currentAmount.add(amount));
      }
    }

    // Add delegated rewards
    for (const [userId, rewards] of Object.entries(userDelegatedRewards)) {
      if (!allUserRewards.has(userId)) {
        allUserRewards.set(userId, new Map<string, Decimal>());
      }
      const userRewardMap = allUserRewards.get(userId)!;

      for (const [denom, amount] of Object.entries(rewards)) {
        const currentAmount = userRewardMap.get(denom) || new Decimal("0");
        userRewardMap.set(denom, currentAmount.add(amount));
      }
    }

    // Convert to result format with string amounts for JSON serialization
    const finalProtocolRewards: { [denom: string]: string } = {};
    for (const [denom, amount] of Object.entries(protocolRewards)) {
      finalProtocolRewards[denom] = amount.toFixed(2);
    }

    const finalHydromancerRewards: { [denom: string]: string } = {};
    for (const [denom, amount] of Object.entries(hydromancerRewards)) {
      finalHydromancerRewards[denom] = amount.toFixed(2);
    }

    const finalUserRewards: { [userId: string]: { [denom: string]: string } } =
      {};
    for (const [userId, rewardMap] of allUserRewards) {
      finalUserRewards[userId] = {};
      for (const [denom, amount] of rewardMap) {
        finalUserRewards[userId][denom] = amount.toFixed(2);
      }
    }

    return {
      protocol_rewards: finalProtocolRewards,
      hydromancer_rewards: finalHydromancerRewards,
      user_rewards: finalUserRewards,
    };
  }
}
