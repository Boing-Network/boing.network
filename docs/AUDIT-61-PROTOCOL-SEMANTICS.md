# Audit #61 item 29 — protocol-invasive DEX / token semantics

**Status (batch 10):** first-mint `√` shipped as **pool v11** (new CREATE2 template). JumpI /
amount-range / bond-debit leftovers from item 29 shipped in
[#147](https://github.com/Boing-Network/boing.network/pull/147). XOR allowance keys and custody
intermediate `min_out` remain as documented below (hash opcode still activation-gated scaffolding).

## 1. First LP mint: `min(A, B)` (v8–v10) vs `√(A·B)` (v11)

**Where (legacy):** `constant_product_pool_bytecode_v8`…`v10` (`native_amm_v8.rs` first-mint path
`p.min(A, B, LP)`); CLI/SDK `quote_lp_minted` mirrors that.

**Shipped:** `constant_product_pool_bytecode_v11` + `NATIVE_CP_POOL_CREATE2_SALT_V11` mint
`⌊√(amount_a · amount_b)⌋` via an in-bytecode Babylonian loop (no new VM opcode). Host helper
`isqrt_u128` / CLI `quote_lp_minted_v11` match the template. Deployed v8–v10 pools stay frozen.

**Activation:** new CREATE2 deployments only — no chain activation height required unless a network
preset flips the default factory salt/version.

## 2. Allowance keys truncate addresses to 16 bytes and XOR

**Where:** reference fungible v2 / wBOING (`ref_fungible_allowance_key`:
`(owner[0..16] ‖ spender[0..16]) ^ REF_FUNGIBLE_ALLOWANCE_XOR`).

**Why not change in place:** The VM has **no general-purpose hash opcode** (no BLAKE3/Keccak in
contract code). Full 32-byte `(owner ‖ spender)` hashing therefore needs either a **VM opcode /
hard-fork activation** or a multi-slot layout redesign in a **new token template**. Truncation still
requires ~2^128 work to collide on a chosen owner half while the owner half of `approve` is always
`CALLER`.

**Scaffolding (batch 10):** `crates/boing-execution/src/hash_opcode_scaffold.rs` — planned
`Blake3` opcode byte `0x20`, env `BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT` (default never), and
host-side `ref_fungible_allowance_key_v3_host` (= `BLAKE3(domain ‖ owner ‖ spender)`) for a future
fungible v3 template. Opcode is **not** in the live ISA until a coordinated hard-fork PR.

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
| First LP `min` vs `√` | New pool **v11** | **Shipped** (batch 10) |
| XOR 16-byte allowance keys | Needs hash opcode **or** new token template | Scaffold + activation plan |
| Custody intermediate `min_out=1` | By design | Keep; document here |

Refs: https://github.com/Boing-Network/boing.network/issues/61
