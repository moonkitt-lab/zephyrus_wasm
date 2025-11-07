# Zephyrus Rewards Testing Framework - Usage Guide

This guide shows you how to use the Zephyrus rewards testing framework to validate rewards calculations.

## Quick Start

### 1. Prerequisites

Make sure you have:

- Neutron devnet running on localhost:26657
- Hydro and Zephyrus contracts deployed
- Node.js 16+ installed

### 2. Install Dependencies

```bash
cd test-rewards/
npm install
```

### 3. Generate Test Scenarios

Use the existing Python scripts to generate test scenarios:

```bash
# Generate test scenarios
python generate-json.py
```

This will create files like `rewards-scenario-1.json` and `rewards-scenario-2.json`.

### 4. Run a Single Test

```bash
# Test a specific scenario
npm run dev rewards-scenario-1.json

# Or with the built version
npm run build
npm test rewards-scenario-1.json
```

### 5. Run Batch Tests

```bash
# Run all available scenario files
npm run test:batch
```

## Usage Examples

### Basic Test Execution

```bash
# Run demo to see the framework in action
npm run ts-node scripts/demo.ts

# Test specific scenario file
npm run dev /path/to/your/scenario.json

# Build and run tests
npm run prepare
npm test rewards-scenario-2.json
```

### Programmatic Usage

```typescript
import { TestRewardsIntegration } from "./src/test-rewards-integration";

const testRunner = new TestRewardsIntegration();
const report = await testRunner.runTest("rewards-scenario-1.json");

if (report.success) {
  console.log("✅ Test passed!");
} else {
  console.log("❌ Test failed with discrepancies:", report.discrepancies);
}
```

### Batch Testing

```typescript
import { runBatchTests } from "./src/test-rewards-integration";

const scenarios = ["rewards-scenario-1.json", "rewards-scenario-2.json"];

const reports = await runBatchTests(scenarios);
const successRate = reports.filter((r) => r.success).length / reports.length;
console.log(`Success rate: ${(successRate * 100).toFixed(1)}%`);
```

## Test Workflow

The framework follows this workflow:

### 1. Environment Setup

- Loads contract addresses from deployment configurations
- Creates test wallets for scenario users
- Funds wallets with required tokens from deployer account
- Verifies contracts are accessible

### 2. Scenario Execution

- Creates tribute proposals with specified amounts and tokens
- Creates vessels (lockups) for each user with correct parameters
- Delegates vessels to Zephyrus if controlled by hydromancer
- Executes votes according to scenario specifications
- Simulates round progression and liquidity deployment

### 3. Rewards Calculation

- Calculates expected rewards using ported Python logic
- Applies token multipliers (dATOM: 1.15x, stATOM: 1.6x)
- Applies duration multipliers (1mo: 1.0x, 2mo: 1.25x, 3mo: 1.5x)
- Computes protocol and hydromancer commissions
- Distributes rewards based on voting power and delegation

### 4. Rewards Validation

- Queries actual claimable rewards from deployed contracts
- Compares expected vs actual rewards with tolerance
- Claims rewards to verify actual transfer amounts
- Reports discrepancies with detailed analysis

### 5. Report Generation

- Creates comprehensive test reports
- Shows pass/fail status with execution metrics
- Lists transaction hashes and gas usage
- Provides detailed discrepancy analysis

## Configuration

### Environment Variables

Create a `.env` file based on `.env.example`:

```bash
cp .env.example .env
# Edit .env with your configuration
```

Key configuration options:

- `NEUTRON_RPC_ENDPOINT`: RPC endpoint for Neutron chain
- `DEPLOYER_MNEMONIC`: Mnemonic for funded deployer wallet
- `GAS_PRICE`: Gas price for transactions
- `PROTOCOL_COMMISSION_BPS`: Protocol commission in basis points
- `HYDROMANCER_COMMISSION_BPS`: Hydromancer commission in basis points

### Contract Addresses

Contract addresses are automatically loaded from deployment configs:

- `deploy_scripts/zephyrus_contract/config_devnet.json`
- `deploy_scripts/zephyrus_contract/instantiate_zephyrus_res.json`

## Scenario Format

Test scenarios are JSON files with this structure:

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

### Scenario Parameters

- **protocol_config**: Commission rates in basis points
- **users**: Array of test users with their vessels
- **proposals**: Array of proposals with tribute rewards
- **vessels**: Lockups with duration, token, amount, and control
- **tributes**: Reward tokens with denominations and amounts

## Output

### Console Output

The framework provides real-time feedback:

