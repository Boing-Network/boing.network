# Audit #61 item 29 — protocol-invasive DEX / token semantics

**Status:** documented deferral (batch 9). JumpI / amount-range / bond-debit leftovers from item 29
shipped in [#147](https://github.com/Boing-Network/boing.network/pull/147). The three items below
touch **deployed template semantics** and are intentionally unchanged on live bytecode.

## 1. First LP mint uses `min(A, B)` instead of `√(A·B)`

**Where:** `constant_product_pool_bytecode_v8`…`v10` (`native_amm_v8.rs` first-mint path
`p.min(A, B, LP)`); CLI/SDK `quote_lp_minted` mirrors that.

**Why not change in place:** Deployed pools are immutable. Rewriting first-mint math would desync
off-chain quotes and LP accounting for every existing v7–v10 pool. The Boing VM also has **no
`Sqrt` opcode**; an integer Newton `√` would belong in a **new pool template version** (for example
v11), not a hard fork, and only for **new** CREATE2 deployments.

**Activation path (future):** ship a new `constant_product_pool_bytecode_vN` + CREATE2 salt, update
`quote_lp_minted` / SDK helpers behind that version, keep v8–v10 behavior frozen. No chain
activation height is required unless a network preset forces a default-version flip.

## 2. Allowance keys truncate addresses to 16 bytes and XOR

**Where:** reference fungible v2 / wBOING (`ref_fungible_allowance_key`:
`(owner[0..16] ‖ spender[0..16]) ^ REF_FUNGIBLE_ALLOWANCE_XOR`).

**Why not change in place:** The VM has **no general-purpose hash opcode** (no BLAKE3/Keccak in
contract code). Full 32-byte `(owner ‖ spender)` hashing therefore needs either a **VM opcode /
hard-fork activation** or a multi-slot layout redesign in a **new token template**. Truncation still
requires ~2^128 work to collide on a chosen owner half while the owner half of `approve` is always
`CALLER`.

**Activation path (future):** prefer a new fungible template version once a hash opcode (or an
agreed multi-slot scheme) exists; do not mutate v2 storage layout under live tokens.

## 3. Custody router intermediate hops use `min_out = 1`

**Where:** `native_dex_custody_router` — intermediate hops store `min = 1`; the **last** hop copies
the user `min_out`.

**Why this is by design:** Intermediate amount_out is unknown until the previous pool returns.
Enforcing the user’s final slippage on every hop would over-reject routes. A returned `0` still
faults. User slippage is enforced on the **final** hop only. See
[NATIVE-DEX-CUSTODY-ROUTER.md](NATIVE-DEX-CUSTODY-ROUTER.md).

**No hard fork / activation:** changing intermediate `min_out` would be a new router template
revision if product ever wants hop-level floors; not required for correctness of the current design.

## Summary

| Finding | Safe without hard fork? | Action |
|---------|-------------------------|--------|
| First LP `min` vs `√` | New pool **version** only | Deferred; document here |
| XOR 16-byte allowance keys | Needs hash opcode **or** new token template | Deferred; document here |
| Custody intermediate `min_out=1` | By design | Keep; document here |

Refs: https://github.com/Boing-Network/boing.network/issues/61
