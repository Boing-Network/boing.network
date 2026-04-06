# Engineering: DEX-style products on Boing L1 (non-EVM)

This document is for **protocol and VM engineers** shipping **factory / router / locker**-style DeFi on Boing L1. It complements **boing.finance** docs:

- [boing-l1-vs-evm-dex.md](https://github.com/Boing-Network/boing.finance/blob/main/docs/boing-l1-vs-evm-dex.md) (conceptual)
- [boing-l1-dex-roadmap.md](https://github.com/Boing-Network/boing.finance/blob/main/docs/boing-l1-dex-roadmap.md) (milestones)

## Facts about L1 today

**Capability vs EVM DEX:** see [BOING-NATIVE-DEX-CAPABILITY.md](./BOING-NATIVE-DEX-CAPABILITY.md) (what Boing VM can do for a native DEX today, and what is out of scope).

- **Application contracts** are **Boing VM bytecode**, deployed via **`ContractDeploy`** (optional **Create2** salt, optional **0xFD** init prefix). See [TECHNICAL-SPECIFICATION.md](./TECHNICAL-SPECIFICATION.md) § deploy / init.
- **Calls** use **`ContractCall`** with Boing calldata words — not Solidity ABI on the wire. Reference layouts: [BOING-REFERENCE-TOKEN.md](./BOING-REFERENCE-TOKEN.md), [NATIVE-AMM-CALLDATA.md](./NATIVE-AMM-CALLDATA.md).
- **Native constant-product AMM** exists as **ledger-native** logic with a **fixed calldata interface** (swap / add / remove liquidity) — not a drop-in replacement for an arbitrary Solidity **DEXFactoryV2** graph.
- **QA** gates deploys (`boing_qaCheck`, registry, optional QA pool). New DEX bytecode must be classified and allowed like other deploy templates.
- **No EVM layer on L1** for user contracts: Solidity artifacts from external repos **do not execute** on Boing VM without a full reimplementation or translation (which does not exist today).

## What “porting the Solidity DEX” means in practice

Deliverables are **new Boing VM programs** (and possibly **off-chain indexers**) that implement the **same product requirements** as `DEXFactoryV2` / `DEXRouter` / `LiquidityLocker` / pairs — **not** the same bytecode.

Suggested work breakdown:

1. **Interface spec (Boing-native)**  
   - Function selectors / calldata layouts for: create pool, swap exact in/out, add/remove liquidity, lock LP representation (if any), admin hooks.  
   - Storage model (which account holds code, how pair accounts are derived — Create2 pattern is supported).

2. **VM implementation**  
   - Implement in the toolchain that produces valid Boing VM bytecode (interpreter opcodes, gas, `LOG0`–`LOG4` for indexers).

3. **QA & purpose**  
   - Register purposes / rules; ensure mempool and execution paths match [QUALITY-ASSURANCE-NETWORK.md](./QUALITY-ASSURANCE-NETWORK.md).

4. **Deploy on testnet**  
   - Publish **32-byte AccountId** hex for factory, router, locker (and pairs if applicable).

5. **SDK + dApp**  
   - **boing-sdk:** encoders for the new calldata (pattern: `nativeAmm.ts`, `callAbi.ts`).  
   - **boing.finance:** set `REACT_APP_BOING_NATIVE_VM_*` and implement read/write flows (roadmap Phase 2).

## Relationship to native AMM

The **canonical native CP pool** is one **specific** on-chain program with **known** selectors and storage keys (see SDK `nativeAmmPool.ts`, execution `native_amm.rs`). A **multi-pair factory** is a **different** program (or set of programs); reusing the native AMM **implementation** may inform math and safety, but **APIs and deployment topology** will differ.

**Pair directory (v1):** execution [`native_dex_factory.rs`](../crates/boing-execution/src/native_dex_factory.rs) + [NATIVE-DEX-FACTORY.md](./NATIVE-DEX-FACTORY.md) + SDK `nativeDexFactory.ts` / `nativeDexFactoryPool.ts` / `nativeDexFactoryLogs.ts` — on-chain registry of `(token_a, token_b, pool)` after pools are deployed via separate `ContractDeploy` transactions.

**Ledger router (v1 / v2 / v3):** execution [`native_dex_ledger_router.rs`](../crates/boing-execution/src/native_dex_ledger_router.rs) + [NATIVE-DEX-LEDGER-ROUTER.md](./NATIVE-DEX-LEDGER-ROUTER.md) + SDK `nativeDexLedgerRouter.ts` — **v1** forwards **128** bytes; **v2** forwards **160** bytes (v5 **`swap_to`**); **v3** forwards **192** bytes (v5 **`remove_liquidity_to`**). **v1** is only safe with ledger-only pools; see doc.

**Multihop swap router (2–4 hops):** execution [`native_dex_multihop_swap_router.rs`](../crates/boing-execution/src/native_dex_multihop_swap_router.rs) + [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](./NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md) + SDK `nativeDexSwap2Router.ts` — sequential pool **`Call`**s (**128**- or **160**-byte inners) in one transaction; [NATIVE-DEX-SWAP2-ROUTER.md](./NATIVE-DEX-SWAP2-ROUTER.md) remains a two-hop-oriented snapshot.

## Checklist before boing.finance can “turn on” L1 DEX UI

- [x] **Calldata + receipt/log spec for integrators** — per-component layouts are documented in-repo: pool **`Log2`** ([NATIVE-AMM-CALLDATA.md](./NATIVE-AMM-CALLDATA.md)), directory **`Log3`** ([NATIVE-DEX-FACTORY.md](./NATIVE-DEX-FACTORY.md)), router payloads ([NATIVE-DEX-LEDGER-ROUTER.md](./NATIVE-DEX-LEDGER-ROUTER.md), [NATIVE-DEX-SWAP2-ROUTER.md](./NATIVE-DEX-SWAP2-ROUTER.md), [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](./NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md)). Breaking wire layouts should land with bytecode + doc bumps in the same change.  
- [x] **Pair directory** bytecode + calldata spec ([NATIVE-DEX-FACTORY.md](./NATIVE-DEX-FACTORY.md)); deploy + `register_pair` path documented.  
- [ ] At least **directory + pools** `AccountId`s on the **target** network (ops publish hex; optional routers only where `Caller` semantics allow; locker if required by product).  
- [x] **boing-sdk** encoders, CREATE2 prediction, factory storage/`return_data` decoders, `Log3` parsing, ledger-router payloads + access lists, **`findNativeDexFactoryPoolByTokens`** (simulate scan).  
- [x] Node RPC regression: `cargo test -p boing-node --test native_dex_factory_rpc_happy_path`.  
- [x] **Optional: pair listing without ad-hoc scans** — **Partial:** index **`Log3`** + SDK helpers ([INDEXER-RECEIPT-AND-LOG-INGESTION.md](./INDEXER-RECEIPT-AND-LOG-INGESTION.md)); a **hosted** catalog API / materialized pair table for dApp UX remains product-owned ([OBSERVER-HOSTED-SERVICE.md](./OBSERVER-HOSTED-SERVICE.md)).

Until **`nativeVm.*`** env ids are set for a given deployment, **boing.finance** correctly keeps them at zero and uses **native CP** + **EVM** stacks only where configured.
