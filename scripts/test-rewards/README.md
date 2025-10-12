# Zephyrus Rewards Testing Framework

A comprehensive TypeScript testing framework for validating Zephyrus protocol rewards calculations by executing scenarios on-chain and comparing results with mathematical expectations.

## Overview

This framework tests the Zephyrus rewards system by:

1. Loading test scenarios from JSON files
2. Setting up blockchain environment with contracts
3. Creating user wallets and funding them
4. Executing vessel creation, delegation, and voting
5. Calculating expected rewards using ported Python logic
6. Querying actual rewards from deployed contracts
7. Comparing expected vs actual rewards with detailed reporting

## Architecture

### Core Components

- **`test-rewards-integration.ts`** - Main orchestrator script
- **`calculate-rewards.ts`** - TypeScript port of Python rewards calculator
- **`scenario-executor.ts`** - Handles on-chain scenario execution
- **`rewards-validator.ts`** - Validates actual vs expected rewards
- **`wallet-manager.ts`** - Manages test wallet creation and funding
- **`environment-setup.ts`** - Sets up contracts and environment
- **`config.ts`** - Configuration and constants
- **`test-utils.ts`** - Utilities, logging, and reporting
- **`error-handler.ts`** - Comprehensive error handling

### Contract Integration

- **`contracts/`** - Contains TypeScript clients for:
  - `ZephyrusMain.client.ts` - Zephyrus contract interactions
  - `HydroBase.client.ts` - Hydro protocol interactions
  - `TributeBase.client.ts` - Tribute contract interactions

## Setup

### Prerequisites

1. **Deployed Contracts**: Hydro and Zephyrus contracts must be deployed
2. **Node.js**: Version 16 or higher
3. **Neutron Devnet**: Running on localhost with standard ports

### Installation

```bash
# Install dependencies
npm install

# Build TypeScript
npm run build
```

### Configuration

The framework uses the provided deployer mnemonic with pre-funded tokens:

- NTRN (untrn)
- dATOM (factory/neutron1k6hr0f83e7un2wjf29cspk7j69jrnskk65k3ek2nj9dztrlzpj6q00rtsa/udatom)
- stATOM (ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C)

Contract addresses are loaded from deployment configuration files in `deploy_scripts/`.

## Usage

### Single Test

Run a test against a specific scenario file:

```bash
# Using npm script
npm run test:scenario rewards-scenario-1.json

# Or directly with ts-node
npm run dev rewards-scenario-1.json

# Or with built version
npm test rewards-scenario-1.json
```

### Batch Testing

Run multiple scenarios programmatically:

```typescript
import { runBatchTests } from "./src/test-rewards-integration";

const scenarios = ["rewards-scenario-1.json", "rewards-scenario-2.json"];

const reports = await runBatchTests(scenarios);
```

## Test Scenarios

### Scenario Format

Test scenarios are JSON files with the following structure:

```json
{
  "protocol_config": {
    "round_length": 300000000000,
    "protocol_commission_bps": 1000,
    "hydromancer_commission_bps": 500
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
          "denom": "NTRN",
          "amount": "1000.00"
        }
      ]
    }
  ]
}
```

### Scenario Generation

Use the existing Python scripts to generate test scenarios:

```bash
cd test-rewards/
python generate-json.py
```

## Test Process

### 1. Environment Setup

- Load contract addresses from deployment configs
- Verify contracts are deployed and accessible
- Create and fund test user wallets
- Initialize contract clients

### 2. Scenario Execution

- Create tribute proposals on Hydro contract
- Create vessels (lockups) for each user
- Delegate vessels to Zephyrus if `controlled_by: "hydromancer"`
- Execute votes according to scenario specifications
- Simulate liquidity deployment to make rewards claimable

### 3. Rewards Calculation

- Calculate expected rewards using TypeScript port of Python logic
- Apply token multipliers: dATOM (1.15x), stATOM (1.6x)
- Apply duration multipliers: 1mo (1.0x), 2mo (1.25x), 3mo (1.5x)
- Calculate protocol commission (10%) and hydromancer commission (5%)
- Distribute rewards based on voting power and delegation

