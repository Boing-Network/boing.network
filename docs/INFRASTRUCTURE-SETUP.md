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

The boing-node RPC server includes CORS headers so browser-based clients (e.g. boing.observer, boing.express, boing.network faucet) can call the RPC from different origins. Allowed origins: `https://boing.observer`, `https://boing.express`, `https://boing.network`, `https://www.boing.network`, and localhost variants for development.

---

## Troubleshooting

| Issue | Check |
|-------|-------|
| Bootnode 2 can't connect | Primary firewall allows TCP 4001; primary is running; correct IP |
| RPC not reachable | Cloudflare tunnel running; node has RPC on 8545 |
| CORS errors in browser | Node must be rebuilt with CORS support (included in boing-node); redeploy |
| Faucet fails | Node started with `--faucet_enable`; tunnel forwards to 8545 |
| "No nodes" in VibeMiner | Website built with `PUBLIC_BOOTNODES` and `PUBLIC_TESTNET_RPC_URL`; config redeployed |

---

## Quick Reference

**Primary (2 terminals):**

1. `scripts/start-bootnode-1.bat` (or .sh)
2. `scripts/start-cloudflare-tunnel.bat` (or cloudflared directly)

**Secondary:**

1. `BOOTNODE_1_IP=<primary_ip> scripts/start-bootnode-2.sh` (or .bat with IP edited)
