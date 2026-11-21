# Boing ↔ Base inventory-free bridge (testnet-first)

**Status:** Protocol txs + Base Sepolia contracts shipped in code. **Not live** on public testnet until operators set activation + deploy keys.  
**Preference:** No Boing-held market inventory · ~**0.5%** platform fee · Base-first · validator **⌈2W/3⌉** stake attestation.

## What this is

| Direction | Source action | Destination action |
|-----------|---------------|--------------------|
| Boing → Base | `BridgeLockToBase` (index **41**) locks native BOING in escrow (fee → treasury) | Validators ECDSA-sign; Base `BoingBridgeVault.mintWithAttestation` mints **bBOING** |
| Base → Boing | User `burnToBoing` on Base vault | Validators Ed25519-sign; `BridgeClaimFromBase` (index **42**) unlocks escrow to recipient |

Boing does **not** pre-fund Base. Bridged supply is backed by escrowed native BOING.

## Activation (coordinated)

```bash
# Example only — H must be above tip; every validator/RPC node must agree
export BOING_BRIDGE_ACTIVATION_HEIGHT=H
# or: --bridge-activation-height H
```

Unset / `never` / `off` → **never admitted** (safe default on chain 6913 today).

Discovery: `boing_getNetworkInfo` / `boing_health` → `bridge_activation_height` (`null` = never).

## Wire / crypto

- Lock fee: `BRIDGE_FEE_BPS` = **50** (0.5%) to `PROTOCOL_TREASURY`; net escrowed under `bridge_escrow_account()`.
- Claim quorum: Ed25519 attestations over `boing.bridge.burn.v1`; signed bonded stake ≥ ⌈2W/3⌉ of **all** bonded stake.
- Base mint quorum: ECDSA over eth-signed mint digest; vault stake weights mirrored from Boing validators.

## Operator checklist (Nico)

1. Deploy Base Sepolia vault (`boing.finance/contracts/bridge` — see finance `docs/bridge.md`).
2. Register secp256k1 attestor addresses + stake weights on the vault.
3. Run attestor (`scripts/bridge/attest-relayer.mjs`) with validator Ed25519 + ECDSA keys.
4. Set `BOING_BRIDGE_ACTIVATION_HEIGHT` on **all** testnet nodes and restart.
5. Wire Finance env: vault + bBOING addresses (testnet only).

## Refs

- Finance contracts: `boing.finance/contracts/bridge/`
- Primitives: `crates/boing-primitives/src/bridge.rs`
- Execution: `Vm::execute_bridge_lock_to_base` / `execute_bridge_claim_from_base`
- Project pack: Project store `docs/boing-base-bridge-implementation.md`
