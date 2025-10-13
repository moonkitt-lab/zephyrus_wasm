import json
import random
from typing import List, Dict, Any, Optional

class ScenarioGenerator:
    def __init__(self):
        # Available token denominations for vessels (locked tokens)
        self.vessel_token_denoms = ["dATOM", "stATOM"]
        
        # Available token denominations for tributes (rewards)
        self.tribute_token_denoms = ["dATOM", "stATOM","USDC"]
        
        # Duration options (in months/rounds)
        self.durations = [1, 2, 3]
        
        # Controllers
        self.controllers = ["user", "hydromancer"]
        
        # Default commission rates (in basis points)
        self.default_protocol_commission_bps = 1000
        self.default_hydromancer_commission_bps = 500
    
    def generate_vessel(self, vessel_id: int, user_id: str, total_rounds: int) -> Dict[str, Any]:
        """Generate a single vessel with random properties and round states."""
        initial_controller = random.choice(self.controllers)
        
        # Generate round states for each round
        rounds = []
        current_controller = initial_controller
        
        for round_id in range(total_rounds):
            round_state = {
                "round_id": round_id,
                "controlled_by": current_controller,
                "voted_proposal_id": None,  # Will be assigned later
                "refresh": random.choice([True, False])  # Random refresh setting
            }
            rounds.append(round_state)
            
            # Randomly change controller between rounds (20% chance)
            if random.random() < 0.2:
                current_controller = "user" if current_controller == "hydromancer" else "hydromancer"
        
        return {
            "id": vessel_id,
            "lock_duration_rounds": random.choice(self.durations),
            "locked_denom": random.choice(self.vessel_token_denoms),  # Only stATOM or dATOM
            "locked_amount": str(random.randint(10, 999)),
            "rounds": rounds
        }
    
    def generate_user(self, user_id: str, vessel_count: int, starting_vessel_id: int, total_rounds: int) -> tuple:
        """Generate a user with specified number of vessels."""
        vessels = []
        for i in range(vessel_count):
            vessel = self.generate_vessel(starting_vessel_id + i, user_id, total_rounds)
            vessels.append(vessel)
        
        user = {
            "user_id": user_id,
            "vessels": vessels
        }
        
        return user, starting_vessel_id + vessel_count
    
    def generate_tribute(self, tribute_id: int) -> Dict[str, Any]:
        """Generate a single tribute with random properties."""
        return {
            "id": tribute_id,
            "denom": random.choice(self.tribute_token_denoms),  # Can be any token
            "amount": f"{random.randint(10, 1000)}.00"
        }
    
    def generate_proposal(self, proposal_id: int, starting_tribute_id: int, round_id: int) -> tuple:
        """Generate a proposal with random tributes."""
        tribute_count = random.randint(1, 2)  # 1 to 2 tributes (max 2)
        tributes = []
        
        for i in range(tribute_count):
            tribute = self.generate_tribute(starting_tribute_id + i)
            tributes.append(tribute)
        
        proposal = {
            "id": proposal_id,
            "round_id": round_id,
            "bid_duration_months": random.choice(self.durations),
            "tributes": tributes
        }
        
        return proposal, starting_tribute_id + tribute_count
    
    def assign_vessels_to_proposals(self, users: List[Dict], proposals: List[Dict]) -> None:
        """Assign vessels to proposals respecting duration constraints."""
        for user in users:
            for vessel in user["vessels"]:
                for round_state in vessel["rounds"]:
                    round_id = round_state["round_id"]
                    controlled_by = round_state["controlled_by"]
                    
                    # Find proposals for this round
                    round_proposals = [p for p in proposals if p["round_id"] == round_id]
                    
                    # Only consider proposals with bid_duration <= vessel lock_duration
                    eligible_proposals = [
                        p for p in round_proposals 
                        if p["bid_duration_months"] <= vessel["lock_duration_rounds"]
                    ]
                    
                    if eligible_proposals:
                        # Different probabilities based on controller
                        if controlled_by == "hydromancer":
                            # 70% chance for hydromancer-controlled vessels
                            if random.random() < 0.7:
                                chosen_proposal = random.choice(eligible_proposals)
                                round_state["voted_proposal_id"] = chosen_proposal["id"]
                        else:  # user controlled
                            # 40% chance for user-controlled vessels
                            if random.random() < 0.4:
                                chosen_proposal = random.choice(eligible_proposals)
                                round_state["voted_proposal_id"] = chosen_proposal["id"]
    
    def generate_scenario(self, 
                         num_users: int = 2,
                         vessels_per_user_range: tuple = (1, 2),
                         num_proposals: int = 4,
                         total_rounds: int = 7,
                         proposals_per_round_range: tuple = (2, 2),
                         protocol_commission_bps: int = None,
                         hydromancer_commission_bps: int = None) -> Dict[str, Any]:
        """
        Generate a complete scenario.
        
        Args:
            num_users: Number of users to generate
            vessels_per_user_range: Tuple of (min, max) vessels per user
            num_proposals: Total number of proposals to generate (for backward compatibility)
            total_rounds: Number of rounds in the scenario
            proposals_per_round_range: Tuple of (min, max) proposals per round
            protocol_commission_bps: Protocol commission in basis points
            hydromancer_commission_bps: Hydromancer commission in basis points
        """
        
        # Set default commission rates if not provided
        if protocol_commission_bps is None:
            protocol_commission_bps = self.default_protocol_commission_bps
        if hydromancer_commission_bps is None:
            hydromancer_commission_bps = self.default_hydromancer_commission_bps
        
        # Generate protocol config
        average_vessels_per_user = (vessels_per_user_range[0] + vessels_per_user_range[1]) / 2
        protocol_config = {
            "round_length": int(num_users * average_vessels_per_user * 15 * 1e9),  # Approximation in nanoseconds
            "protocol_commission_bps": protocol_commission_bps,
            "hydromancer_commission_bps": hydromancer_commission_bps,
            "total_rounds": total_rounds
        }
        
        # Generate users
        users = []
        current_vessel_id = 0
        user_ids = [str(i) for i in range(num_users)]  # 0, 1, 2, ...
        
        for user_id in user_ids:
            vessel_count = random.randint(*vessels_per_user_range)
            user, current_vessel_id = self.generate_user(user_id, vessel_count, current_vessel_id, total_rounds)
            users.append(user)
        
        # Generate proposals per round
        proposals = []
        current_tribute_id = 0
        current_proposal_id = 0
        
        for round_id in range(total_rounds):
            # Generate random number of proposals for this round
            proposals_this_round = random.randint(*proposals_per_round_range)
            
            for _ in range(proposals_this_round):
                proposal, current_tribute_id = self.generate_proposal(current_proposal_id, current_tribute_id, round_id)
                proposals.append(proposal)
                current_proposal_id += 1
        
        # Assign vessels to proposals
        self.assign_vessels_to_proposals(users, proposals)
        
        return {
            "protocol_config": protocol_config,
            "users": users,
            "proposals": proposals
        }
    
    def generate_multiple_scenarios(self, count: int, **kwargs) -> List[Dict[str, Any]]:
        """Generate multiple scenarios."""
        scenarios = []
        for i in range(count):
            scenario = self.generate_scenario(**kwargs)
            scenarios.append(scenario)
        return scenarios
    
    def save_scenario(self, scenario: Dict[str, Any], filename: str) -> None:
        """Save a scenario to a JSON file."""
        with open(filename, 'w') as f:
            json.dump(scenario, f, indent=4)
    
    def save_scenarios(self, scenarios: List[Dict[str, Any]], filename_prefix: str) -> None:
        """Save multiple scenarios to separate JSON files."""
        for i, scenario in enumerate(scenarios, 1):
            filename = f"{filename_prefix}-{i}.json"
            self.save_scenario(scenario, filename)