```
==================================================
Zephyrus Rewards Integration Test
==================================================
[2.1s] INFO: Scenario file: rewards-scenario-1.json
[2.3s] INFO: Loaded scenario with 3 users and 4 proposals

==================================================
Environment Setup
==================================================
[3.1s] INFO: Loading contract addresses from deployment configs...
[3.2s] INFO: Loaded contract addresses:
          Hydro: neutron1kjxjh26ravmzxlrq5n8scucta0c4w9teuvnc379ev6egytaqrzhsa8tqlt
          Tribute: neutron1x2quxjvl9p2hnth7dtq0cp4aetv0uqxkzpyc3srm6nne5288dn4sa4t73h
          Zephyrus: neutron1abc...

[4.5s] INFO: Test wallets setup completed for users: A, B, C
```

### Test Reports

Detailed JSON reports are saved to the `reports/` directory:

```json
{
  "testName": "Scenario Test: rewards-scenario-1.json",
  "success": true,
  "executionTime": 45.23,
  "transactionHashes": ["A1B2C3...", "F6E5D4..."],
  "expectedRewards": {
    "protocol_rewards": { "NTRN": "100.00" },
    "hydromancer_rewards": { "NTRN": "47.50" },
    "user_rewards": {
      "A": { "NTRN": "285.00" },
      "B": { "NTRN": "332.50" },
      "C": { "NTRN": "235.00" }
    }
  },
  "actualRewards": {
    /* ... actual results ... */
  },
  "discrepancies": []
}
```

## Troubleshooting

### Common Issues

1. **Contract Not Found**

   ```
   Error: CONTRACT_NOT_FOUND - Hydro contract not found
   ```

   - Solution: Deploy contracts using deployment scripts
   - Check contract addresses in config files

2. **Insufficient Funds**

   ```
   Error: WALLET_INSUFFICIENT_FUNDS - insufficient funds for gas
   ```

   - Solution: Ensure deployer wallet has NTRN for gas
   - Check token balances with `npm run dev` in debug mode

3. **RPC Connection Failed**

   ```
   Error: NETWORK_UNREACHABLE - connect ECONNREFUSED
   ```

   - Solution: Start Neutron devnet on localhost:26657
   - Verify RPC endpoint in configuration

4. **Scenario Invalid**

   ```
   Error: SCENARIO_INVALID - Scenario missing users array
   ```

   - Solution: Check scenario JSON format
   - Regenerate scenarios with Python scripts

### Debug Mode

Enable detailed logging:

```bash
DEBUG=true npm run dev rewards-scenario-1.json
```

### Manual Testing Steps

If automated tests fail, you can test manually:

1. **Check Contract Deployment**

   ```bash
   # Query contract info
   neutrond query wasm contract neutron1kjx... --node http://localhost:26657
   ```

2. **Verify Token Balances**

   ```bash
   # Check deployer balance
   neutrond query bank balances neutron1wgv... --node http://localhost:26657
   ```

3. **Test Contract Calls**
   ```bash
   # Query current round
   neutrond query wasm contract-state smart neutron1kjx... '{"current_round":{}}' --node http://localhost:26657
   ```

## Performance Considerations

### Optimization Tips

1. **Parallel Execution**: Tests run contract calls in parallel where possible
2. **Gas Optimization**: Uses "auto" gas estimation with 1.3x adjustment
3. **Connection Pooling**: Reuses CosmJS clients across operations
4. **Batch Operations**: Groups similar operations together

### Expected Timing

- Simple scenario (3 users, 4 proposals): ~45 seconds
- Complex scenario (5 users, 10 proposals): ~120 seconds
- Batch tests (multiple scenarios): ~300-600 seconds

### Resource Requirements

- RAM: ~512MB for Node.js process
- Network: Stable connection to Neutron devnet
- Disk: ~10MB for reports and logs
- Tokens: Sufficient NTRN for gas, test tokens for scenarios

## Best Practices

### Scenario Design

1. **Realistic Parameters**: Use realistic token amounts and durations
2. **Edge Cases**: Test boundary conditions (minimum amounts, maximum durations)
3. **Mixed Control**: Include both user-controlled and hydromancer-controlled vessels
4. **Multiple Tokens**: Test different token combinations

### Testing Strategy

1. **Incremental Testing**: Start with simple scenarios, add complexity
2. **Regression Testing**: Run full test suite after changes
3. **Performance Monitoring**: Track execution times and gas usage
4. **Error Analysis**: Investigate all discrepancies thoroughly

### Maintenance

1. **Regular Updates**: Keep contract addresses updated
2. **Token Updates**: Verify token denominations remain correct
3. **Gas Price Adjustment**: Update gas prices based on network conditions
4. **Dependency Updates**: Keep CosmJS and other dependencies current
