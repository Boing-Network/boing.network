# Low base fee + optional priority tips (fee market v1)

**Status:** shipped on main ([#58](https://github.com/Boing-Network/boing.network/pull/58), [#59](https://github.com/Boing-Network/boing.network/pull/59)). Product direction: **Solana-like micro base fee + optional priority tip** (not gasless).

## Goal

- **Always charge a small base execution fee** so spam stays economically costly.
- **Optional `priority_tip`** for preferred mempool / block ordering under congestion.
- Blob data / Forever pin fees stay **independent** of the execution-gas BPS.

Fully waived execution gas (`execution_gas_fee_bps = 0`) remains technically supported for experiments but is **not** the recommended product default.

## Magnitude (recommended testnet / operator default)

| Schedule | `execution_gas_fee_bps` | 21k-gas transfer | ~380k-gas deploy |
|----------|-------------------------|------------------|------------------|
| Historical full | `10000` | **0.021 BOING** | ~0.38 BOING |
| **Micro (recommended)** | **`100`** ([`MICRO_FEE_EXECUTION_GAS_BPS`](../crates/boing-tokenomics/src/fee_policy.rs)) | **0.00021 BOING** | ~0.0038 BOING |

Helper: `NetworkFeePolicy::micro_fee_default()`.

Example `network_fee_config.json` (also [`docs/config/network_fee_config.canonical.json`](config/network_fee_config.canonical.json)):

```json
{
  "fee_validators_bps": 0,
  "fee_treasury_bps": 10000,
  "fee_burn_bps": 0,
  "execution_gas_fee_bps": 100,
  "tip_proposer_bps": 10000,
  "tip_treasury_bps": 0,
  "tip_burn_bps": 0,
  "blob_base_fee_per_kib": "0",
  "blob_forever_pin_fee_per_kib": "0"
}
```

Apply live with `boing_operatorApplyFeePolicy` (operator auth when configured) or load via `--network-fee-config`.

## Operator / testnet enablement

1. **Ship a binary** that includes fee market v1 + micro-fee height gate ([PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md)).
2. **Set activation height** on history networks (public testnet) so reduced BPS is not applied during replay:
   - Env: `BOING_MICRO_FEES_ACTIVATION_HEIGHT=<height>`
   - CLI: `--micro-fees-activation-height <height>`
   - Template: [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example)
   - Below that height: full historical gas. At/after (or no history / new chain): honor `execution_gas_fee_bps`.
3. **Apply micro fee policy** — write `execution_gas_fee_bps: 100` into `network_fee_config.json`, call `boing_operatorApplyFeePolicy`, or run **`npm run apply-micro-fee-policy`** (loads the canonical JSON).
4. **Verify** — `boing_getNetworkFeePolicy` should show `execution_gas_fee_bps: 100`, `micro_fee_execution_gas_bps: 100`, and tip BPS fields.

Fresh / private chains with no `ExecutionHistory`: micro BPS applies from genesis once the policy is loaded (no height gate needed).

## Priority tip

- `SignedTransaction.priority_tip` (omitted on the wire when `0`)
- Tip `> 0` uses signing domain `BOING_TX_PRIORITY`
- Blocks carry optional `priority_tips`
- Tip split (governance-tunable; defaults **100% proposer**):
  - `tip_proposer_bps` → block proposer
  - `tip_treasury_bps` → `PROTOCOL_TREASURY`
  - `tip_burn_bps` → `FEE_BURN_SINK`
  - Must sum to `10000`. Rounding remainder → proposer.

## Mempool

- Block building drains **per-sender nonce-contiguous runs** (a sender's txs always execute in nonce order), choosing the highest tip among each sender's next-ready tx first; lowest-tip eviction preference when the pool is full
- Wire format and signing domain: [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md) (decode with `SignedTransaction::from_bincode`)
- Existing global / per-sender pending caps remain the structural limits
- Legacy free-pending cap + `free_tx_min_balance` apply **only** if someone sets `execution_gas_fee_bps = 0` after the micro-fee height — not the main spam story

## SDK / wallet UX

- `TransactionInput.priorityTip`, `withPriorityTip`, `signableTransactionHashWithTip`
- `MICRO_FEE_EXECUTION_GAS_BPS`, `estimateExecutionGasFee`, `estimateMicroTransferFee`, `suggestPriorityTipLadder`, `estimateTotalFeeWithTip`, `formatTipLadderForUi`
- Ladder (base units): `normal=0`, `faster=1×micro`, `priority=10×micro`, `urgent=100×micro`
- Express wallet integration notes: [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) § Priority tips

## Later

- Richer wallet congestion signals for tip suggestions (mempool depth / pending tip percentiles)
