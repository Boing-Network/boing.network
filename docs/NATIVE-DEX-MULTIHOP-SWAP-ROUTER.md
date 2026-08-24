# Native DEX multihop swap router (Boing VM)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


Bytecode: `native_dex_multihop_swap_router_bytecode()` in `crates/boing-execution/src/native_dex_multihop_swap_router.rs` (alias: `native_dex_swap2_router_bytecode()`).

## Purpose

Execute **2–6** native constant-product **`swap`** / **`swap_to`** calls **in one transaction**. Each hop uses the same **`Call`** pattern as [NATIVE-DEX-LEDGER-ROUTER.md](./NATIVE-DEX-LEDGER-ROUTER.md); the pool sees **`Caller` = this router** for every hop.

## Calldata

| Selector (word0 low byte) | Hops | Outer size | Inner size per hop |
|---------------------------|------|------------|---------------------|
| **`0xE5`** | 2 | **352** bytes | **128** |
| **`0xE6`** | 2 | **416** bytes | **160** |
| **`0xE7`** | 3 | **512** bytes | **128** |
| **`0xE8`** | 3 | **608** bytes | **160** |
| **`0xE9`** | 4 | **672** bytes | **128** |
| **`0xEA`** | 4 | **800** bytes | **160** |
| **`0xEB`** | 5 | **832** bytes | **128** |
| **`0xEC`** | 5 | **992** bytes | **160** |
| **`0xED`** | 6 | **992** bytes | **128** |
| **`0xEE`** | 6 | **1184** bytes | **160** |

Layout: word0 = selector; then for each hop, one **pool** word (32 bytes) and one **inner** calldata slice (**128** or **160** bytes).

Rust: `encode_swap2_router_calldata_*` … `encode_swap6_router_calldata_*`. TypeScript: `boing-sdk` `nativeDexSwap2Router.ts` (same module name; includes 3–6 hop encoders).

## SDK (routing + Express tx)

- **`nativeDexRouting.ts`:** **`findBestCpRoutes`** (default max hops = **6**), **`minOutFloorAfterSlippageBps`** / **`minOutPerHopFromQuotedRouteSlippageBps`**, **`encodeNativeDexMultihopRouterCalldata128FromRoute`**, **`encodeNativeDexMultihopRouterCalldata160FromRoute`**, and **`…WithSlippage`** shortcuts.
- **`nativeAmmPool.ts`:** **`buildNativeDexMultihopRouterAccessList`**, **`buildNativeDexMultihopRouterContractCallTx`**, **`mergeNativeDexMultihopRouterAccessListWithSimulation`**.
- **`nativeDexSeamless.ts`:** **`buildNativeDexMultihopSwapExpressTxFromRoute128`** / **`buildNativeDexMultihopSwapExpressTxFromRoute160`** — pass **`minOutPerHop`** or **`slippageBps`** (explicit **`minOutPerHop`** wins when set); optional **`includeVenueTokenAccounts`** for v2 token **`CALL`** access lists; **`applyNativeDexMultihopSimulationToContractCallTx`** after **`boing_simulateTransaction`**.
- **`pickFirstMultihopCpRoute`**, **`uniqueSortedTokenHex32FromCpRoute`** (`nativeDexRouting.ts`).

## CREATE2

Salt: **`NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1`** (label `BOING_NATIVEDEX_MHOP_V1`, zero-padded to 32 bytes). **`NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1`** is a **deprecated alias** to the **same** bytes.

## Older doc name

[NATIVE-DEX-SWAP2-ROUTER.md](./NATIVE-DEX-SWAP2-ROUTER.md) described the two-hop-only revision; this file is the canonical multihop spec.
