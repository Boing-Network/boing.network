# Handoff: testnet height stuck at 0 + VibeMiner two-PC peering (from PudgyMon)

**Date:** 2026-07-22 (America/New_York evening probe)  
**Source chat:** PudgyMon workspace (`C:\Users\chiku\Projects\PudgyMon`) diagnosing Boing RPC for game contract deploy  
**Target workspace:** Cursor multi-root `boing.network-vibeminer-workspace` (`boing.network` + `vibeminer`)  
**Operator note:** User is **stopping both VibeMiner nodes** for now; please apply infra/product fixes offline, then re-validate when nodes are restarted.

---

## Verdict (re-check)

Shared/public Boing testnet is **not** currently a healthy deploy target from this PC:

| Endpoint | Ready probes | `boing_chainHeight` | Deployer funded? |
|----------|--------------|---------------------|------------------|
| VibeMiner full node `http://127.0.0.1:8545` (this PC) | `/live` + `/ready` OK | **0** (no growth over 5s+) | **No** — faucet returns `ok` / mempool message, balance stays `0` |
| Public `https://testnet-rpc.boing.network` | OK (see UA note) | **0** | **No** — balance `0`; faucet may return cooldown `-32016` |
| Local solo validator `http://127.0.0.1:8546` (PudgyMon Docker, **separate private chain**) | OK | **≥3** | **Yes** — balance increased after faucet |

**Implication:** Faucet + “ready” are insufficient. Without block production / sync, faucet txs never commit, so apps cannot deploy or spend.

---

## What is running on the probed PC (Windows)

### VibeMiner full node (owns `:8545` and `:4001`)

- Process: `boing-node-windows-x86_64.exe` (PID observed ~31420)
- Path pattern:  
  `%APPDATA%\com.nicobuilds.vibeminer\nodes\devnet__boing-devnet\bin\<hash>\boing-node-windows-x86_64.exe`
- Data dir:  
  `%APPDATA%\com.nicobuilds.vibeminer\nodes\devnet__boing-devnet\data\full-node`
- Command (no `--validator`):

```text
boing-node-windows-x86_64.exe
  --data-dir …\data\full-node
  --p2p-listen /ip4/0.0.0.0/tcp/4001
  --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001
  --rpc-port 8545
  --faucet-enable
```

- Network id folder: **`devnet__boing-devnet`** only (no separate public-testnet node dir observed under VibeMiner `nodes/`).
- Local validator data dirs exist under the same network (`data/validator`, `data/windows-validator`) but **no second `boing-node` process** was running on this PC; user reports validator on a **second PC**.

### Also present (noise / conflict risk)

- Docker `tools-boing-node-1` (boing.network `tools/` compose): same bootnodes, `--faucet-enable`, **no `--validator`**, `network_mode: host`, logs show dialing `73.84.106.121:4001`. On Windows, **VibeMiner already binds host `8545` / `4001`** — treat this container as stale/conflicting for local RPC diagnosis.
- Docker `pudgymon-boing-solo` on **`:8546`**: solo `--validator --faucet-enable` (no P2P). Used only for PudgyMon local contract deploy. **Not** the shared testnet.

---

## Connectivity findings

1. **Bootnode `73.84.106.121:4001` unreachable** from this PC (`/dev/tcp` timeout; no HTTP on 80/443/8545 either).
2. Full node therefore **never syncs**; height stays 0; `boing_getNetworkInfo.chain_native.as_of_height` = 0; `account_count` = 2 (genesis-like).
3. Public RPC JSON-RPC works when called with a normal browser-like `User-Agent` (bare curl from this environment previously got **403**). Height still **0** with UA.
4. Two-PC VibeMiner setup (full node here + validator elsewhere) is **not peering** via the configured public bootnode. Until the full node has a working peer path to a block-producing validator (LAN IP bootnode, working public bootnode, or mDNS on same LAN), RPC will look “healthy” but stay useless for deploys.

---

## Deployer probe (no secrets)

- Account (from PudgyMon `scripts/boing` deploy key):  
  `0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f`
