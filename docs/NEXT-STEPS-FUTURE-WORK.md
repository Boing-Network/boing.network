# Next steps and future work (backlog)

This file **does not** replace detailed checklists ([EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md), [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md), [BUILD-ROADMAP.md](BUILD-ROADMAP.md), [TESTNET.md](TESTNET.md)). It **groups** what is still **partial**, **ops-dependent**, or **multi-sprint** so contributors can find the next slice quickly — including **enhancements**, **optimizations** (protocol, SDK, and ops), and **infrastructure / CI** upgrades.

**Protocol / VM crate tasks:** [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) — currently **all tracks checked**; new opcode or receipt work should add rows there again.

**Product-facing parity matrix:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md).

**Strategic enhancements & ecosystem vision (not sprint-sized tasks):** [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) — SDK depth, automation, incentives, P2P, audits; pairs with [BUILD-ROADMAP.md](BUILD-ROADMAP.md) phases.

**Cost / scale planning:** [NETWORK-COST-ESTIMATE.md](NETWORK-COST-ESTIMATE.md).

**Hosting independence (self-host vs Cloudflare, dependency posture):** [BOING-INFRASTRUCTURE-INDEPENDENCE.md](BOING-INFRASTRUCTURE-INDEPENDENCE.md).

---

## Infrastructure, hosting, and CI (upgrade map)

Use this table when the task is **ops**, **release**, or **pipeline** — not application code in `crates/` or `boing-sdk/`.

| Area | Doc or artifact |
|------|-----------------|
| Bootnodes, tunnel, GitHub Actions variables for site deploy | [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) |
| Pages / Workers / D1 / R2, deploy flow | [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md); workflow **`.github/workflows/deploy-pages.yml`** |
| First-time public testnet RPC + faucet order | [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md); [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) |
| Binary / RPC surface upgrade on an **existing** public endpoint | [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) |
| Hosted observer / durable indexer (product scale) | [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md); reference **`.github/workflows/observer-ingest-d1.yml`** + **`examples/observer-d1-worker/`** |
| **`boing-sdk`** build + unit tests on SDK changes | **`.github/workflows/boing-sdk.yml`** |
| Local node + live RPC Vitest + tutorial preflight | **`.github/workflows/boing-sdk-rpc-integration.yml`** — starts **`boing-node`** with **`BOING_CHAIN_ID=6913`** / **`BOING_CHAIN_NAME=Boing Testnet`**; Vitest **`BOING_EXPECT_CHAIN_ID`** checks **`getNetworkInfo`** |
| Playwright + Boing Express (install smoke; extension E2E optional) | **`.github/workflows/native-boing-playwright.yml`**; [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md) |
| **Canonical native AMM pool vs public testnet RPC** | **`.github/workflows/canonical-pool-public-rpc.yml`** — daily + **`workflow_dispatch`**; **`BOING_REQUIRE_NONZERO_RESERVE=1`** |
| Desktop / release artifacts | **`.github/workflows/release.yml`**, **`.github/workflows/release-desktop-hub.yml`** |

---

## Enhancements and optimizations (where to plan them)

| Kind | Where it lives |
|------|----------------|
| **VM / receipts / RPC opcodes** | [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) (re-open rows when adding features); [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md); [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) |
| **Full-stack capability targets** (wallet, indexer, SDK ergonomics) | [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) |
| **Native AMM** (fees, logs, pool ids, wallet paths) | [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md); [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) |
| **Native DEX** (pair directory, routers, multihop, discovery helpers) | [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md); [BOING-L1-DEX-ENGINEERING.md](BOING-L1-DEX-ENGINEERING.md); [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md); [NATIVE-DEX-LEDGER-ROUTER.md](NATIVE-DEX-LEDGER-ROUTER.md); [NATIVE-DEX-SWAP2-ROUTER.md](NATIVE-DEX-SWAP2-ROUTER.md); [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md) |
| **Indexer throughput, pruned nodes, gaps** | [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md); SDK **`indexerSync`**, **`indexerGaps`** |
| **Node limits, rate limits, public RPC policy** | [RUNBOOK.md](RUNBOOK.md) (e.g. **`boing_getLogs`**, **`BOING_OPERATOR_RPC_TOKEN`**); [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) (**Public RPC: QA operator methods**) |
| **Security, audits, bounty** | [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) |

