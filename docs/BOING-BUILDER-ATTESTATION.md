# Builder attestation registry (on-chain) — scaffold

> 👋 **Everyday users:** skip this page. Product apps will say **Builder attested**, not “registry.”
> 🛠️ **Developers:** selectors + CREATE2 salt are stubbed in `boing-execution` / `boing-sdk`; **bytecode is not assembled yet** — do not deploy.
> 🛰️ **Operators:** no live AccountId until bytecode ships and a testnet deploy lands.

## Status

| Piece | Status |
|-------|--------|
| Product + protocol design | Agent Store `docs/builder-attestations-onchain.md` |
| Selectors / salt / encoders (Rust + SDK) | **Scaffold** (this pass) |
| Registry bytecode + testnet deploy | **Not yet** |
| Soft wallet-proof finance #23 | **Do not merge** (superseded by on-chain design) |

## Design summary

- **Companions** (linked NFT↔token registry) = project structure authority.
- **Builder attestation** (this registry) = on-chain “this wallet attested as builder of this collection + token.”
- Auth: dual **asset claimer** (first `claim_asset` wins; `attest` / privileged revoke need claimer of both — finalize revoke rule with Nico).
- CREATE2 salt: `BOING_BUILDER_ATTEST_REG_V1` (`BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1`).
- Selectors (per-contract): `0xE0`–`0xE6` — claim / attest / revoke_at / count / get_at / get_claimer / transfer_claimer.
- Slot: collection, token, builder, note_hash (optional zeros). Mutability: append + tombstone revoke. Max **4096** slots.

## Scaffold API surface

| Crate / package | Symbols |
|-----------------|---------|
| `boing-execution` | `builder_attestation_registry` — salts, topics, storage keys, encode helpers |
| `boing-sdk` | `builderAttestationRegistry.ts` — matching selectors + encoders + decode helpers |

No `DEFAULT_*_BYTECODE_HEX` and no deploy helper until the assembler lands.

## Related

- Companions: [BOING-LINKED-NFT-TOKEN.md](BOING-LINKED-NFT-TOKEN.md)
- Rust: `crates/boing-execution/src/builder_attestation_registry.rs`
- SDK: `boing-sdk/src/builderAttestationRegistry.ts`