# Example usage
if __name__ == "__main__":
    generator = ScenarioGenerator()
    
    # Generate a single scenario
    scenario = generator.generate_scenario(
        num_users=3,
        vessels_per_user_range=(2, 4),
        num_proposals=6,  # Total proposals (for backward compatibility)
        total_rounds=3,   # 3 rounds scenario
        proposals_per_round_range=(1, 2)  # 1-2 proposals per round
    )
    
    print("Generated scenario:")
    print(json.dumps(scenario, indent=2))
    
    # Save the scenario
    generator.save_scenario(scenario, "rewards-scenario-8.json")
    
    # # Generate multiple scenarios for testing
    # print("\n" + "="*50)
    # print("Generating multiple test scenarios...")
    # print("Note: Vessels can only contain stATOM or dATOM tokens")
    # print("      Tributes can contain any token (dATOM, stATOM, USDC, NTRN)")
    
    # scenarios = generator.generate_multiple_scenarios(
    #     count=5,
    #     num_users=3,
    #     vessels_per_user_range=(3, 7),
    #     num_proposals=4
    # )
    
    # # Save all scenarios
    # generator.save_scenarios(scenarios, "test-scenario")
    # print(f"Generated and saved {len(scenarios)} test scenarios")
    
    # # Print summary of each scenario
    # for i, scenario in enumerate(scenarios, 1):
    #     total_vessels = sum(len(user["vessels"]) for user in scenario["users"])
    #     assigned_vessels = sum(
    #         sum(1 for vessel in user["vessels"] if vessel["voted_proposal_id"] is not None)
    #         for user in scenario["users"]
    #     )
    #     total_tributes = sum(len(proposal["tributes"]) for proposal in scenario["proposals"])
        
    #     print(f"Scenario {i}: {total_vessels} vessels, {assigned_vessels} assigned, {total_tributes} tributes")