---

## Recently landed in-repo (reference implementations)

| Item | Location |
|------|----------|
| Block replay range over RPC | Tutorial script **`npm run fetch-blocks-range`** — [examples/native-boing-tutorial/scripts/fetch-blocks-range.mjs](../examples/native-boing-tutorial/scripts/fetch-blocks-range.mjs) (`fetchBlocksWithReceiptsForHeightRange`, optional **`BOING_CLAMP_TO_DURABLE`**) |
| SDK ingestion pseudo-flow | [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) § **SDK-assisted ingestion tick (pseudo-flow)** — `getIndexerChainTips` → `clampIndexerHeightRange` → `fetchBlocksWithReceiptsForHeightRange` |
| Chunked logs | `npm run fetch-logs-range` — [examples/native-boing-tutorial/scripts/fetch-logs-range.mjs](../examples/native-boing-tutorial/scripts/fetch-logs-range.mjs) |
| Chain tips / clamp demo | `npm run indexer-chain-tips` |
| Indexer tick (plan + optional fetch) | **`npm run indexer-ingest-tick`** — [examples/native-boing-tutorial/scripts/indexer-ingest-tick.mjs](../examples/native-boing-tutorial/scripts/indexer-ingest-tick.mjs) (`planIndexerCatchUp`, **`BOING_FETCH=1`** for **`fetchBlocksWithReceiptsForHeightRange`**) |
| Native AMM pool storage (reserves + LP) | `npm run fetch-native-amm-reserves` — **`fetchNativeConstantProductPoolSnapshot`** (batched reserves + total LP; optional signer LP via **`BOING_SIGNER_HEX`**); **`fetchNativeConstantProductReserves`** still available for two-key reads only |
| Native AMM LP shares + remove + swap fee | **NAMM-1** / **NAMM-2** — `native_amm.rs` LP keys + `remove_liquidity`; **30 bps** output fee; SDK `constantProductAmountOut` / `NATIVE_CP_SWAP_FEE_BPS`; [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) |
| Native AMM `Log2` events + parsing | **NAMM-3** — `swap` / `add_liquidity` / `remove_liquidity` each emit **`Log2`** (`NATIVE_AMM_TOPIC_*`, caller, 96-byte data); SDK **`NATIVE_AMM_TOPIC_*_HEX`**, **`nativeAmmLogs.ts`** (`tryParseNativeAmmLog2`, **`filterMapNativeAmmRpcLogs`**); tutorial **`npm run fetch-native-amm-logs`** |
| Verbose block-range JSON + indexer diagram + opt-in RPC tests | **`fetch-blocks-range`**: `--verbose` / **`BOING_VERBOSE`**; indexer mermaid flow; **`boing-sdk`** `tests/rpcIntegration.test.ts` + **`npm run verify`** + **`BOING_INTEGRATION_RPC_URL`** |
| RPC capability probe + CI | **`probeBoingRpcCapabilities`** / **`npm run probe-rpc`**; GitHub Actions **`.github/workflows/boing-sdk.yml`** (build + test on `boing-sdk/**` changes) |
| Native AMM browser smoke harness | **`examples/native-boing-playwright`** — Playwright + unpacked Boing Express (`BOING_EXPRESS_EXTENSION_PATH`); **`npm run native-amm-e2e`** from repo root; workflow **`.github/workflows/native-boing-playwright.yml`** (install + run; tests skip without extension) |
| Public RPC preflight + go-live doc | **`npm run preflight-rpc`** or **`check-testnet-rpc`** (tutorial + repo root delegate); [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md); **`deploy-native-amm-pool.mjs`** calls **`client.chainHeight()`** before submit; tutorial **`.gitignore`** **`pool.hex`** |
| Mempool cap wired to security config | **`boing-node`** applies **`RateLimitConfig::default_mainnet().pending_txs_per_sender`** (16) at startup; **`--pending-txs-per-sender`** override; [RUNBOOK.md](RUNBOOK.md) §2 |
| Dev rate-limit profile | **`boing-node --dev-rate-limits`** or **`BOING_RATE_PROFILE=dev`** → **`RateLimitConfig::default_devnet`**; **`BOING_RATE_PROFILE=mainnet`** overrides flag; [RUNBOOK.md](RUNBOOK.md) §2 |
| Native AMM access list + future token `CALL` | **`NativePoolAccessListOptions.additionalAccountsHex32`** on **`buildNativeConstantProductPoolAccessList`** / **`buildNativeConstantProductContractCallTx`** / **`mergeNativePoolAccessListWithSimulation`**; [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) § Minimal access list |
| Tutorial: native AMM `contract_call` JSON | **`npm run native-amm-print-contract-call-tx`** (+ repo root delegate) — [native-amm-print-contract-call-tx.mjs](../examples/native-boing-tutorial/scripts/native-amm-print-contract-call-tx.mjs) |
| Testnet ops umbrella + Playwright CI + observer poll | [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md), [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md), **`observer-chain-tip-poll`** |
| Public RPC node upgrade checklist | [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) |
| **`BOING_POLL_ONCE` + CI** | **`preflight-rpc`** bundles check + one-shot poll; **`native-amm-print-contract-call-tx`** smoke in [boing-sdk-rpc-integration.yml](../.github/workflows/boing-sdk-rpc-integration.yml) |
| **`preflight-rpc`** (check + observer once) | [preflight-rpc.mjs](../examples/native-boing-tutorial/scripts/preflight-rpc.mjs); repo root + tutorial **`npm run preflight-rpc`**; [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) kept in sync |
| Live RPC **`getTransactionReceipt`** + indexer pruned-node guidance | [rpcIntegration.test.ts](../boing-sdk/tests/rpcIntegration.test.ts); [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) § **Pruned nodes and missing blocks** |
| **`/api/networks` `meta` + VibeMiner §6 + kebab-case CLI docs** | [networks.js](../website/functions/api/networks.js) **`buildNetworksMeta`**; [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) §3.1 / §6; docs + site copy **`--p2p-listen`** (clap); [main.rs](../crates/boing-node/src/main.rs) help text |
| **P2P signed tx gossip + per-IP connection cap + unbounded `P2pEvent` channel** | Gossip **`boing/transactions`** carries **`SignedTransaction`**; **`--max-connections-per-ip`**; swarm → node events use **`mpsc::unbounded_channel`** (no await backpressure stall) — [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) §12.3, [RUNBOOK.md](RUNBOOK.md) §8.1 |
| **Integration test: RPC submit → peer mempool via gossip** | **`cargo test -p boing-node --test p2p_tx_gossip_rpc`** — four-node full mesh ([`p2p_tx_gossip_rpc.rs`](../crates/boing-node/tests/p2p_tx_gossip_rpc.rs)) |
| **`Mempool::contains_tx_id`** | [mempool.rs](../crates/boing-node/src/mempool.rs) — observability for tests and tooling |
| **Canonical pool on Join Testnet page + ops smoke** | [website/src/config/testnet.ts](../website/src/config/testnet.ts) **`CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX`**; [join.astro](../website/src/pages/testnet/join.astro) § Native AMM pool; **`npm run check-canonical-pool`** — [check-canonical-native-amm-pool.mjs](../scripts/check-canonical-native-amm-pool.mjs) |
| **`boing_getNetworkInfo` + public testnet operator env** | RPC method + SDK **`getNetworkInfo()`**; operator template [**`tools/boing-node-public-testnet.env.example`**](../tools/boing-node-public-testnet.env.example) (**`BOING_CHAIN_ID`**, **`BOING_CHAIN_NAME`**); docs: [RUNBOOK.md](RUNBOOK.md), [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md), [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md) |
| **Native DEX pair directory** | VM **`native_dex_factory_bytecode`** — `register_pair` / `pairs_count` / `get_pair_at` + **`Log3`**; **`cargo run -p boing-execution --example dump_native_dex_factory`**; SDK **`nativeDexFactory.ts`**, **`nativeDexFactoryPool.ts`** (**`findNativeDexFactoryPoolByTokens`**, **`fetchNativeDexFactoryPairsCount`**), **`nativeDexFactoryLogs.ts`**; tutorial **`npm run deploy-native-dex-directory`** — [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md) |
| **Native DEX ledger + multihop routers** | Ledger forwarder **v1–v3** ([NATIVE-DEX-LEDGER-ROUTER.md](NATIVE-DEX-LEDGER-ROUTER.md)); **swap2** + **2–4 hop** bundles ([NATIVE-DEX-SWAP2-ROUTER.md](NATIVE-DEX-SWAP2-ROUTER.md), [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md)); SDK **`nativeDexLedgerRouter.ts`**, **`nativeDexSwap2Router.ts`** + CREATE2 predictors in **`create2.ts`** |
| **Native DEX capability summary** | [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md) — what is / is not EVM-parity on L1 |
| **Node RPC: native DEX factory** | **`cargo test -p boing-node --test native_dex_factory_rpc_happy_path`** |
| **dApp token wizard helpers** | **`boing-sdk`:** **`buildReferenceFungibleDeployMetaTx`**, **`buildReferenceNftCollectionDeployMetaTx`**, **`isBoingTestnetChainId`**, **`preflightContractDeployMetaQa`**, **`isBoingNativeAccountIdHex`** — [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) |