### 4. Rewards Validation

- Query actual claimable rewards from contracts
- Compare expected vs actual rewards with tolerance for rounding
- Claim rewards to verify actual transfer amounts
- Generate detailed discrepancy reports

### 5. Reporting

- Generate comprehensive test reports with:
  - Test execution summary
  - Expected vs actual rewards breakdown
  - Discrepancies with percentages
  - Transaction hashes and gas usage
  - Pass/fail status with detailed errors

## Rewards Logic

### Token Multipliers

- **dATOM**: 1.15x voting power
- **stATOM**: 1.6x voting power

### Duration Multipliers

- **1 month**: 1.0x voting power
- **2 months**: 1.25x voting power
- **3 months**: 1.5x voting power

### Commission Structure

- **Protocol Commission**: 10% (1000 basis points)
- **Hydromancer Commission**: 5% (500 basis points)

### Reward Distribution

1. **Protocol Rewards**: 10% of all tributes go to protocol
2. **Direct User Rewards**: Users voting directly get their share minus protocol commission
3. **Delegated User Rewards**: Users delegating to hydromancer get their share minus both commissions
4. **Hydromancer Rewards**: 5% of hydromancer-controlled voting power rewards

## Error Handling

The framework includes comprehensive error handling with:

- Categorized error codes for different failure types
- Detailed error context and debugging information
- Retry mechanisms for transient failures
- Error logging and reporting
- Graceful cleanup on failures

### Common Error Types

- `CONTRACT_NOT_FOUND`: Contract not deployed or wrong address
- `WALLET_INSUFFICIENT_FUNDS`: Insufficient tokens for operations
- `TX_FAILED`: Transaction execution failed
- `REWARDS_MISMATCH`: Expected vs actual rewards don't match
- `SCENARIO_INVALID`: Invalid scenario format

## Output

### Console Output

- Real-time test execution progress
- Detailed reward calculations and comparisons
- Error messages with context
- Final pass/fail status

### Report Files

- JSON reports saved to `reports/` directory
- Detailed transaction logs
- Error summaries and debugging information

### Example Output

```
==================================================
TEST REPORT: Scenario Test: rewards-scenario-1.json
==================================================
Status: ✅ PASSED
Execution Time: 45.23s
Gas Used: 2547391
Transactions: 15

✅ All rewards match expected values within tolerance

TRANSACTION HASHES:
────────────────────────────────
  A1B2C3D4E5F6...
  F6E5D4C3B2A1...
==================================================
```

## Troubleshooting

### Common Issues

1. **Contracts Not Found**
   - Ensure Hydro and Zephyrus contracts are deployed
   - Check contract addresses in deployment configs

2. **Insufficient Gas**
   - Increase gas limits in configuration
   - Ensure deployer wallet has sufficient NTRN for gas

3. **RPC Connection Issues**
   - Verify Neutron devnet is running on localhost:26657
   - Check network connectivity and RPC endpoint

4. **Wallet Funding Failures**
   - Ensure deployer wallet has sufficient token balances
   - Verify token denominations are correct

### Debug Mode

Enable detailed logging by setting environment variables:

```bash
DEBUG=true npm run test:scenario rewards-scenario-1.json
```

## Integration with Existing Scripts

This framework integrates with existing deployment and testing infrastructure:

- Uses contract addresses from `deploy_scripts/` configurations
- Leverages existing TypeScript contract clients
- Compatible with existing scenario generation scripts
- Follows same token denomination conventions

## Development

### Adding New Tests

1. Create scenario JSON files using `generate-json.py`
2. Run tests with the framework
3. Analyze reports for any discrepancies
4. Debug using detailed error logs and transaction hashes

### Extending Functionality

- Add new contract interactions in scenario executor
- Extend rewards calculator for new token types
- Add new validation rules in rewards validator
- Enhance reporting with additional metrics

## License

MIT License - see LICENSE file for details.
