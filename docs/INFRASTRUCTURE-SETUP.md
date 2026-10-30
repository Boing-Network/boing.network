# Boing Testnet — Infrastructure Setup Guide

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


**Routing:** [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) places this guide next to **public RPC** and **testnet** docs in one map.

**Canonical public testnet** is Fly **`boing-testnet-1`** plus the Cloudflare RPC gateway Worker ([FLY-IO.md](FLY-IO.md)). Use this guide only when you want a **local / home-lab** two-machine layout (bootnodes + optional private Cloudflare Tunnel). **Do not** point `testnet-rpc.boing.network` at a laptop tunnel — that hostname is the Worker in `workers/public-rpc-gateway` → Fly only (`boing-testnet-2` retired 2026-10-24).

---

## Quick Run Instructions

### Computer 1 (Primary — bootnode + faucet + local RPC)

**Order:** Start the node first. A tunnel is optional and only for a **private** hostname you control — never for the public `testnet-rpc` record.

**Terminal 1 — Node:**
```bat
scripts\start-bootnode-1.bat
```
(Linux/macOS: `./scripts/start-bootnode-1.sh`)

**Terminal 2 — Optional private Cloudflare tunnel** (self-hosted RPC hostname only):
```bat
scripts\start-cloudflare-tunnel.bat
```
(Linux/macOS: `cloudflared tunnel --config ~/.cloudflared/config.yml run <your-tunnel-name>`)

**Canonical public RPC (do not override):** https://testnet-rpc.boing.network/ → Worker → `https://boing-testnet-1.fly.dev`

Get your public IP: `curl -s ifconfig.me` — share with Computer 2 for P2P.

---

### Computer 2 (Secondary — Bootnode 2)

1. **Build:** `cargo build --release`
2. **Edit** `scripts\start-bootnode-2.bat` — replace `REPLACE_WITH_PRIMARY_IP` with Computer 1's public IP.
3. **Run:** `scripts\start-bootnode-2.bat` (or `BOOTNODE_1_IP=<ip> ./scripts/start-bootnode-2.sh`)
4. **Get Computer 2's public IP:** `curl -s ifconfig.me` — add to `PUBLIC_BOOTNODES`.

---

### Deploy website

Production website env should keep pointing at the **hosted** edge (not your home-lab tunnel):

| Variable | Value |
|----------|-------|
| `PUBLIC_TESTNET_RPC_URL` | `https://testnet-rpc.boing.network/` |
| `PUBLIC_BOOTNODES` | Hosted: `/ip4/169.155.48.188/tcp/4001` — or your lab multiaddrs for a private net |

Set via **GitHub → Settings → Secrets and variables → Actions → Variables**, then push to `main`.

---

## Architecture

| Machine | Role | Ports | Notes |
|---------|------|-------|-------|
| **Primary** | Bootnode 1 + Faucet + RPC | 4001 (P2P), 8545 (RPC) | Local/home-lab only; public RPC is the Fly + Worker edge in [FLY-IO.md](FLY-IO.md) |
| **Secondary** | Bootnode 2 | 4001 (P2P), 8546 (RPC) | Connects to Bootnode 1; provides redundancy |

### Public RPC: QA operator methods

If you need operator RPCs (or the endpoint is reachable from the open internet), set a long random secret in the node environment as **`BOING_OPERATOR_RPC_TOKEN`**. Operator methods then require the HTTP header **`X-Boing-Operator: <same value>`** (in addition to normal governance checks for votes). If the variable is unset, operator methods (including slash appeals and admin-only `boing_qaPoolVote`) are rejected with `-32057` (fail-closed); set it for local dev too if you use them. This is a breaking change from earlier releases that treated unset as open. See [RPC-API-SPEC.md](RPC-API-SPEC.md).

