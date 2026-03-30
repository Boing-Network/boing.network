# Boing Testnet — Infrastructure Setup Guide

This guide walks you through setting up the full testnet infrastructure: **Bootnode 1**, **Bootnode 2**, **Faucet RPC**, and **Cloudflare Tunnel** for the public URL.

---

## Quick Run Instructions

### Computer 1 (Primary — bootnode + faucet + public RPC)

**Order:** Start node first, then tunnel.

**Terminal 1 — Node:**
```bat
scripts\start-bootnode-1.bat
```
(Linux/macOS: `./scripts/start-bootnode-1.sh`)

**Terminal 2 — Cloudflare tunnel:**
```bat
scripts\start-cloudflare-tunnel.bat
```
(Linux/macOS: `.cloudflared/cloudflared tunnel --config ~/.cloudflared/config.yml run boing-testnet-rpc`)

**Public RPC:** https://testnet-rpc.boing.network/

Get your public IP: `curl -s ifconfig.me` — share with Computer 2.

---

### Computer 2 (Secondary — Bootnode 2)

1. **Build:** `cargo build --release`
2. **Edit** `scripts\start-bootnode-2.bat` — replace `REPLACE_WITH_PRIMARY_IP` with Computer 1's public IP.
3. **Run:** `scripts\start-bootnode-2.bat` (or `BOOTNODE_1_IP=<ip> ./scripts/start-bootnode-2.sh`)
4. **Get Computer 2's public IP:** `curl -s ifconfig.me` — add to `PUBLIC_BOOTNODES`.

---

### Deploy website

Set in **GitHub → Settings → Secrets and variables → Actions → Variables**:

| Variable | Value |
|----------|-------|
| `PUBLIC_TESTNET_RPC_URL` | `https://testnet-rpc.boing.network/` |
| `PUBLIC_BOOTNODES` | `/ip4/PRIMARY_IP/tcp/4001,/ip4/SECONDARY_IP/tcp/4001` |

Then push to `main`; the deploy workflow will use them.

---

## Architecture

| Machine | Role | Ports | Notes |
|---------|------|-------|-------|
| **Primary** | Bootnode 1 + Faucet + RPC | 4001 (P2P), 8545 (RPC) | Starts the chain; Cloudflare tunnel exposes RPC at testnet-rpc.boing.network |
| **Secondary** | Bootnode 2 | 4001 (P2P), 8546 (RPC) | Connects to Bootnode 1; provides redundancy |

### Public RPC: QA operator methods

If the JSON-RPC endpoint is reachable from the open internet, set a long random secret in the node environment as **`BOING_OPERATOR_RPC_TOKEN`**. When set, **`boing_qaPoolVote`** and **`boing_operatorApplyQaPolicy`** require the HTTP header **`X-Boing-Operator: <same value>`** (in addition to normal governance checks for votes). Without this, a public RPC could allow anyone to submit votes using a guessed or known admin address hex. Local dev can omit the variable to keep prior behavior. See [RPC-API-SPEC.md](RPC-API-SPEC.md).