- On `:8545` / public RPC: `boing_getBalance` → `{ balance: "0" }` even after successful-looking `boing_faucetRequest`.
- On solo `:8546`: funded and contracts already deployed into PudgyMon `data/boing/contracts.json` (local chain only).

---

## Suspected product / docs fix areas

### `boing.network`

- [ ] Confirm whether public testnet tip is actually producing blocks; if tip is 0 network-wide, bootstrap / validator set / bootnode ops need attention (`docs/OPS-FRESH-TESTNET-BOOTSTRAP.md`, `docs/TESTNET.md`, `docs/TESTNET-RPC-INFRA.md`, `docs/RUNBOOK.md`). **Ops — still needed (live bootnode / tip).**
- [ ] Restore or replace bootnode `73.84.106.121:4001` (or publish current bootnode multiaddrs on join page + VibeMiner listing templates). **Ops — still needed.**
- [x] Public RPC edge: document **403 without User-Agent** + SDK Node/CLI default UA (`docs/TESTNET-RPC-INFRA.md`, `boing-sdk` `BOING_SDK_DEFAULT_USER_AGENT`). Cloudflare allow-list still operator-side.
- [x] Clarify faucet UX: `boing_faucetRequest` returns `tip_height`, `connected_peers`, optional `warning` when tip is 0; website faucet surfaces warning; RPC spec updated.

### `vibeminer`

- [x] Two-PC / home-lab path: **Custom bootnodes** field in Boing network modal (replaces `--bootnodes`, persisted in `localStorage`).
- [x] UI health: desktop polls local `boing_chainHeight` and shows `height N` / stuck-at-0 messaging in node status.
- [x] Faucet honesty: covered on node/website side (`warning` when tip 0); in-app local faucet balance poll still optional follow-up.
- [x] Preset naming: documented `devnet__boing-devnet` cache key vs Testnet product copy in `docs/NODE_RUNNING.md`.
- [ ] Windows: detect port conflicts if both Docker host-network `boing-node` and VibeMiner bind `4001`/`8545`. **Partial:** RPC port preflight already exists; P2P `4001` conflict messaging still open.

### Applied in this agent session (2026-07-22)

Code/docs landed in both repos while nodes are stopped.

**VibeMiner one-click:** defaults to **local validator (recommended)** so tip advances without public bootnodes. Public stake join preflights tip≠0. Height status + optional custom bootnodes remain for advanced/two-PC cases.

**Still blocked on ops for shared public testnet:** reachable bootnode + advancing public tip (`73.84.106.121:4001` / public RPC height 0).

---

## Suggested validation checklist (after fixes; nodes restarted)

1. On validator PC: process has `--validator`, height increases locally.
2. From full-node PC: TCP connect to validator `:4001` (or updated bootnode) succeeds.
3. Full node `boing_chainHeight` increases and tracks validator tip.
4. `boing_faucetRequest(deployer)` → balance &gt; 0 within a few blocks.
5. `boing_getNetworkInfo` `as_of_height` &gt; 0.
6. Optional: redeploy reference NFT + fungible via PudgyMon `scripts/boing/deploy_reference_assets.mjs` against the shared RPC (not solo `:8546`) and refresh `contracts.json`.

---

## PudgyMon context (consumer)

- Game/accounts integration expects a funded deployer + NFT/fungible AccountIds in `data/boing/contracts.json`.
- Current committed contracts point at local solo `http://127.0.0.1:8546` after public/VibeMiner RPC proved unusable.
- Once shared testnet is healthy, PudgyMon should redeploy and switch `BOING_RPC_URL` / contracts file to that tip.

---

## Related docs in this monorepo

- `docs/VIBEMINER-INTEGRATION.md` — validator vs full node flags, public testnet appendix  
- `docs/TESTNET.md` — single-node vs multi-node, bootnodes  
- `docs/OPS-FRESH-TESTNET-BOOTSTRAP.md` — bootstrap ops  
- VibeMiner: `docs/NODE_RUNNING.md`, `docs/BOING_TESTNET_PIN_SYNC.md`
