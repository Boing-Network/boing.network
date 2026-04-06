# Native DEX multihop swap router (Boing VM)

Bytecode: `native_dex_multihop_swap_router_bytecode()` in `crates/boing-execution/src/native_dex_multihop_swap_router.rs` (alias: `native_dex_swap2_router_bytecode()`).

## Purpose

Execute **2–4** native constant-product **`swap`** / **`swap_to`** calls **in one transaction**. Each hop uses the same **`Call`** pattern as [NATIVE-DEX-LEDGER-ROUTER.md](./NATIVE-DEX-LEDGER-ROUTER.md); the pool sees **`Caller` = this router** for every hop.

## Calldata

| Selector (word0 low byte) | Hops | Outer size | Inner size per hop |
|---------------------------|------|------------|---------------------|
| **`0xE5`** | 2 | **352** bytes | **128** |
| **`0xE6`** | 2 | **416** bytes | **160** |
| **`0xE7`** | 3 | **512** bytes | **128** |
| **`0xE8`** | 3 | **608** bytes | **160** |
| **`0xE9`** | 4 | **672** bytes | **128** |
| **`0xEA`** | 4 | **800** bytes | **160** |

Layout: word0 = selector; then for each hop, one **pool** word (32 bytes) and one **inner** calldata slice (**128** or **160** bytes).

Rust: `encode_swap2_router_calldata_*`, `encode_swap3_router_calldata_*`, `encode_swap4_router_calldata_*`. TypeScript: `boing-sdk` `nativeDexSwap2Router.ts` (same module name; includes 3–4 hop encoders).

## CREATE2

Salt: **`NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1`** (label `BOING_NATIVEDEX_MHOP_V1`, zero-padded to 32 bytes). **`NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1`** is a **deprecated alias** to the **same** bytes.

## Older doc name

[NATIVE-DEX-SWAP2-ROUTER.md](./NATIVE-DEX-SWAP2-ROUTER.md) described the two-hop-only revision; this file is the canonical multihop spec.