---

## Indexer / explorer (roadmap still **partial**)

| Follow-up | Notes |
|-----------|--------|
| **Hosted observer** | **Architecture spec:** [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md) (OBS-1 — schema, reorgs, read API, deployment). **Interim:** **`npm run observer-chain-tip-poll`** ([TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) §3). **In-repo stepping stones:** [examples/observer-ingest-reference](../examples/observer-ingest-reference/) (JSON + **`node:sqlite`** tick), [examples/observer-d1-worker](../examples/observer-d1-worker/) (cron + D1), **`tools/observer-indexer-schema.sql`**. |
| **Full historical archive** | Pruned nodes → missing blocks; indexers need **`onMissingBlock: 'omit'`** + persist gaps (**`summarizeIndexerFetchGaps`** in **`boing-sdk`**) + archive backfill strategy — [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) § **Pruned nodes and missing blocks**. |
| **Multi-pool / contract discovery** | **In-repo:** pair directory + **`register_pair`** + SDK **`findNativeDexFactoryPoolByTokens`** + **`Log3`** parsers ([NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md), [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md)). **Still optional at product scale:** subgraph / REST / observer-maintained catalog so UIs avoid long RPC scans ([NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) A3.3). |
| **Public RPC rate limits** | [RUNBOOK.md](RUNBOOK.md) § Public RPC operators and **`boing_getLogs`**. |

