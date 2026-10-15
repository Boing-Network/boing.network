# Native batch transactions (`TransactionPayload::Batch`)

**Status:** implemented in `boing-primitives` / `boing-execution` / `boing-node` / `boing-sdk`.
**Not live until validators and RPC nodes are redeployed** with this release. Older nodes cannot
decode payload variant 9 and will reject the tx at deserialization.

## What it is

One signed transaction, one nonce, up to **16 steps**, applied **in order and atomically**:

| Step | Effect |
|------|--------|
| `BatchCall::Transfer { to, amount }` | Native BOING transfer from the sender (creates `to` if missing). |
| `BatchCall::ContractCall { contract, calldata }` | Runs the contract with `CALLER` = tx sender — identical semantics to a standalone `ContractCall`, so fungible-token `transfer` and NFT transfer calldata work unchanged. |

If **any** step fails (insufficient balance, missing contract, contract revert/out of gas, …), the
block executor reverts the whole tx to its pre-tx checkpoint. As with every failed tx, the nonce is
still consumed and the receipt is `success: false` with an error like
`Batch step 2 failed: Insufficient balance (whole batch reverted)`.

No new runtime, no EVM/Solana code, no change to existing contract bytecode.

## Wire format (bincode, appended variant → old txs unchanged)

`TransactionPayload` discriminant **9** = `Batch { calls: Vec<BatchCall> }`:

```
u32 LE 9 | u64 LE n | n × BatchCall
BatchCall::Transfer      = u32 LE 0 | to (32 B) | amount u128 LE
BatchCall::ContractCall  = u32 LE 1 | contract (32 B) | u64 LE len | calldata
```

Variants 0–8 keep their indices, so every previously signed tx encodes and hashes identically.
Cross-checked by `crates/boing-execution/tests/batch_tx.rs::bincode_layout_is_backward_compatible`
and `boing-sdk/tests/nativeBatchTx.test.ts` (same byte vector).

## Limits (consensus rules — checked at mempool admission **and** in the VM)

| Limit | Value |
|-------|-------|
| Steps per batch | 1 – `MAX_BATCH_CALLS` = 16 |
| Summed calldata of contract steps | `MAX_BATCH_CALLDATA_BYTES` = 64 KiB |
| Sum of transfer amounts | must fit `u128` |
| Interpreter gas for all contract steps together | `GAS_BATCH_CONTRACT_BUDGET` = one `ContractCall` budget (40M) |

The shared VM budget means a batch can never do more contract work than a single contract call.

## Gas and fees

| Component | Gas |
|-----------|-----|
| Envelope (`GAS_BATCH_BASE`) | 21 000 (one transfer) |
| Each transfer step (`GAS_PER_BATCH_TRANSFER`) | 7 000 |
| Each contract step (`GAS_PER_BATCH_CONTRACT_CALL_STEP`) | 7 000 + interpreter gas used |

Fee = `network_fee_amount(gas)` as usual (21 000 gas = 1 BOING). Example: 16 BOING transfers =
133 000 gas ≈ 7 BOING vs 16 BOING as 16 separate transfers. The policy's `extra_fixed_fee` is
charged **once per tx**; the optional `transfer_amount_bps` fee is applied to the **sum** of all
transfer steps so batching cannot dodge it.

## Parallel scheduling

`Transaction::suggested_parallel_access_list` returns sender + every recipient/contract (deduped)
for both read and write. Batches always execute on the sequential path (only plain `Transfer`
batches use the parallel rayon path).

## SDK

```ts
import { buildBatchTransaction, signTransactionInput } from 'boing-sdk';

const tx = buildBatchTransaction({
  nonce,
  senderHex,
  calls: [
    { kind: 'transfer', toHex: bob, amount: 10n ** 18n },
    { kind: 'contractCall', contractHex: token, calldata: encodeTokenTransfer(carol, 500n) },
  ],
}); // access list auto-filled
const signedHex = await signTransactionInput(tx, secretKey);
```

`validateBatchCalls`, `MAX_BATCH_CALLS`, `MAX_BATCH_CALLDATA_BYTES` and `PayloadVariant.Batch`
are exported for UI pre-validation.

## Capability detection

Batch-capable nodes advertise it in `boing_health.rpc_surface`:
`tx_payload_batch: true`, `tx_batch_max_calls`, `tx_batch_max_calldata_bytes`. Older nodes omit
these keys. SDK: `nodeSupportsNativeBatch(await client.health())` / `nodeBatchMaxCalls(health)`.
Boing Express uses this to pick batch vs sequential bulk sends.

## Known gaps / follow-ups

- **Receipt log attribution:** logs from several contracts are concatenated; RPC's
  `log_emitting_account` returns no single address for a batch. Indexers that rely on the
  per-receipt emitting address should decode logs by topic, or a later change can add
  per-log addresses.
- **`return_data`** is the last contract step's return data.
- Contract deploys, staking and QA votes are intentionally **not** allowed inside a batch.
- **Rollout:** this is a consensus change (new payload variant + new state transition). All
  validators must upgrade before any batch is included; plan a coordinated testnet restart or
  activation height.
