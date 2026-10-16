# LP vault v2 (custody pools v8 / v9)

Source: `crates/boing-execution/src/native_lp_vault_v2.rs` · SDK: `boing-sdk/src/boingLpVaultV2.ts` · script: `scripts/lp-vault-v2-testnet.mjs`

The v1 vault ([NATIVE-AMM-LP-VAULT.md](NATIVE-AMM-LP-VAULT.md)) drives ledger-only v1..v7 pools, is configure-once with a separate share token, and has no withdraw. **v2** works with the token-custody pools (v8/v9, reference-token v2 `transfer_from`) and supports withdraw.

## Public testnet

| | AccountId |
|---|---|
| LP vault v2 (USDC/BOING) | `0x236a851dd50faaa268e2e1b7ba95cfb9993773abf63a4c0d949183d8302e65c7` |
| Pool (v9) | `0x2ad75c2e11b1e93761f4f8df39502deecbe7148324e5a80a951798142b2c5b61` |
| token A / B | USDC `0x6676f971…5e93` / BOING `0x6fce9d3e…9c6b` |

SDK constant `CANONICAL_BOING_TESTNET_LP_VAULT_V2_HEX`; manifest key `lp_vault_v2_usdc_boing`. Txs are listed in [NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md).

## Selectors

| Selector | Words after selector | Notes |
|---|---|---|
| `0xC0` configure | `pool`, `token_a`, `token_b` | once; all non-zero, tokens distinct. Anyone can call it first, so deploy and configure back-to-back and verify the stored config (`getBoingLpVaultV2State`). |
| `0xC1` deposit | `amount_a`, `amount_b`, `min_shares`, `deadline` | returns shares minted |
| `0xC2` withdraw | `shares`, `min_a`, `min_b`, `deadline` | pool pays the caller |
| `0xC3` transfer_shares | `to`, `amount` | `to == caller` is a no-op |

Storage: configured `k[31]=0xe1`, pool `0xe2`, token A `0xe3`, token B `0xe4`, total shares `0xe5`, lock `0xe6`; share balance at `holder XOR "BOING_LP_VAULT_V2_SHARES"`.

## Flow

**Deposit** (user approves the **vault** for exactly `amount_a` / `amount_b`):
1. `transfer_from(caller → vault)` both amounts;
2. `approve(pool, amount)` on each token, then `pool.add_liquidity(amount_a, amount_b, min_shares, deadline)`; the pool pulls exactly those amounts, so the vault ends with **no tokens and no allowance**;
3. shares minted = LP returned by the pool (**1:1**; the vault's pool LP balance always equals total shares). Swap fees accrue to pool reserves, so to shares.

The pool consumes both amounts even if they are off-ratio (excess is donated to all LPs, same as a direct `add_liquidity`). Use `quoteBoingLpVaultV2Deposit` (balanced `amountB` from live reserves + `minShares`).

**Withdraw:** shares are burned **before** the external call, then `pool.remove_liquidity_to(shares, min_a, min_b, caller, caller, deadline)` pays the user directly.

## Safety properties (tested in `native_lp_vault_v2::tests`)

- Every rejection is a VM fault, so the whole transaction reverts: missing approvals (including a partial one), `min_shares` / `min_a` / `min_b`, deadline, zero amounts, over-withdraw, unknown selector, re-configure.
- Reentrancy lock; it is released after every revert.
- Self-transfer can't mint (no-op); recipient balance is read after the sender debit; zero recipient rejected.
- A vault configured with the wrong tokens can't take funds: the pool pulls its own tokens from the vault, which holds none, so the deposit faults.
- Works with v8 and v9 pools.

## Script

```bash
node scripts/lp-vault-v2-testnet.mjs state                       # BOING_LP_VAULT_V2_HEX=0x236a…65c7
node scripts/lp-vault-v2-testnet.mjs deposit --amount-a 50       # dry run (signs + simulates the first tx)
BOING_SECRET_HEX=… node scripts/lp-vault-v2-testnet.mjs deposit --amount-a 50 --send
BOING_SECRET_HEX=… node scripts/lp-vault-v2-testnet.mjs withdraw --shares 20 --send
```