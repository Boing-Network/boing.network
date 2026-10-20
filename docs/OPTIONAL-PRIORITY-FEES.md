# Low base fee + optional priority tips (fee market v1)

**Status:** shipped on main via [#58](https://github.com/Boing-Network/boing.network/pull/58); product direction refined to **micro base fee** (not free-by-default) — see follow-up PR.

## Goal

- **Always charge a small base execution fee** (Solana-like micro-fees) so spam stays economically costly.
- **Optional `priority_tip`** for preferred mempool / block ordering under congestion.
- Blob data / Forever pin fees stay **independent** of the execution-gas BPS.

Fully waived execution gas (`execution_gas_fee_bps = 0`) remains technically supported for experiments but is **not** the recommended product default.

## Magnitude (recommended testnet / operator default)

| Schedule | `execution_gas_fee_bps` | 21k-gas transfer | ~380k-gas deploy |
|----------|-------------------------|------------------|------------------|
| Historical full | `10000` | **0.021 BOING** | ~0.38 BOING |
| **Micro (recommended)** | **`100`** ([`MICRO_FEE_EXECUTION_GAS_BPS`](../crates/boing-tokenomics/src/fee_policy.rs)) | **0.00021 BOING** | ~0.0038 BOING |

Helper: `NetworkFeePolicy::micro_fee_default()`.

Example `network_fee_config.json`:

```json
{
  "fee_validators_bps": 0,
  "fee_treasury_bps": 10000,
  "fee_burn_bps": 0,
  "execution_gas_fee_bps": 100,
  "blob_base_fee_per_kib": "0",
  "blob_forever_pin_fee_per_kib": "0"
}
```

## Height gate

On networks with `ExecutionHistory` (public testnet), reduced BPS is ignored until **`micro_fees_from`**:

- Env / CLI: `BOING_MICRO_FEES_ACTIVATION_HEIGHT` / `--micro-fees-activation-height`
- Below that height: execution gas is always the full historical schedule (replay-safe)
- At/after: honor `execution_gas_fee_bps` (including micro `100` or experimental `0`)
- New chains / no history: honor policy from genesis

## Priority tip

- `SignedTransaction.priority_tip` (omitted on the wire when `0`)
- Tip `> 0` uses signing domain `BOING_TX_PRIORITY`
- Blocks carry optional `priority_tips`
- Tips pay **100% to the block proposer** (today)

## Mempool

- **Always** tip-descending drain + lowest-tip eviction preference
- Existing global / per-sender pending caps remain the structural limits
- Legacy free-pending cap + `free_tx_min_balance` apply **only** if someone sets `execution_gas_fee_bps = 0` after the micro-fee height — not the main spam story

## SDK

- `TransactionInput.priorityTip`, `withPriorityTip`, `signableTransactionHashWithTip`
- `MICRO_FEE_EXECUTION_GAS_BPS`, `estimateExecutionGasFee`, `suggestPriorityTipLadder`

## Later

- Tip share split / burn knobs
- Richer wallet congestion signals for tip suggestions
