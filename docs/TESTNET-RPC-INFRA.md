# Testnet operations, public RPC, and infrastructure

**Audience:** Operators and integrators who need **bootnodes**, **public JSON-RPC**, **Cloudflare tunnel / website env**, **canonical native AMM**, **QA policy**, and **monitoring** in one place.

Specialist guides (RPC methods, VM calldata, security) stay in their own files; this page is the **operator hub**.

---

## 1. Pick your doc (routing)

| You need… | Start here |
|-----------|------------|
| **User-facing testnet** (join, faucet, bootnodes) | [TESTNET.md](TESTNET.md) |
| **Copy-paste commands** (`cargo`, `boing-node`, tutorial `npm run`) | [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) |
| **Two-machine bootnode + tunnel + website secrets** | [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) |
| **Beta readiness / six pillars** | [READINESS.md](READINESS.md) |
| **Upgrade / restart the node behind public RPC** | [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) |
| **JSON-RPC methods, errors, canonical pool hex** | [RPC-API-SPEC.md](RPC-API-SPEC.md) |
| **Canonical native CP pool (OPS-1) + rotations** | [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published |
| **Self-hosted RPC + deploy pool (VibeMiner)** | [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md) |
| **VibeMiner listings + two-node Windows layout** | [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) |
| **Node flags, tunnel 530/1033, incidents** | [RUNBOOK.md](RUNBOOK.md) |
| **QA protocol + content blocklist** | [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md); apply policy: `npm run apply-public-testnet-qa-policy` |
| **Backlog / future work index** | [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) |

---

## 2. Current status snapshot (2026-05-21)

| Surface | Status | Verify |
|---------|--------|--------|
| **Public RPC** | **Live** — `https://testnet-rpc.boing.network/` | `npm run preflight-rpc` |
| **QA transparency RPC** | **Live** — registry + pool config | `npm run verify-public-testnet-rpc` |
| **QA content policy** | **132-term** `content_blocklist` on public RPC (when applied) | `boing_getQaRegistry`; `npm run apply-public-testnet-qa-policy` after edits |
| **Canonical CP pool** | **`0x7247ddc3…`** (reserve A readable) | `BOING_REQUIRE_NONZERO_RESERVE=1 npm run check-canonical-pool` |
| **QA registry vs docs** | Baseline in `docs/config/`; live may differ after operator apply | `npm run verify-qa-alignment` |
| **Explorer** | **Live** — [boing.observer](https://boing.observer) | `/qa`, `/tokens`, `/dex/pools` |
| **Chain tip** | Operator-dependent (may reset after redeploy) | `npm run observer-chain-tip-poll` |

After a chain reset, **`head_height`** may be **0** while prior bootstrap contracts remain readable. Treat **`boing_getNetworkInfo.end_user`** as the live contract hint source.

---

## 3. Go-live sequence (order matters)

**Prerequisite:** Workspace builds and tests clean — [READINESS.md](READINESS.md) §1 (`cargo test`, `boing-sdk` `npm test`).

1. **Same genesis everywhere** — All validators and full nodes share one genesis (faucet account, chain id, etc.).
2. **Bootnodes** — At least two stable P2P listeners (or one host today); publish multiaddrs on the website and in [TESTNET.md](TESTNET.md) §6. See [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md).
3. **Validators / block production** — Enough stake and connectivity that height advances.
4. **Public RPC** — `boing-node` with `--rpc-port` (direct or **Cloudflare Tunnel**). Set **`BOING_CHAIN_ID=6913`**, **`BOING_CHAIN_NAME=Boing Testnet`**, and optional **`BOING_CANONICAL_NATIVE_*`** from [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example). See [RUNBOOK.md](RUNBOOK.md) §8.
5. **Verify RPC from the internet** — Before announcing “testnet is up”:

   ```bash
   cd examples/native-boing-tutorial && npm install
   export BOING_RPC_URL=https://testnet-rpc.boing.network/
   npm run preflight-rpc
   ```

   Or **`npm run check-testnet-rpc`** only; optional **`BOING_PROBE_FULL=1`**. See [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md).

6. **Faucet** — Node behind the public URL runs with **`--faucet-enable`**; confirm via [boing.network/faucet](https://boing.network/faucet).
7. **Website / portal env** — [READINESS.md](READINESS.md) §3.3, [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md): `PUBLIC_BOOTNODES`, `PUBLIC_TESTNET_RPC_URL`.
8. **VibeMiner** — [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md); app reads **`GET https://boing.network/api/networks`**.
9. **Optional: native DEX full stack** — **`npm run deploy-native-dex-full-stack`** from [examples/native-boing-tutorial](../examples/native-boing-tutorial/) after **`boing-sdk`** build. For **public** testnet, publish canonical pool id per [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) (**OPS-1**).
10. **Optional: QA content policy** — Edit [`docs/config/qa_content_blocklist.en.json`](config/qa_content_blocklist.en.json), then **`npm run apply-public-testnet-qa-policy`** (requires **`BOING_OPERATOR_RPC_TOKEN`** when the node enforces operator auth). Restart nodes after upgrading **`boing-node`** so **`boing_qaCheck`** uses the live registry.

**Later upgrades** to the RPC binary: [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md).

### HTTP 530 / Cloudflare error 1033

Cloudflare returns **530** when the **tunnel cannot reach the origin** (RPC down, wrong port, or `cloudflared` stopped). Restore **`cloudflared` + `boing-node`** on the origin — not an SDK or dApp fix. Details: [RUNBOOK.md](RUNBOOK.md) §8.3.

---

## 4. Environment variables (integrators)

| Consumer | Typical vars | Where documented |
|----------|----------------|------------------|
| **Cloudflare Pages / website** | `PUBLIC_TESTNET_RPC_URL`, `PUBLIC_BOOTNODES` | [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md), [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md) |
| **boing.finance** | `REACT_APP_BOING_NATIVE_AMM_POOL`, `REACT_APP_BOING_NATIVE_VM_*` | [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) §3 |
| **Tutorial / scripts** | `BOING_RPC_URL`, `BOING_POOL_HEX` | [examples/native-boing-tutorial/README.md](../examples/native-boing-tutorial/README.md) |
| **Public `boing-node`** | `BOING_CHAIN_ID`, `BOING_CHAIN_NAME`, `BOING_CANONICAL_NATIVE_*`, `BOING_DEX_*`, optional `BOING_OPERATOR_RPC_TOKEN` | [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example), [RUNBOOK.md](RUNBOOK.md) §8 |
| **Relaxed testnet rate limits** | `BOING_RATE_PROFILE=dev` or `--dev-rate-limits` | [RUNBOOK.md](RUNBOOK.md) §2 — **not** for public RPC |

---

## 5. Canonical native AMM pool (integration contract)

1. **Source of truth:** [RPC-API-SPEC.md](RPC-API-SPEC.md) § Native constant-product AMM — **`0x7247ddc3180fdc4d3fd1e716229bfa16bad334a07d28aa9fda9ad1bfa7bdacc3`** (published **2026-05-21**). **`boing_getNetworkInfo.end_user`** on public RPC should match.
2. **Procedure:** [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md).
3. **SDK constant:** **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** in **`boing-sdk`**.

---

## 6. After deploy — verification

| Layer | Check |
|-------|--------|
| **RPC from internet** | `npm run preflight-rpc` or `check-testnet-rpc` ([PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md)) |
| **Canonical pool** | `npm run check-canonical-pool` — CI: [canonical-pool-public-rpc.yml](../.github/workflows/canonical-pool-public-rpc.yml) |
| **QA alignment** | `npm run verify-qa-alignment`, `npm run verify-public-testnet-rpc` |
| **QA vulgarity smoke** | `boing_qaCheck` with blocked `asset_name` → **`reject`** / **`CONTENT_POLICY_VIOLATION`** (after node binary includes registry fix) |
| **Tunnel health** | No HTTP **530** / **1033** on public URL |
| **Manual dApp smoke** | [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) § Manual E2E smoke |
| **Playwright (optional CI)** | [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md), [examples/native-boing-playwright/README.md](../examples/native-boing-playwright/README.md) |

---

## 7. Monitoring without a hosted observer

[boing.observer](https://boing.observer) is a **separate deploy** — see [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md). Until durable indexing is needed:

1. **`npm run observer-chain-tip-poll`** — height + **`boing_getSyncState`**; **`BOING_POLL_ONCE=1`** for one sample.
2. **`npm run check-testnet-rpc`**, **`verify-public-testnet-rpc`**, **`npm run probe-rpc`**.
3. **Indexer scripts** — `indexer-chain-tips`, `indexer-ingest-tick` ([INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md)).

Production-grade explorer backend: [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md) (OBS-1). Deployed D1 worker: **`GET /api/readiness`** on [examples/observer-d1-worker](../examples/observer-d1-worker/).

---

## 8. Security and incidents

| Topic | Doc |
|-------|-----|
| Disclosure | [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) §5 |
| Incident steps | [RUNBOOK.md](RUNBOOK.md) §6 |
| Public RPC operator token | **`BOING_OPERATOR_RPC_TOKEN`** + header **`X-Boing-Operator`** — [RPC-API-SPEC.md](RPC-API-SPEC.md) |

---

## 9. Line endings (Windows)

Repo **`.gitattributes`** enforces **`eol=lf`** for `*.js` / `*.mjs`. Re-clone or `git add --renormalize .` once if CRLF diffs appear.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