---

## Scoped next passes (engineering order)

Rough sizing for **in-repo** work after the current SDK / indexer / RPC CI slice. Revisit when product picks a release theme.

| Pass | Scope | Depends on | Outcome |
|------|--------|------------|---------|
| **NAMM-1** | **LP share accounting** (mint on `add_liquidity`, burn on `remove_liquidity`) + pro-rata reserve withdrawal + `min_a` / `min_b` checks | Spec row in [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md); `boing-qa` opcode walk unchanged | **Done** — checklist **A1.1** / **A1.3** |
| **NAMM-2** | **Trading fee** — fixed **30 bps** on **swap output** + doc’d formula | **NAMM-1** (or independent of LP) | **Done** — `NATIVE_CP_SWAP_FEE_BPS`, `constant_product_amount_out_after_fee`, TS `constantProductAmountOut`, proptest |
| **NAMM-3** | **`Log2`** on swap / add / remove — fixed **topic0** + caller + data | [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) § Logs | **Done** — filter **`boing_getLogs`** / receipts by topic0 |
| **OPS-1** | **Canonical testnet pool `AccountId`** in [RPC-API-SPEC.md](RPC-API-SPEC.md) + [TESTNET.md](TESTNET.md) + **`boing-sdk`** + **boing.network** site config + boing.finance | Ops deploy + freeze address | **Done (2026-04-03):** pool **`0xffaa…d0c2`** — [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published. **In this repo:** `website/src/config/testnet.ts` **`CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX`**. **Outside repo:** confirm **boing.finance** `contracts.js` / env matches (same hex). |
| **E2E-1** | **Playwright** + loaded Boing Express extension (optional CI) | [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md) | **In-repo harness:** [examples/native-boing-playwright](../examples/native-boing-playwright/) (headed Chromium; manual unlock; skips without `BOING_EXPRESS_EXTENSION_PATH`). **Ops / CI limits:** [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md); default workflow validates install only. |
| **OBS-1** | **Hosted observer** (durable DB, reorg tail, scaling) | Product + infra | Beyond RPC+SDK scripts ([INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md)) |
| **VM-1** | **VibeMiner desktop app** — merge **`GET /api/networks`** **`meta`**, kebab-case **`node_command_template`**, tunnel name | Boing site deployed with [networks.js](../website/functions/api/networks.js) | **Boing-side contract:** [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) §3.1 / §6; implement in the **VibeMiner** repository (not in boing.network) |

**Now:** **NAMM-1**–**NAMM-3** are implemented in-repo; **OPS-1** hex is published in spec + TESTNET (**2026-04-03**). **VM-1** is documented and **`/api/networks`** exposes **`meta`** for the desktop app. Next typical slices are **VM-1** (apply §6 in VibeMiner), **E2E-1** (Playwright), or boing.finance env alignment, unless product expands pool bytecode again.

---

## Native AMM (checklist **still open** items)

| Follow-up | Notes |
|-----------|--------|
| **u128-wide math / adjustable on-chain fee** | **Done (VM base):** **`Mul` (`0x03`)** is **256×256 → 256** (`interpreter.rs`, **`proptest_mul.rs`**, `TECHNICAL-SPECIFICATION.md` §7.2) — native CP **output-side** fee math is no longer low-**64** truncated. **Still open:** Uniswap-style **fee-on-input** formula and/or **governable `fee_bps`** in pool storage (new bytecode revision). |
| **Richer log schemas** | Optional extra topics / `LOG3`+ if indexers need more indexed fields — current **NAMM-3** uses **`Log2`** only ([NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) § Logs). |
| **Canonical testnet pool `AccountId`** | **Published** **`0xffaa…d0c2`** in spec + **`boing-sdk`** + **`website/src/config/testnet.ts`** — verify **boing.finance** / partner apps use [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) §2 env matrix. |
| **Optional on-chain view selectors** | If bytecode adds explicit read methods beyond storage layout — **A5.2** remainder. |
| **Playwright + loaded extension CI** | **A4.3** — headed + manual unlock; default GH **ubuntu** cannot run it unattended ([PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md)). Optional **self-hosted** or private zip pipeline. |
| **Access list when pool `CALL`s tokens** | **SDK:** **`additionalAccountsHex32`** + sim merge (**A2.2**). **Remaining:** boing.finance / wallet paths pass token ids when product ships token-linked pool bytecode; execution must actually **`CALL`** those accounts. |

---

## BUILD-ROADMAP / TESTNET / ops (not single-PR SDK work)

Pointers only; details stay in source docs. **CI and hosting upgrades** are summarized in **§ Infrastructure, hosting, and CI** above.

| Area | Doc |
|------|-----|
| P2P, bridges, light clients, governance, meta-router, … | [BUILD-ROADMAP.md](BUILD-ROADMAP.md) (many unchecked rows) |
| Testnet launch checklist, bootnodes, faucet, portal | [TESTNET.md](TESTNET.md) |
| Website / Cloudflare / D1 / R2 | [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md) |
| Bootnodes, tunnel, `PUBLIC_TESTNET_RPC_URL` / `PUBLIC_BOOTNODES` | [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) |
| Security / audits / bounty | [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) |
| Ecosystem & automation vision (cross-cutting enhancements) | [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) |

---

## Optional in-repo slices (if you need a small next PR)

| Idea | Status |
|------|--------|
| **Public RPC smoke: canonical native AMM pool readable** | **Done** — **`npm run check-canonical-pool`** ([check-canonical-native-amm-pool.mjs](../scripts/check-canonical-native-amm-pool.mjs)); Join Testnet page shows pool id ([testnet.ts](../website/src/config/testnet.ts), [join.astro](../website/src/pages/testnet/join.astro)); **CI** [canonical-pool-public-rpc.yml](../.github/workflows/canonical-pool-public-rpc.yml) (daily **`workflow_dispatch`**, **`BOING_REQUIRE_NONZERO_RESERVE=1`**) |
| Extend **`fetch-blocks-range.mjs`** output (`--verbose` / **`BOING_VERBOSE`**, **`txIdsSample`**, **`perBlock`**) | **Done** — [fetch-blocks-range.mjs](../examples/native-boing-tutorial/scripts/fetch-blocks-range.mjs) |
| Vitest integration test against a live RPC (**opt-in** env) | **Done** — `boing-sdk/tests/rpcIntegration.test.ts` (live suite grows with **`BOING_INTEGRATION_RPC_URL`**; strict tests with **`BOING_EXPECT_FULL_RPC=1`**) + **`npm run verify`** explains skips when the URL is unset |
| **Indexer** doc: mermaid diagram (tick + reorg tail) | **Done** — [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) § Reorgs |
| **Operator go-live + RPC smoke** | **Done** — [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md), **`preflight-rpc`** / **`check-testnet-rpc.mjs`**, deploy preflight |
| **`probeBoingRpcCapabilities` + `probe-rpc` script + `boing-sdk` CI workflow** | **Done** — [rpcCapabilities.ts](../boing-sdk/src/rpcCapabilities.ts), [probe-rpc.mjs](../boing-sdk/scripts/probe-rpc.mjs), [boing-sdk.yml](../.github/workflows/boing-sdk.yml) |
| **`planIndexerCatchUp` + tutorial `indexer-ingest-tick`** | **Done** — [indexerSync.ts](../boing-sdk/src/indexerSync.ts), [indexer-ingest-tick.mjs](../examples/native-boing-tutorial/scripts/indexer-ingest-tick.mjs) |
| **Mempool `pending_txs_per_sender` from `RateLimitConfig`** | **Done** — [main.rs](../crates/boing-node/src/main.rs) startup + **`--pending-txs-per-sender`**; [mempool.rs](../crates/boing-node/src/mempool.rs) **`set_max_pending_per_sender`** |
| **`--dev-rate-limits` / `BOING_RATE_PROFILE` + optional token accounts in native pool access list** | **Done** — [main.rs](../crates/boing-node/src/main.rs); [nativeAmmPool.ts](../boing-sdk/src/nativeAmmPool.ts) **`NativePoolAccessListOptions`** |
| **Tutorial: print native AMM `contract_call` JSON** | **Done** — **`npm run native-amm-print-contract-call-tx`** — [native-amm-print-contract-call-tx.mjs](../examples/native-boing-tutorial/scripts/native-amm-print-contract-call-tx.mjs) |
| **TESTNET-OPS-RUNBOOK + Playwright CI ops doc + observer poll** | **Done** — [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md), [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md), [observer-chain-tip-poll.mjs](../examples/native-boing-tutorial/scripts/observer-chain-tip-poll.mjs) |
| **PUBLIC-RPC upgrade checklist + `BOING_POLL_ONCE` + CI AMM print smoke** | **Done** — [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md); [boing-sdk-rpc-integration.yml](../.github/workflows/boing-sdk-rpc-integration.yml) |
| **`preflight-rpc` + expanded PRE-VIBEMINER** | **Done** — [preflight-rpc.mjs](../examples/native-boing-tutorial/scripts/preflight-rpc.mjs); [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) |
| **RPC integration CI: tutorial smoke via `preflight-rpc`** | **Done** — [boing-sdk-rpc-integration.yml](../.github/workflows/boing-sdk-rpc-integration.yml) replaces separate **`check-testnet-rpc`** + **`observer-chain-tip-poll`** steps |
| **Live RPC: `getTransactionReceipt` + full probe assertions** | **Done** — [rpcIntegration.test.ts](../boing-sdk/tests/rpcIntegration.test.ts) unknown tx → **`null`**; **`BOING_EXPECT_FULL_RPC=1`** asserts **`boing_getNetworkInfo`** / **`boing_getBlockByHeight`** / **`boing_getTransactionReceipt`** in **`rpcSupportedMethods`** and **`probeBoingRpcCapabilities`** (6/6) |
| **Indexer doc: pruned / missing blocks** | **Done** — [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) § **Pruned nodes and missing blocks** |
| **Docs index + PRE-VIBEMINER: `BOING_OMIT_MISSING`, go-live wording** | **Done** — [README.md](README.md) lists [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md); [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) **`indexer-ingest-tick`** env; [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) cross-link from reorg §; [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) §7 RPC smoke pointer |
| **Docs index: native DEX + AMM integration map** | **Done** — [README.md](README.md) § Design & Build lists capability doc, router specs, [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md), integration checklist, LP vault/share, E2E smoke, **OPS-1**; [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) § Enhancements + indexer **Multi-pool / contract discovery** |
| **Root `package.json` delegates for tutorial scripts** | **Done** — every **`examples/native-boing-tutorial`** **`npm run`** name is wired at repo root (same script name); [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) §4, [README.md](../README.md) Quick Start, tutorial [README.md](../examples/native-boing-tutorial/README.md) |
| **dApp deploy wizard DX (`boing-sdk`)** | **Done** — **`buildReferenceFungibleDeployMetaTx`**, **`buildReferenceNftCollectionDeployMetaTx`**, **`isBoingTestnetChainId`** / **`normalizeBoingChainIdHex`**, **`preflightContractDeployMetaQa`**, **`isBoingNativeAccountIdHex`**; [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) § **One wizard, two backends**; [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md) § **Wizard shortcut** |
| **`/api/networks` `meta` + VibeMiner maintainer checklist + `--p2p-listen` docs** | **Done** — [networks.js](../website/functions/api/networks.js); [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md); kebab-case in [TESTNET.md](TESTNET.md), [RUNBOOK.md](RUNBOOK.md), website testnet pages |
| **Observer D1: optional CDN-friendly `Cache-Control` on stable reads** | **Done** — **`BOING_READ_CACHE_MAX_AGE`** on [observer-d1-worker](../examples/observer-d1-worker/) (**`public, max-age=…`**, cap **86400**); [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md) §4 |

**Still open (larger):** hosted observer **scale-out read plane** (horizontal read replicas, multi-tenant product hardening — architecture in [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md)); **in-repo D1 worker** already ships **reorg rewind**, read routes, optional **`BOING_READ_CACHE_MAX_AGE`**, and Vitest coverage under **`examples/observer-d1-worker/`**. Unattended Playwright + extension on **shared** GitHub runners. **OPS-1:** canonical pool hex is in [RPC-API-SPEC.md](RPC-API-SPEC.md) / [TESTNET.md](TESTNET.md) (**2026-04-03**); align **boing.finance** production env. **Done:** CI workflow **`.github/workflows/boing-sdk-rpc-integration.yml`** builds **`boing-node`**, starts it on **8545**, runs **`npm run probe-rpc`**, **`npm run verify`** (**`BOING_EXPECT_FULL_RPC=1`**: discovery + **`probeBoingRpcCapabilities`** 6/6 + **`planIndexerCatchUp`** live test), then **`examples/native-boing-tutorial`** **`npm run preflight-rpc`**, **`indexer-ingest-tick`**, **`native-amm-print-contract-call-tx`** parse check. **Done:** **`.github/workflows/observer-ingest-d1.yml`** — **`examples/observer-d1-worker`** **`wrangler deploy --dry-run`** + **`tsc`**, **`examples/observer-ingest-reference`** ingest scripts **`node --check`**. Triggers include **`examples/native-boing-tutorial/**`**; repo root **`npm run indexer-ingest-tick`** delegates to the tutorial package (run **`npm install`** there first if needed).

---

## References

- [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md)
- [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md)
- [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md)
- [BOING-INFRASTRUCTURE-INDEPENDENCE.md](BOING-INFRASTRUCTURE-INDEPENDENCE.md)
- [NETWORK-COST-ESTIMATE.md](NETWORK-COST-ESTIMATE.md)
- [boing-sdk README](../boing-sdk/README.md)

*Edit this file when you close a backlog row or add a new theme.*
