import json
from typing import Dict, List, Any, Tuple
from collections import defaultdict
from decimal import Decimal, ROUND_HALF_UP

class RewardsCalculator:
    def __init__(self):
        # Token multipliers for voting power (only applies to vessel tokens: stATOM and dATOM)
        self.token_multipliers = {
            "dATOM": Decimal("1.3"),
            "stATOM": Decimal("1.6"),
        }
        
        # Duration multipliers for voting power
        self.duration_multipliers = {
            1: Decimal("1.0"),
            2: Decimal("1.25"),
            3: Decimal("1.5")
        }
        
        # Note: Vessels can only contain stATOM or dATOM
        # Tributes can contain any token (dATOM, stATOM, USDC, NTRN, etc.)
    
    def calculate_voting_power(self, vessel: Dict[str, Any]) -> Decimal:
        """Calculate the voting power of a vessel.
        Note: Vessels can only contain stATOM or dATOM tokens."""
        amount = Decimal(vessel["locked_amount"])
        token_denom = vessel["locked_denom"]
        lock_duration = vessel["lock_duration_rounds"]
        
        # Apply token multiplier (should always exist since vessels only contain stATOM/dATOM)
        if token_denom not in self.token_multipliers:
            raise ValueError(f"Invalid vessel token: {token_denom}. Vessels can only contain stATOM or dATOM.")
        
        token_multiplier = self.token_multipliers[token_denom]
        
        # Apply duration multiplier
        duration_multiplier = self.duration_multipliers[lock_duration]
        
        voting_power = amount * token_multiplier * duration_multiplier
        return voting_power
    
    def get_vessels_by_proposal(self, scenario: Dict[str, Any]) -> Dict[int, List[Dict]]:
        """Group vessels by the proposal they voted for."""
        vessels_by_proposal = defaultdict(list)
        
        for user in scenario["users"]:
            for vessel in user["vessels"]:
                if vessel["voted_proposal_id"] is not None:
                    vessel_with_user = vessel.copy()
                    vessel_with_user["user_id"] = user["user_id"]
                    vessels_by_proposal[vessel["voted_proposal_id"]].append(vessel_with_user)
        
        return vessels_by_proposal
    
    def calculate_protocol_rewards(self, scenario: Dict[str, Any]) -> Dict[str, Decimal]:
        """Calculate protocol rewards from commission on active proposals."""
        protocol_commission_rate = Decimal(scenario["protocol_config"]["protocol_commission_bps"]) / Decimal("10000")
        protocol_rewards = defaultdict(Decimal)
        
        vessels_by_proposal = self.get_vessels_by_proposal(scenario)
        
        # Only proposals with at least one vessel assigned generate rewards
        active_proposal_ids = set(vessels_by_proposal.keys())
        
        for proposal in scenario["proposals"]:
            if proposal["id"] in active_proposal_ids:
                for tribute in proposal["tributes"]:
                    tribute_amount = Decimal(tribute["amount"])
                    commission = tribute_amount * protocol_commission_rate
                    protocol_rewards[tribute["denom"]] += commission
        
        return dict(protocol_rewards)
    
    def calculate_hydromancer_voting_power_by_proposal(self, scenario: Dict[str, Any]) -> Dict[int, Decimal]:
        """Calculate hydromancer's total voting power for each proposal."""
        vessels_by_proposal = self.get_vessels_by_proposal(scenario)
        hydromancer_power_by_proposal = {}
        
        for proposal_id, vessels in vessels_by_proposal.items():
            hydromancer_power = Decimal("0")
            for vessel in vessels:
                if vessel["controlled_by"] == "hydromancer":
                    hydromancer_power += self.calculate_voting_power(vessel)
            hydromancer_power_by_proposal[proposal_id] = hydromancer_power
        
        return hydromancer_power_by_proposal
    
    def calculate_total_voting_power_by_proposal(self, scenario: Dict[str, Any]) -> Dict[int, Decimal]:
        """Calculate total voting power for each proposal."""
        vessels_by_proposal = self.get_vessels_by_proposal(scenario)
        total_power_by_proposal = {}
        
        for proposal_id, vessels in vessels_by_proposal.items():
            total_power = Decimal("0")
            for vessel in vessels:
                total_power += self.calculate_voting_power(vessel)
            total_power_by_proposal[proposal_id] = total_power
        
        return total_power_by_proposal
    
    def calculate_hydromancer_rewards(self, scenario: Dict[str, Any]) -> Dict[str, Decimal]:
        """Calculate hydromancer rewards from commission on their voting power."""
        hydromancer_commission_rate = Decimal(scenario["protocol_config"]["hydromancer_commission_bps"]) / Decimal("10000")
        protocol_commission_rate = Decimal(scenario["protocol_config"]["protocol_commission_bps"]) / Decimal("10000")
        
        hydromancer_rewards = defaultdict(Decimal)
        hydromancer_power_by_proposal = self.calculate_hydromancer_voting_power_by_proposal(scenario)
        total_power_by_proposal = self.calculate_total_voting_power_by_proposal(scenario)
        
        for proposal in scenario["proposals"]:
            proposal_id = proposal["id"]
            
            if (proposal_id in hydromancer_power_by_proposal and 
                hydromancer_power_by_proposal[proposal_id] > 0):
                
                # Calculate hydromancer's share of each tribute
                hydromancer_power = hydromancer_power_by_proposal[proposal_id]
                total_power = total_power_by_proposal[proposal_id]
                hydromancer_share = hydromancer_power / total_power
                
                for tribute in proposal["tributes"]:
                    tribute_amount = Decimal(tribute["amount"])
                    # Remove protocol commission first
                    after_protocol_commission = tribute_amount * (Decimal("1") - protocol_commission_rate)
                    # Hydromancer gets their share
                    hydromancer_tribute_share = after_protocol_commission * hydromancer_share
                    # Hydromancer takes commission from their share
                    hydromancer_commission = hydromancer_tribute_share * hydromancer_commission_rate
                    
                    hydromancer_rewards[tribute["denom"]] += hydromancer_commission
        
        return dict(hydromancer_rewards)
    
    def calculate_user_direct_rewards(self, scenario: Dict[str, Any]) -> Dict[str, Dict[str, Decimal]]:
        """Calculate rewards for users who vote directly (not through hydromancer)."""
        protocol_commission_rate = Decimal(scenario["protocol_config"]["protocol_commission_bps"]) / Decimal("10000")
        user_rewards = defaultdict(lambda: defaultdict(Decimal))
        
        vessels_by_proposal = self.get_vessels_by_proposal(scenario)
        total_power_by_proposal = self.calculate_total_voting_power_by_proposal(scenario)
        
        for proposal in scenario["proposals"]:
            proposal_id = proposal["id"]
            
            if proposal_id in vessels_by_proposal:
                total_power = total_power_by_proposal[proposal_id]
                
                # Find user-controlled vessels for this proposal
                user_vessels = [v for v in vessels_by_proposal[proposal_id] if v["controlled_by"] == "user"]
                
                for vessel in user_vessels:
                    user_id = vessel["user_id"]
                    vessel_power = self.calculate_voting_power(vessel)
                    user_share = vessel_power / total_power
                    
                    for tribute in proposal["tributes"]:
                        tribute_amount = Decimal(tribute["amount"])
                        # Remove protocol commission
                        after_protocol_commission = tribute_amount * (Decimal("1") - protocol_commission_rate)
                        user_reward = after_protocol_commission * user_share
                        
                        user_rewards[user_id][tribute["denom"]] += user_reward
        
        return {user_id: dict(rewards) for user_id, rewards in user_rewards.items()}
    
    def calculate_user_delegated_rewards(self, scenario: Dict[str, Any]) -> Dict[str, Dict[str, Decimal]]:
        """Calculate rewards for users who delegate to hydromancer."""
        protocol_commission_rate = Decimal(scenario["protocol_config"]["protocol_commission_bps"]) / Decimal("10000")
        hydromancer_commission_rate = Decimal(scenario["protocol_config"]["hydromancer_commission_bps"]) / Decimal("10000")
        
        user_rewards = defaultdict(lambda: defaultdict(Decimal))
        
        # Get all vessels controlled by hydromancer, grouped by user and eligible proposal duration
        user_vessels_by_duration = defaultdict(lambda: defaultdict(list))
        for user in scenario["users"]:
            for vessel in user["vessels"]:
                if vessel["controlled_by"] == "hydromancer":
                    vessel_with_user = vessel.copy()
                    vessel_with_user["user_id"] = user["user_id"]
                    user_vessels_by_duration[user["user_id"]][vessel["lock_duration_rounds"]].append(vessel_with_user)
        
        # For each proposal, calculate rewards that should be shared among delegated users
        for proposal in scenario["proposals"]:
            proposal_duration = proposal["bid_duration_months"]
            proposal_id = proposal["id"]
            
            # Find hydromancer vessels that voted for this proposal
            vessels_by_proposal = self.get_vessels_by_proposal(scenario)
            if proposal_id not in vessels_by_proposal:
                continue
                
            hydromancer_vessels_for_proposal = [
                v for v in vessels_by_proposal[proposal_id] 
                if v["controlled_by"] == "hydromancer"
            ]
            
            if not hydromancer_vessels_for_proposal:
                continue
            
            # Calculate total hydromancer voting power for this proposal
            total_hydromancer_power_for_proposal = sum(
                self.calculate_voting_power(vessel) for vessel in hydromancer_vessels_for_proposal
            )
            
            # Calculate hydromancer's share of total voting power
            total_power_by_proposal = self.calculate_total_voting_power_by_proposal(scenario)
            total_power = total_power_by_proposal[proposal_id]
            hydromancer_share = total_hydromancer_power_for_proposal / total_power
            
            # Calculate eligible user voting power for this proposal duration
            eligible_user_power = defaultdict(Decimal)
            total_eligible_power = Decimal("0")
            
            for user_id, vessels_by_duration in user_vessels_by_duration.items():
                user_power = Decimal("0")
                # Users can participate if their vessel duration >= proposal duration
                for duration, vessels in vessels_by_duration.items():
                    if duration >= proposal_duration:
                        for vessel in vessels:
                            user_power += self.calculate_voting_power(vessel)
                
                if user_power > 0:
                    eligible_user_power[user_id] = user_power
                    total_eligible_power += user_power
            
            if total_eligible_power > 0:
                # Distribute hydromancer's tribute share among eligible users
                for tribute in proposal["tributes"]:
                    tribute_amount = Decimal(tribute["amount"])
                    # Remove protocol commission
                    after_protocol_commission = tribute_amount * (Decimal("1") - protocol_commission_rate)
                    # Get hydromancer's share
                    hydromancer_tribute_share = after_protocol_commission * hydromancer_share
                    # Remove hydromancer commission
                    after_hydromancer_commission = hydromancer_tribute_share * (Decimal("1") - hydromancer_commission_rate)
                    
                    # Distribute among eligible users based on their voting power
                    for user_id, user_power in eligible_user_power.items():
                        user_share = user_power / total_eligible_power
                        user_reward = after_hydromancer_commission * user_share
                        user_rewards[user_id][tribute["denom"]] += user_reward
        
        return {user_id: dict(rewards) for user_id, rewards in user_rewards.items()}
    
    def calculate_all_rewards(self, scenario: Dict[str, Any]) -> Dict[str, Any]:
        """Calculate all rewards for the scenario."""
        
        # Calculate protocol rewards
        protocol_rewards = self.calculate_protocol_rewards(scenario)
        
        # Calculate hydromancer rewards
        hydromancer_rewards = self.calculate_hydromancer_rewards(scenario)
        
        # Calculate user direct rewards
        user_direct_rewards = self.calculate_user_direct_rewards(scenario)
        
        # Calculate user delegated rewards
        user_delegated_rewards = self.calculate_user_delegated_rewards(scenario)
        
        # Combine user rewards
        all_user_rewards = defaultdict(lambda: defaultdict(Decimal))
        
        # Add direct rewards
        for user_id, rewards in user_direct_rewards.items():
            for denom, amount in rewards.items():
                all_user_rewards[user_id][denom] += amount
        
        # Add delegated rewards
        for user_id, rewards in user_delegated_rewards.items():
            for denom, amount in rewards.items():
                all_user_rewards[user_id][denom] += amount
        
        # Convert to regular dict with string amounts for JSON serialization
        final_user_rewards = {}
        for user_id, rewards in all_user_rewards.items():
            final_user_rewards[user_id] = {
                denom: str(amount.quantize(Decimal('0.01'), rounding=ROUND_HALF_UP))
                for denom, amount in rewards.items()
            }
        
        return {
            "protocol_rewards": {
                denom: str(amount.quantize(Decimal('0.01'), rounding=ROUND_HALF_UP))
                for denom, amount in protocol_rewards.items()
            },
            "hydromancer_rewards": {
                denom: str(amount.quantize(Decimal('0.01'), rounding=ROUND_HALF_UP))
                for denom, amount in hydromancer_rewards.items()
            },
            "user_rewards": final_user_rewards
        }
    
    def calculate_rewards_from_file(self, filename: str) -> Dict[str, Any]:
        """Load scenario from file and calculate rewards."""
        with open(filename, 'r') as f:
            scenario = json.load(f)
        return self.calculate_all_rewards(scenario)
    
    def save_rewards_calculation(self, rewards: Dict[str, Any], filename: str) -> None:
        """Save rewards calculation to file."""
        with open(filename, 'w') as f:
            json.dump(rewards, f, indent=4)


