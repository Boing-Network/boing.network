# Optional fees + priority tips (fee market v1 draft)

**Status:** draft protocol change (this PR). Operators enable free-by-default via `network_fee_config.json`.

## Goal

- **Default path:** end users can be included with **no required execution gas fee** (zero effective gas charge).
- **Optional path:** attach a **priority tip** for preferred mempool / block ordering under congestion.
- **Anti-spam:** free txs cannot trivially DoS the mempool.

Blob data fees (`blob_base_fee_per_kib`) and Forever pin fees (`blob_forever_pin_fee_per_kib`) stay **independent** — they are never waived by the execution-gas multiplier.

## Policy knobs (`NetworkFeePolicy`)

| Field | Default | Meaning |
|-------|---------|---------|
| `execution_gas_fee_bps` | `10000` | Share of the historical gas schedule that is charged. `0` = waive execution gas (free-by-default). |
| `free_tx_min_balance` | `0` | Minimum sender balance to admit a **zero-tip** tx when gas is waived. |
| `extra_fixed_fee` / `transfer_amount_bps` | `0` | Unchanged; still charged when set. |
| blob / pin fees | `0` | Unchanged; independent of `execution_gas_fee_bps`. |

Helper: `NetworkFeePolicy::optional_fees_default()` sets `execution_gas_fee_bps = 0`.

Example `network_fee_config.json`:

```json
{
  "fee_validators_bps": 0,
  "fee_treasury_bps": 10000,
  "fee_burn_bps": 0,
  "execution_gas_fee_bps": 0,
  "free_tx_min_balance": "1000000000000000",
  "blob_base_fee_per_kib": "0",
  "blob_forever_pin_fee_per_kib": "0"
}
```

RPC: `boing_getNetworkFeePolicy` exposes the new fields plus `execution_gas_waived` / `optional_priority_tips`.

## Priority tip (wire + signing)

- `SignedTransaction.priority_tip` (ledger units). **Omitted when zero** so historical `(tx, signature)` bincode stays byte-identical.
- Decode via `SignedTransaction::from_bincode` (RPC submit / gossip).
- Tip `0`: existing v2 signable preimage (`BOING_TX_V2`).
- Tip `> 0`: `BOING_TX_PRIORITY` domain binds `tip` into the signature.
- Blocks carry optional `priority_tips: Vec<u128>` (empty = all zero; omitted when empty). Producers omit an all-zero vector. Importers verify tip-aware signatures and charge tips on execute.

Tips are paid **100% to the block proposer** (not split by fee BPS).

## Mempool / anti-spam

When `execution_gas_fee_bps == 0` (node sets `mempool.set_optional_fees_active(true)`):

1. **Free pending cap:** at most `DEFAULT_MAX_FREE_PENDING_PER_SENDER` (4) zero-tip pending txs per sender (tipped txs still use the normal per-sender cap of 16).
2. **Balance gate:** reject zero-tip admits when `balance < free_tx_min_balance` (if configured).
3. **Ordering:** `drain_for_block` sorts by tip descending, then nonce, then sender.
4. **Eviction:** when the global pool is full, prefer dropping the lowest-tip (then highest-nonce) tx of the busiest other sender.

Existing global cap (`BOING_MEMPOOL_MAX_TXS`) and per-sender pending limits still apply.

### Tradeoffs

| Choice | Pros | Cons |
|--------|------|------|
| Waive gas via policy BPS (not a hard fork of tx bodies) | Replay-safe; operators opt in | Must coordinate policy on all validators |
| Tip sidecar (not inside `Transaction`) | `tx.id()` / historical bodies unchanged | Tip verified via signature + block vector, not tx root |
| Free pending cap + min balance | Fits current mempool design; cheap | Not a full stake-weighted quota; sophisticated spam still needs ops monitoring |
| Blob fees separate | Media congestion stays priced | Users publishing blobs still pay even when execution gas is free |

## SDK

- `TransactionInput.priorityTip?: bigint`
- `withPriorityTip(tx, tip)`, `signableTransactionHashWithTip`, `TX_SIGNING_DOMAIN_PRIORITY`
- Zero tip keeps existing encode/sign paths.

## Later (not in this draft)

- Height-gated activation era in `ExecutionHistory` for networks that must flip free-default at a fixed height without a file edit.
- Tip share split / burn (today: 100% proposer).
- Per-account free-tx rate across committed blocks (durable counters).
- Wallet UX surfacing tip suggestions from congestion signals.
