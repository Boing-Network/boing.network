# Native BOING denomination: 18 decimals

Approved 2026-10-17 (mainnet plan step 1). **1 BOING = 10^18 base units.** Every native amount on the
ledger (balances, stake, pending unbond, open-validator bonds, transfer and bond amounts, fees, emission)
is an integer number of base units. Amounts travel as `u128` on chain and as **decimal strings** over RPC.

## When it applies

| Chain | Ledger unit |
|-------|-------------|
| New chains, local dev, tests, mainnet (no network history, or `base_units_from = 0`) | base units from genesis |
| Public testnet 6913 | whole BOING below height **1050**, base units from **1050** |

Testnet 6913 switched in place with a height-gated migration (`BOING_TESTNET_BASE_UNITS_FROM = 1050`
in `crates/boing-node/src/network_preset.rs`), so history and replay from genesis stay intact:

- At the start of block **1050**, before its transactions, `BlockExecutor::prepare_state_for_height` calls
  `StateStore::redenominate(10^18)`: every account's `balance`, `stake` and `pending_unbond`, and every
  open-validator record's `bond`, is multiplied by 10^18 (saturating; the largest testnet balance is far
  below `u128::MAX / 10^18 ≈ 3.4 × 10^20`). Nothing else holds native BOING: the VM has no native-value
  opcodes (contracts, pools and routers cannot hold native BOING), the name registry stores no amounts,
  and QA pending records hold none.
- It runs exactly once: only the state of height 1049 is ever migrated (producer and every import path go
  through `execute_block`; a re-execution of 1050 starts again from the 1049 state). Tests:
  `redenomination_multiplies_exactly_once_at_h` (executor), `redenominate_scales_every_native_amount_once_and_is_journaled`
  (state), `replay_then_cross_redenomination_locally` (replays the recorded testnet, then crosses a local
  migration height on two nodes with identical roots, checks chain-wide sums before/after and that the
  next block does not multiply again).
- Blocks below 1050 keep whole-BOING amounts forever. Explorers must format a historical block's amounts
  with the unit of *its* height (`native_currency.base_units_from_height` in `boing_getNetworkInfo`).
- A transaction is interpreted in the unit of the block that includes it. Admission and simulation for
  the next block apply the migration to their state copy, so a tx admitted at tip 1049 is checked in base
  units. Wallets should not sign native amounts right around the switch (the faucet makes one block per tx,
  so this only matters for a tx that waits in the mempool across 1050).

## Fees and gas price

```text
from base units:  network_fee = gas_used × GAS_PRICE_BASE_UNITS + extra_fixed_fee + transfer_amount × transfer_amount_bps / 10_000
GAS_PRICE_BASE_UNITS = 10^12 base units per gas = 0.000001 BOING/gas
```

| Tx | Gas | Fee |
|----|-----|-----|
| Transfer | 21,000 | 0.021 BOING |
| Failed tx (`FAILED_TX_GAS`) | 21,000 | 0.021 BOING |
| Contract call (typical) | 30k–150k | 0.03–0.15 BOING |
| Contract deploy | 200k–400k | 0.2–0.4 BOING |

Fees are exact (no rounding up to a whole BOING). The old whole-unit rule
`ceil(gas_used / 21_000)` BOING (a transfer cost 1 BOING, a deploy 10–20) still applies to blocks below
1050 on testnet. The price is about 50× cheaper than the old rule for a transfer while keeping spam cost
non-trivial (1M transfers ≈ 21,000 BOING); it is a single constant in `boing-tokenomics` and can be
revisited with a height gate. `extra_fixed_fee` in `network_fee_config.json` is a ledger amount
(base units from the switch; it is 0 on testnet).

## Protocol prices (configured in whole BOING, charged in ledger units)

Configuration stays human-readable; the executor scales it by the ledger unit of the block
(`Denomination::scale`):

| Price | Value | Charged from the switch |
|-------|-------|-------------------------|
| Open-validator bond (`BOING_OPEN_VALIDATOR_BOND`) | 100 BOING | 100 × 10^18 |
| Name rent per year (3 / 4 / 5 / 6+ chars) | 10 / 5 / 2 / 1 BOING | × 10^18 |
| Faucet dispense | 20 BOING | 20 × 10^18 |
| QA pool `reward_per_counted_vote`, `min_voter_stake` | whole BOING | × 10^18 |
| Stake mode `min_stake` | whole BOING | × 10^18 |

## Emission

From the switch the proposer emission is computed directly in base units with integer math
(`block_emission_validators_in`): about 3.55 BOING per block in year 1, decaying 15%/year, and it no
longer rounds down to a whole number of BOING. Multiplying the old rounded value (3) by 10^18 was
rejected: it would keep the whole-unit truncation and its zero floor (review finding #8). Below 1050
the historical whole-unit emission replays unchanged.

## RPC

- Amounts stay **decimal strings** in ledger units: `boing_getBalance`, `boing_getAccount`, account
  transactions, validator bonds, faucet. New formatted helpers: `balance_boing`, `stake_boing`,
  `pending_unbond_boing`, `decimals` (`boing_getAccount` / `boing_getBalance`), `bond_boing`
  (`boing_getValidators`), `rent_per_year_boing` (`boing_checkNames`), `amount_boing` (faucet).
- `boing_getNetworkInfo.native_currency` and `boing_health.native_currency`:
  `{ symbol, decimals, base_unit_decimals: 18, base_units_per_boing, base_units_from_height, gas_price_base_units }`.
  `decimals` is the unit of the amounts the node returns now (0 before the switch, 18 from it).
- `boing_simulateTransaction` returns `network_fee` (ledger units, for the next block) and `network_fee_decimals`.
- Block JSON (`boing_getBlockByHeight` / `boing_getBlockByHash`): any integer above 2^53 − 1 is emitted
  as a decimal string so JavaScript never rounds it (transfer amounts in base units are ~10^18+).

## Tokens (reference fungible v1/v2/secured, LP shares)

Unchanged. Token balances live in contract storage with the token's own decimals metadata (the reference
templates are whole units and report `decimals = 0`). The native switch does not touch token math, and
no pool holds native BOING (the VM cannot move native value into a contract), so pool reserves, LP shares
and swap quotes are unaffected.

## SDK and apps

`boing-sdk` exports `BOING_DECIMALS`, `parseBoing` / `formatBoing` (bigint base units) and the
height-aware `nativeDecimalsAt` / `formatNativeAmount`, plus `feeForGasBaseUnits`. Boing Express, boing.observer and boing.finance format native amounts with
`native_currency` from the node, so they show whole BOING before the switch and 18-decimal amounts after.
Each app carries its own small helper with the same rules (Express `src/boing/nativeUnit.ts`, observer
`src/lib/native-unit.ts`, finance `frontend/src/utils/boingNativeAmount.js`) instead of bumping its pinned SDK.

## Public testnet activation (2026-10-17)

Both public nodes ran testnet-v0.1.17 (cc9d451) from block ~942 and crossed 1050 together (block 1050
hash `86678e58…`, identical on both). Checked live after the switch: a faucet recipient funded with 20
whole BOING at block 1003 reads `20000000000000000000` (`balance_boing` "20", `decimals` 18); the faucet
transfers in blocks 1049 and 1050 carry `20` and `20000000000000000000`; each 21,000-gas transfer from
1050 on costs 0.021 BOING; the open-validator bond reads `100000000000000000000` (`bond_boing` "100").
The recorded blocks 942..=1052 are in `crates/boing-node/tests/fixtures/testnet_blocks_after_942.jsonl`
and replay with exact hashes (`replay_live_chain_across_v4_and_redenomination`).