# Example usage and testing
if __name__ == "__main__":
    calculator = RewardsCalculator()
    
    # Test with a sample scenario (you would replace this with actual file)
    sample_scenario = {
        "protocol_config": {
            "protocol_commission_bps": 1000,  # 10%
            "hydromancer_commission_bps": 500  # 5%
        },
        "users": [
            {
                "user_id": "A",
                "vessels": [
                    {
                        "id": 1,
                        "lock_duration_rounds": 2,
                        "locked_denom": "dATOM",
                        "locked_amount": "100",
                        "controlled_by": "hydromancer",
                        "voted_proposal_id": 1
                    },
                    {
                        "id": 2,
                        "lock_duration_rounds": 1,
                        "locked_denom": "stATOM",
                        "locked_amount": "50",
                        "controlled_by": "user",
                        "voted_proposal_id": 1
                    }
                ]
            }
        ],
        "proposals": [
            {
                "id": 1,
                "bid_duration_months": 1,
                "tributes": [
                    {
                        "id": 1,
                        "denom": "dATOM",
                        "amount": "1000.00"
                    },
                    {
                        "id": 2,
                        "denom": "USDC",
                        "amount": "500.00"
                    }
                ]
            }
        ]
    }
    
    # rewards = calculator.calculate_all_rewards(sample_scenario)
    # print("Sample calculation:")
    # print(json.dumps(rewards, indent=2))
    
    # print("\n" + "="*50)
    # print("To use with your scenario files:")
    # print("calculator = RewardsCalculator()")
    # print("rewards = calculator.calculate_rewards_from_file('scenario.json')")
    # print("calculator.save_rewards_calculation(rewards, 'rewards_output.json')")

    rewards = calculator.calculate_rewards_from_file('rewards-scenario-2.json')
    print(json.dumps(rewards, indent=2))