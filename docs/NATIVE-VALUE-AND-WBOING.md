# Native value on contract calls, wBOING and v10 pools

From testnet block **1150** (`BOING_TESTNET_NATIVE_VALUE_FROM`, chain 6913) a contract call can carry
native BOING, and contracts can read and pay out native balances. This is what lets a dApp show native
**BOING** in a swap: the wallet wraps it into **wBOING** (a token-v2 compatible 1:1 wrapper) inside the same
signed batch, and the swap runs against a **v10** custody pool that holds 18-decimal amounts.

Requires node **testnet-v0.1.18** or newer. Chains without an execution history (devnets, tests) have the
rules active from genesis.

## Transaction format

Two variants are appended at the end of their enums, so every older transaction encodes and hashes
exactly as before (`bincode_layout_of_value_calls_is_appended` in
`crates/boing-execution/tests/native_value.rs`):

```
TransactionPayload::ContractCallWithValue = u32 LE 20 | contract (32 B) | u64 LE len | calldata | value u128 LE
BatchCall::ContractCallWithValue          = u32 LE 2  | contract (32 B) | u64 LE len | calldata | value u128 LE
```

`value` is in base units (1 BOING = 10^18). Transactions are still signed with the v2 scheme (chain id
plus the bincode transaction), so `value` is covered by the signature. There is no new signing version.

JSON-RPC and indexers show the new variant as `kind: "contract_call"` with extra `value` (base units,
decimal string) and `value_boing` fields (`boing_getAccountTransactions`, observer).

## Execution rules

| Rule | Behavior |
|---|---|
| Order | The sender's nonce is consumed, then `value` moves from the sender to the contract, then the code runs. `SELFBALANCE` includes the value. |
| Target | The target must be a contract. Value sent to an account without code fails (use a plain `Transfer`). |
| Revert | Any fault reverts the whole transaction, the value included. The sender pays the failed-transaction fee as for any failed tx; the nonce is consumed. |
| Fee | The network fee is charged after execution. If the sender cannot pay `value + fee` the transaction fails (value refunded, failed-tx fee charged from what is left). Value is not subject to the transfer-amount fee. |
| Batch | A `ContractCallWithValue` step moves its value just before that step runs. The batch stays atomic: a later failing step refunds every earlier value. |
| Before 1150 | Mempool admission rejects value-carrying transactions (`reason: "native_value_not_active"`); in a block they fail with `NativeValueNotActive`. The new opcodes are invalid (QA `INVALID_OPCODE` at deploy, `InvalidBytecode` at run time). |
| Deploy | Deploys cannot carry value. |
| Scheduling | Value calls are contract calls, so they run on the sequential path (only plain-transfer batches run in parallel). |

## Opcodes (active from 1150)

| Opcode | Byte | Gas | Stack | Behavior |
|---|---|---|---|---|
| `BALANCE` | `0x31` | 100 | `account` → `balance` | Native balance of any account (0 if absent). |
| `CALLVALUE` | `0x34` | 2 | → `value` | The value of the current top-level value call. 0 in nested `CALL` frames, deploy init and `CREATE2` init. |
| `SELFBALANCE` | `0x47` | 5 | → `balance` | Native balance of the running contract. |
| `NATIVE_TRANSFER` | `0xf2` | 7,000 | `to` (top), `amount` → | Pays `amount` native BOING from the running contract to `to`. Faults on an insufficient balance or an amount of 2^128 or more. |

**Reentrancy.** `NATIVE_TRANSFER` only moves the balance. It never runs code at the recipient, even when
the recipient is a contract, so a payout cannot re-enter the payer. Nested `CALL` cannot carry value. A
contract cannot refuse native BOING (plain transfers and payouts always land), so contracts must not
assume `SELFBALANCE` equals what they accounted for; wBOING only requires `supply <= SELFBALANCE`.

## wBOING

`native_wrapped_boing.rs` (`wrapped_boing_bytecode()`): deposit mints 1:1, withdraw burns and pays out. It
uses the reference fungible v2 storage layout and selectors (`transfer` 0x01, `approve` 0x08,
`transfer_from` 0x09, `allowance` 0x0a), so the custody pools, the router and SDK readers treat it as any
token v2. Decimals 18.

| Selector | Calldata words | Effect |
|---|---|---|
| `0x20` `deposit` | (send value) | Mints `CALLVALUE` to the caller (faults on 0); returns the amount. |
| `0x21` `withdraw` | `amount` | Burns `amount` from the caller and pays it out in native BOING. |
| `0x22` `withdraw_to` | `amount`, `recipient` | Same, paying `recipient`. |
| `0x23` `withdraw_all` | | Burns and pays out the caller's whole balance (unwrap after a swap in one batch). |
| `0x24` `total_supply` | | Supply (always at most the contract's native balance). |
| `0x25` `balance_of` | `holder` | Balance. |
| `0x26` `decimals` | | 18. |

Value sent with any selector other than `deposit` faults (the whole call reverts and the value is
refunded). The burn happens before the payout. Logs: deposit `topic0 = "BOING_WBOING_DEPOSIT"`,
`topic1 = caller`; withdraw `topic0 = "BOING_WBOING_WITHDRAW"`, `topic1 = caller`, `topic2 = recipient`;
data = amount.

## v10 custody pool

`constant_product_pool_bytecode_v10()` (CREATE2 salt `NATIVE_CP_POOL_CREATE2_SALT_V10`) is the v9 pool (same
calldata; swap returns the output amount) with the amount bound raised from 2^64 to
2^128: every amount and reserve may be any u128 and the LP supply must stay below 2^128. The fee is taken
from the input first and all products use 256-bit words, so nothing can overflow. Min-received rounding is
unchanged: the output is floored, so a quote computed with the same formula in big-int arithmetic is exact.
v8 and v9 pools are unchanged and keep working (they still reject 18-decimal amounts above 2^64,
roughly 18.4 BOING).

Pools register in the canonical factory like any other pair. Clients tell v10 pools apart by their code.

## Swapping native BOING (one signature)

```
BOING -> token: Batch [ ContractCallWithValue(wBOING, deposit, amount),
                        ContractCall(wBOING, approve(pool, amount)),
                        ContractCall(pool, swap(...min_out)) ]
token -> BOING: Batch [ ContractCall(token, approve(pool, amount)),
                        ContractCall(pool, swap(...min_out)),
                        ContractCall(wBOING, withdraw_all) ]
```

`withdraw_all` also unwraps any wBOING the signer already held; wallets show this in the approval.

## Tests

- `cargo test -p boing-execution --test native_value`: value transfer, revert refunds, insufficient
  balance, payouts, the activation gate, batches, wBOING, v10 wrap/swap/unwrap batches, u128 reserves.
- `cargo test -p boing-node --test testnet_replay`: replays the live chain, then crosses a local
  activation height on two nodes (value calls rejected before it, wBOING deployed and used after it, both
  nodes agree).