**Read-only transparency:** **`boing_getQaRegistry`**, **`boing_qaPoolList`**, and **`boing_qaPoolConfig`** do not use the operator token — they are intended for public explorers (e.g. [boing.observer/qa](https://boing.observer/qa)). Canonical baseline JSON for docs lives under [docs/config/CANONICAL-QA-REGISTRY.md](config/CANONICAL-QA-REGISTRY.md).

The **`boing-node` process behind your Cloudflare tunnel** (the one listening on `localhost:8545`) must be a build that includes these methods, or explorers will show **Method not found** even when users run a newer binary locally (e.g. via VibeMiner). See [THREE-CODEBASE-ALIGNMENT.md §2.1](THREE-CODEBASE-ALIGNMENT.md#21-qa-registry-rpc-boing_getqaregistry--two-different-surfaces).

## Prerequisites

- **Rust** 1.70+ on both machines
- **Cloudflare account** with tunnel configured for `testnet-rpc.boing.network` → `localhost:8545`
- **Firewall:** TCP 4001 open on both machines (P2P)
- Same **genesis** (default; all nodes use the built-in genesis)

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

**Verify faucet is enabled on the tunneled node** (the process listening on `8545` must be started with `--faucet-enable`, e.g. `scripts/start-bootnode-1.bat`):

```bash
curl -s -X POST https://testnet-rpc.boing.network/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_faucetRequest","params":["0x0000000000000000000000000000000000000000000000000000000000000001"]}'
```

- **Good:** JSON with `"result"` (`ok`, `amount`, …) or a rate-limit / balance error — faucet RPC is active.
- **Bad:** `"message":"Faucet not enabled on this node."` — the node behind the tunnel was started **without** `--faucet-enable`, or the tunnel points at the wrong machine (e.g. secondary full node only). **Fix:** Run the primary bootnode script on the same PC as the tunnel, or add `--faucet-enable` to whichever node receives tunnel traffic on `8545`.

### QA transparency RPC (`boing_getQaRegistry`, `boing_qaPoolConfig`)

Explorers (e.g. **boing.observer/qa**) and tooling call these read-only methods on **the same public URL** (`https://testnet-rpc.boing.network/`). They are implemented in **current `boing-node`** in this repo (`crates/boing-node/src/rpc.rs`). If the public URL returns **Method not found**, the process behind the tunnel is an **older binary** (or not this codebase’s node)—**not** a DNS or “wrong URL in the website” problem.

**You do not need to buy a new domain.** `testnet-rpc.boing.network` must be a **DNS record in your existing `boing.network` zone** (Cloudflare) pointing at your **Cloudflare Tunnel** (public hostname → `http://127.0.0.1:8545`). VibeMiner’s tunnel button only starts `cloudflared` on **your** PC; it does not create that DNS record for you.

**Operator checklist (primary machine — the one that runs the tunnel):**

1. **Cloudflare (one-time per tunnel):** Zero Trust → Tunnels → create or select tunnel (e.g. `boing-testnet-rpc`). Add a **public hostname**: `testnet-rpc.boing.network` → `http://127.0.0.1:8545`. In DNS for `boing.network`, the tunnel should own the `testnet-rpc` record (CNAME to `*.cfargotunnel.com` as shown in the dashboard).
2. **Config file:** `cloudflared` uses `~/.cloudflared/config.yml` (Windows: `%USERPROFILE%\.cloudflared\config.yml`) with that tunnel id and ingress; see Cloudflare’s tunnel docs if you are setting this up from scratch.
3. **Upgrade the node binary** (this fixes “Method not found” for QA methods):
   ```bash
   git pull
   cargo build --release
   ```
   Stop the old `boing-node`, then start the new one with the **same** arguments as today (primary bootnode + `--faucet-enable` + RPC on **8545**), e.g. `scripts/start-bootnode-1.bat` or `./scripts/start-bootnode-1.sh`.
4. **Start the tunnel** after the node is listening (second terminal): `scripts/start-cloudflare-tunnel.bat` or the `cloudflared tunnel run …` command from Step 2 above.
5. **Verify from any machine:**
   ```bash
   node scripts/verify-public-testnet-rpc.mjs
   ```
   Or manually:
   ```bash
   curl -s -X POST https://testnet-rpc.boing.network/ \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"boing_getQaRegistry","params":[]}'
   ```
   Expect `"result":{...}` not `"error"` with **Method not found**.

**Apps and env (already aligned if you use the canonical URL):**

| App | Variable | Value |
|-----|----------|--------|
| **boing.network** (Astro) | `PUBLIC_TESTNET_RPC_URL` at build | `https://testnet-rpc.boing.network/` (default in `website/src/config/testnet.ts` if unset) |
| **boing.observer** | `NEXT_PUBLIC_TESTNET_RPC` | `https://testnet-rpc.boing.network` (set in Cloudflare Pages / build env for that project) |

No code change is required on the website **only** to “point” at this URL—it is already the default. **boing.observer** must be **deployed** with its env var set to that URL (or rely on its own default if it matches).

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

Once both bootnodes and the tunnel are running:

| Config | Location | Value |
|--------|----------|-------|
| `PUBLIC_TESTNET_RPC_URL` | Website env / GitHub Actions / Cloudflare Pages | `https://testnet-rpc.boing.network/` |
| `PUBLIC_BOOTNODES` | Website env / GitHub Actions / Cloudflare Pages | `/ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001` |

**TESTNET.md** §6: Bootnode table updated; set env vars for production deploy.

**Verify:**

1. **Faucet:** Visit [boing.network/faucet](https://boing.network/faucet); enter a 32-byte hex account ID; request testnet BOING.
2. **VibeMiner:** Should show nodes once the website is deployed with `PUBLIC_BOOTNODES`.
3. **Terminal validator:** `boing-node --p2p_listen /ip4/0.0.0.0/tcp/4001 --bootnodes <LIST> --validator --rpc-port 8545`

---

## CORS

The boing-node RPC server includes CORS headers so browser-based clients (e.g. boing.observer, boing.express, boing.network faucet, **boing.finance**) can call the RPC from different origins. Allowed origins include: `https://boing.observer`, `https://boing.express`, `https://boing.network`, `https://www.boing.network`, `https://boing.finance`, `https://www.boing.finance`, and localhost variants for development. After adding an origin, **rebuild and restart** the node that serves public RPC.

---

## Troubleshooting

| Issue | Check |
|-------|-------|
| Bootnode 2 can't connect | Primary firewall allows TCP 4001; primary is running; correct IP |
| RPC not reachable | Cloudflare tunnel running; node has RPC on 8545 |
| CORS errors in browser | Node must be rebuilt with CORS support (included in boing-node); redeploy |
| **boing.finance** shows “Testnet RPC unreachable” | Often **CORS**: browser blocks `fetch` if the node build predates `boing.finance` in the allow list — rebuild `boing-node`, restart the **primary** RPC process, confirm `OPTIONS` from that origin returns `access-control-allow-origin`. |
| Faucet: “Faucet not enabled on this node.” | The RPC URL hits a node **without** `--faucet-enable`. Use `--faucet-enable` on the **same** node the tunnel forwards to (see `start-bootnode-1` scripts). A secondary full node alone cannot serve the public faucet. |
| "No nodes" in VibeMiner | Website built with `PUBLIC_BOOTNODES` and `PUBLIC_TESTNET_RPC_URL`; config redeployed |
| **Method not found:** `boing_getQaRegistry` / `boing_qaPoolConfig` on public URL | Tunnel points to **8545**, but the **boing-node binary** there is too old. Rebuild from this repo on the primary, restart node, keep tunnel. Run `node scripts/verify-public-testnet-rpc.mjs`. |
| **HTTP 405** on `https://testnet-rpc.boing.network/` (verify script or `curl -X POST`) | **POST is not reaching boing-node.** Usually wrong DNS (hostname points to Pages/static host, not the tunnel), duplicate DNS records, or tunnel ingress missing this hostname. `curl -X POST http://127.0.0.1:8545` on the node PC must succeed first. JSON-RPC **requires POST**; GET-only frontends return 405. |

---

## Quick Reference

**Primary (2 terminals):**

1. `scripts/start-bootnode-1.bat` (or .sh)
2. `scripts/start-cloudflare-tunnel.bat` (or cloudflared directly)

**Secondary:**

1. `BOOTNODE_1_IP=<primary_ip> scripts/start-bootnode-2.sh` (or .bat with IP edited)