**Read-only transparency:** **`boing_getQaRegistry`**, **`boing_qaPoolList`**, and **`boing_qaPoolConfig`** do not use the operator token — they are intended for public explorers (e.g. [boing.observer/qa](https://boing.observer/qa)). Canonical baseline JSON for docs lives under [docs/config/CANONICAL-QA-REGISTRY.md](config/CANONICAL-QA-REGISTRY.md).

The **`boing-node` process behind public RPC** (Fly `boing-testnet-1`, or a self-hosted origin you expose) must be a build that includes these methods, or explorers will show **Method not found** even when users run a newer binary locally. See [THREE-CODEBASE-ALIGNMENT.md §2.1](THREE-CODEBASE-ALIGNMENT.md#21-qa-registry-rpc-boing_getqaregistry--two-different-surfaces).

## Prerequisites

- **Rust** 1.70+ on both machines
- **Firewall:** TCP 4001 open on both machines (P2P)
- Same **genesis** (default; all nodes use the built-in genesis)
- Optional: Cloudflare account for a **private** tunnel hostname → `localhost:8545` (never steal the public `testnet-rpc` DNS from the Worker)

---

## Step 1: Primary Machine — Bootnode 1 + Faucet

### Windows

**Important:** On Windows, the batch script builds with `--no-default-features` to disable mDNS (avoids EADDRINUSE when libp2p adds interface-specific listeners). Bootnodes connect via explicit `--bootnodes`, so mDNS is not needed.

```bat
scripts\start-bootnode-1.bat
```

### Linux / macOS

```bash
chmod +x scripts/start-bootnode-1.sh
./scripts/start-bootnode-1.sh
```

Keep this terminal open. In another terminal, get your public IP:

```bash
curl -s ifconfig.me
```

Save this IP — Bootnode 2 will need it. Example: `73.84.106.121`.

---

## Step 2: Primary Machine — Cloudflare Tunnel

**Important:** A “custom domain” on Cloudflare **Pages** or a random **DNS** record is **not** enough — JSON-RPC needs **POST** to your node. Use a **Cloudflare Tunnel** public hostname `testnet-rpc.boing.network` → `http://127.0.0.1:8545`. See **[Cloudflare tunnel vs Pages-only DNS (HTTP 405)](#cloudflare-tunnel-vs-pages-only-dns-http-405)** below and run `node scripts/check-cloudflared-alignment.mjs` after editing `%USERPROFILE%\.cloudflared\config.yml` (or `~/.cloudflared/config.yml`).

In a **second terminal** on the primary:

### Windows

```bat
scripts\start-cloudflare-tunnel.bat
```

### Manual

```bash
.cloudflared/cloudflared tunnel --config ~/.cloudflared/config.yml run boing-testnet-rpc
```

Ensure your Cloudflare tunnel config routes `testnet-rpc.boing.network` to `http://127.0.0.1:8545`.

**Verify RPC:**

```bash
curl -s -X POST https://testnet-rpc.boing.network/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]}'
```

Expected: `{"jsonrpc":"2.0","id":1,"result":<number>}`

**Verify faucet on the public edge** (Fly `boing-testnet-1` must run with `--faucet-enable`):

```bash
curl -s -A boing-sdk/json-rpc -X POST https://testnet-rpc.boing.network/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_faucetRequest","params":["0x0000000000000000000000000000000000000000000000000000000000000001"]}'
```

- **Good:** JSON with `"result"` (`ok`, `amount`, …) or a rate-limit / balance error — faucet RPC is active.
- **Bad:** `"message":"Faucet not enabled on this node."` — redeploy/restart Fly with faucet enabled ([FLY-IO.md](FLY-IO.md)).

### QA transparency RPC (`boing_getQaRegistry`, `boing_qaPoolConfig`)

Explorers (e.g. **boing.observer/qa**) and tooling call these read-only methods on **the same public URL** (`https://testnet-rpc.boing.network/`). They are implemented in **current `boing-node`**. If the public URL returns **Method not found**, the Fly process (or your self-hosted origin) is an **older binary** — not a “wrong URL in the website” problem.

**Canonical public hostname:** `testnet-rpc.boing.network` is a **proxied CNAME** to the **`boing-public-rpc-gateway`** Worker (`workers/public-rpc-gateway`), which health-checks and proxies to Fly **`boing-testnet-1`**. Do **not** re-point that DNS at `*.cfargotunnel.com` or a home laptop.

**Operator checklist (hosted public RPC):**

1. Confirm DNS: `testnet-rpc` → Worker (`*.workers.dev`), proxied.
2. Confirm Fly: `GET https://boing-testnet-1.fly.dev/live` → **200**.
3. Confirm gateway: `GET https://testnet-rpc.boing.network/__gateway/health` → single backend, `live: true`.
4. Upgrade the Fly node when shipping RPC changes: [FLY-IO.md](FLY-IO.md), [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md).
5. Verify from any machine:
   ```bash
   node scripts/verify-public-testnet-rpc.mjs
   ```
   Or manually:
   ```bash
   curl -s -A boing-sdk/json-rpc -X POST https://testnet-rpc.boing.network/ \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"boing_getQaRegistry","params":[]}'
   ```
   Expect `"result":{...}` not `"error"` with **Method not found**.

**Self-hosted lab RPC:** use a **different** hostname and optional Cloudflare Tunnel → `http://127.0.0.1:8545`. Keep Bot Fight Mode off (or skipped) for any JSON-RPC hostname.

**Apps and env (already aligned if you use the canonical URL):**

| App | Variable | Value |
|-----|----------|--------|
| **boing.network** (Astro) | `PUBLIC_TESTNET_RPC_URL` at build | `https://testnet-rpc.boing.network/` (default in `website/src/config/testnet.ts` if unset) |
| **boing.observer** | `NEXT_PUBLIC_TESTNET_RPC` | `https://testnet-rpc.boing.network` (set in Cloudflare Pages / build env for that project) |

No website code change is required to “point” at this URL—it is already the default.

---

## Step 3: Secondary Machine — Bootnode 2

1. Clone the repo and build:
   ```bash
   cargo build --release
   ```

2. Edit the script and set `BOOTNODE_1_IP` to the primary's public IP:
   - **Linux/macOS:** `export BOOTNODE_1_IP=73.84.106.121` (or edit the script)
   - **Windows:** Edit `scripts\start-bootnode-2.bat` and replace `REPLACE_WITH_PRIMARY_IP` with the primary IP

3. Run Bootnode 2:

   **Windows:**
   ```bat
   scripts\start-bootnode-2.bat
   ```

   **Linux/macOS:**
   ```bash
   BOOTNODE_1_IP=73.84.106.121 ./scripts/start-bootnode-2.sh
   ```

4. Get secondary's public IP:
   ```bash
   curl -s ifconfig.me
   ```

   Add this to `PUBLIC_BOOTNODES` when deploying the website (comma-separated with primary's multiaddr).

---

## Step 4: Update Config and Deploy

For the **hosted** public network, website env stays on the Worker URL and hosted bootnode (even if your lab nodes are also running):

| Config | Location | Value |
|--------|----------|-------|
| `PUBLIC_TESTNET_RPC_URL` | Website env / GitHub Actions / Cloudflare Pages | `https://testnet-rpc.boing.network/` |
| `PUBLIC_BOOTNODES` | Website env / GitHub Actions / Cloudflare Pages | `/ip4/169.155.48.188/tcp/4001` |

**TESTNET.md** §6: Bootnode table; set env vars for production deploy.

**Verify:**

1. **Faucet:** Visit [boing.network/faucet](https://boing.network/faucet); enter a 32-byte hex account ID; request testnet BOING.
2. **Join page / listings:** [boing.network/testnet/join](https://boing.network/testnet/join) and `GET /api/networks` ([NETWORKS-API.md](NETWORKS-API.md)).
3. **Terminal validator:** `boing-node --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes <LIST> --validator --rpc-port 8545`

---

## CORS

The boing-node RPC server includes CORS headers so browser-based clients (e.g. boing.observer, boing.express, boing.network faucet, **boing.finance**) can call the RPC from different origins. Allowed origins include: `https://boing.observer`, `https://boing.express`, `https://boing.network`, `https://www.boing.network`, `https://boing.finance`, `https://www.boing.finance`, and localhost variants for development. After adding an origin, **rebuild and restart** the node that serves public RPC.

---

## Public RPC edge vs home tunnel (HTTP 405 / 530)

### Canonical path (production)

```text
Client → https://testnet-rpc.boing.network/
      → Cloudflare Worker (workers/public-rpc-gateway)
      → https://boing-testnet-1.fly.dev  (boing-node)
```

DNS for `testnet-rpc` must be a **proxied CNAME to the Worker**, not Pages and not `*.cfargotunnel.com`. Bot Fight Mode must stay **off** (or skipped) for that hostname. Details: [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md), [FLY-IO.md](FLY-IO.md).

### Why you see HTTP 405

Pointing **`testnet-rpc.boing.network`** only at **Cloudflare Pages** or a **generic proxied A/AAAA** often allows **GET** for static content and returns **`405 Method Not Allowed`** for JSON-RPC **POST**. The public Worker accepts POST and proxies to Fly.

### Optional private tunnel (lab only)

For a **self-hosted** RPC hostname you control (never the public `testnet-rpc` record), Cloudflare Tunnel can forward to `http://127.0.0.1:8545`:

```text
cloudflared tunnel --config %USERPROFILE%\.cloudflared\config.yml run <your-lab-tunnel>
```

Template: **[cloudflared-config.example.yml](./cloudflared-config.example.yml)** — change the hostname away from production `testnet-rpc`.

**Related:** [RUNBOOK.md](RUNBOOK.md) §8.3 (HTTP **530** when a tunnel origin is down; hosted path uses Worker → Fly).

---

## Troubleshooting

| Issue | Check |
|-------|-------|
| Bootnode 2 can't connect | Primary firewall allows TCP 4001; primary is running; correct IP |
| RPC not reachable | Cloudflare tunnel running; node has RPC on 8545 |
| CORS errors in browser | Node must be rebuilt with CORS support (included in boing-node); redeploy |
| **boing.finance** shows “Testnet RPC unreachable” | Often **CORS**: browser blocks `fetch` if the node build predates `boing.finance` in the allow list — rebuild `boing-node`, restart the **primary** RPC process, confirm `OPTIONS` from that origin returns `access-control-allow-origin`. |
| Faucet: “Faucet not enabled on this node.” | The RPC URL hits a node **without** `--faucet-enable`. Use `--faucet-enable` on the **same** node the tunnel forwards to (see `start-bootnode-1` scripts). A secondary full node alone cannot serve the public faucet. |
| "No nodes" on the join page | Website built with `PUBLIC_BOOTNODES` and `PUBLIC_TESTNET_RPC_URL`; config redeployed |
| **Method not found:** `boing_getQaRegistry` / `boing_qaPoolConfig` on public URL | Tunnel points to **8545**, but the **boing-node binary** there is too old. Rebuild from this repo on the primary, restart node, keep tunnel. Run `node scripts/verify-public-testnet-rpc.mjs`. |
| **HTTP 405** on `https://testnet-rpc.boing.network/` (verify script or `curl -X POST`) | **POST is not reaching boing-node.** Usually wrong DNS (hostname points to Pages/static host, not the tunnel), duplicate DNS records, or tunnel ingress missing this hostname. `curl -X POST http://127.0.0.1:8545` on the node PC must succeed first. JSON-RPC **requires POST**; GET-only frontends return 405. |

---

## Quick Reference

**Primary (2 terminals):**

1. `scripts/start-bootnode-1.bat` (or .sh)
2. `scripts/start-cloudflare-tunnel.bat` (or cloudflared directly)

**Secondary:**

1. `BOOTNODE_1_IP=<primary_ip> scripts/start-bootnode-2.sh` (or .bat with IP edited)
