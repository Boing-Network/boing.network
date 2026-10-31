# Fly.io — hosted Boing testnet

> 👋 **Everyday users:** you call `https://testnet-rpc.boing.network/` — you do not SSH to Fly.  
> 🛠️ **Developers:** that hostname is a Cloudflare Worker in front of Fly `boing-testnet-1`. Never point it at a home tunnel.  
> 🛰️ **Operators:** this is the canonical hosted cluster. First deploy creates **new chain state** on volumes.

Run **`boing-node`** on [Fly.io](https://fly.io): a validator with faucet and public JSON-RPC.

This is the **canonical hosted testnet**. Public JSON-RPC at `https://testnet-rpc.boing.network/` is served by the Cloudflare Worker in [`workers/public-rpc-gateway`](../workers/public-rpc-gateway/) (health-checked proxy to `boing-testnet-1`). The first Fly deploy creates **new chain state** on volumes — it is not the old home-lab / tunnel ledger unless you restore that volume.

```mermaid
flowchart LR
  Client[Wallets / explorer / dApps] --> GW[public-rpc-gateway]
  GW --> F1[Fly boing-testnet-1]
```

**Related:** [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md), [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md), [RUNBOOK.md](RUNBOOK.md), [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example).


## What gets deployed

| Fly app | Role | HTTPS RPC | P2P |
|---|---|---|---|
| **`boing-testnet-1`** | Validator + faucet + public RPC | `https://boing-testnet-1.fly.dev/` | TCP **4001** |

`boing-testnet-2` (former full-node companion) was **retired 2026-10-24**. Its `fly.testnet-2.toml` was removed 2026-10-31 (restore with `git show dda495d:fly.testnet-2.toml`).

The live app:

- Uses the repo-root [`Dockerfile`](../Dockerfile) (release `boing-node`)
- Persists `--data-dir` on a Fly volume (`boing_data` → `/data`)
- Stays running (`auto_stop_machines = "off"`, `min_machines_running = 1`)
- Sets **`BOING_CHAIN_ID=6913`**, **`BOING_CHAIN_NAME=Boing Testnet`**, browser CORS for the official sites
- Requires **`BOING_OPERATOR_RPC_TOKEN`** for operator QA RPCs
- Optional fee market: set **`BOING_MICRO_FEES_ACTIVATION_HEIGHT`**, then **`npm run apply-micro-fee-policy`** for Solana-like micro fees ([OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md))

Only **testnet-1** runs `--validator`. Promote a second validator later with a shared `--validators` / `BOING_VALIDATOR_KEY` set ([TESTNET.md](TESTNET.md) §1, [RUNBOOK.md](RUNBOOK.md) §8.1).

## Prerequisites

- [flyctl](https://fly.io/docs/flyctl/install/) authenticated (`fly auth whoami`)
- A Fly organization (default script org: **`personal`**)
- From this repository root

## One-command deploy

```bash
./scripts/fly-deploy-testnet.sh
```

The script creates the app, a 10 GB volume in **`iad`**, IPs, an operator token secret, builds on Fly’s remote builder, and deploys **testnet-1**. (A companion full node needs `FLY_APP_2` and `FLY_CONFIG_2`.)

On Windows Git Bash, the script sets **`MSYS_NO_PATHCONV=1`** so `/ip4/...` bootnode multiaddrs are not rewritten as filesystem paths.

Override names/region:

```bash
FLY_ORG=personal FLY_REGION=iad FLY_APP_1=boing-testnet-1 \
  ./scripts/fly-deploy-testnet.sh
```

## Manual deploy

```bash
fly apps create boing-testnet-1 --org personal
fly volumes create boing_data --region iad --size 10 -a boing-testnet-1 -y
fly secrets set BOING_OPERATOR_RPC_TOKEN="$(openssl rand -hex 24)" -a boing-testnet-1

fly deploy --config fly.testnet-1.toml --remote-only --ha=false
```

Config lives at the repo root so the Docker context includes `Cargo.toml` and `crates/`: [`fly.testnet-1.toml`](../fly.testnet-1.toml). Entrypoint: [`deploy/fly/entrypoint.sh`](../deploy/fly/entrypoint.sh).

## Verify

```bash
curl -fsS https://boing-testnet-1.fly.dev/live
curl -fsS -A boing-sdk/json-rpc -X POST https://boing-testnet-1.fly.dev/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_health","params":[]}'

export BOING_RPC_URL=https://boing-testnet-1.fly.dev/
# from examples/native-boing-tutorial after boing-sdk build:
npm run preflight-rpc
```

Height stays **0** until a transaction commits (a faucet request is enough). That matches local single-validator behavior: blocks are produced when the mempool is non-empty ([RUNBOOK.md](RUNBOOK.md)).

Restarts keep the tip: the node writes `chain/blocks/0.bin` on first start if it was missing, then reloads later heights from the volume.

## Public bootnodes

- **HTTPS RPC** uses Fly’s shared IPv4 + `*.fly.dev`.
- **Public P2P** (`/ip4/…/tcp/4001`) needs a **dedicated IPv4** on the app (`fly ips allocate-v4`). Shared anycast IPv4 is HTTP-only.
- Hosted public bootnode: **`/ip4/169.155.48.188/tcp/4001`** (`boing-testnet-1`). The former companion multiaddr `/ip4/109.105.220.118/tcp/4001` (`boing-testnet-2`) is **retired**.

### Peer connectivity and recovery

- Bootnodes (`BOING_BOOTNODES` + optional `BOING_EXTRA_BOOTNODES`) are **kept connected**: dialed at startup, then redialed with exponential backoff (2 s doubling to 60 s) whenever they are not connected. Remote restarts, proxy restarts and dropped TCP paths heal on their own, without restarting the node.
- A libp2p `ping` runs every 15 s on each connection. A connection that misses a ping for 20 s is closed and then redialed.
- The block-sync loop asks a connected peer for `height + 1` every 2 s and chains requests while it is behind, so a node that missed gossip catches up once a peer is back.
- `boing_health` returns `p2p: { peer_count, bootnodes, bootnodes_connected }` (read-only).
- Logs record `P2P: connection established …` and `P2P: peer disconnected … cause=…` (WARN when peers reach 0).

Publish dedicated IPv4 multiaddrs in `PUBLIC_BOOTNODES` / [TESTNET.md](TESTNET.md) §6 when you want community nodes to join this hosted net.

## Public RPC edge

`https://testnet-rpc.boing.network/` is a Cloudflare Worker ([`workers/public-rpc-gateway`](../workers/public-rpc-gateway/)) that:

1. Probes **`GET /live`** on each configured Fly origin.
2. Proxies JSON-RPC **POST /** (and node probe paths) to a live backend.
3. With a single backend (`boing-testnet-1`), there is no multi-origin failover; optional extra origins can be listed in **`RPC_BACKENDS`**.
4. Fans out **`boing_submitTransaction`** to **all** configured backends so contract deploys reach the validator when more than one origin is listed.

Do **not** point that hostname at a home Cloudflare Tunnel. A down laptop or `cloudflared` process is what produced explorer HTTP **530**.

```bash
cd workers/public-rpc-gateway && npm run deploy
curl -fsS https://testnet-rpc.boing.network/__gateway/health
export BOING_RPC_URL=https://testnet-rpc.boing.network/
# from examples/native-boing-tutorial after boing-sdk build:
npm run preflight-rpc
```

Keep **`BOING_CANONICAL_NATIVE_*`** in sync if you bootstrap DEX contracts on this chain. `boing.observer` also failsover to the Fly origins if the public hostname is down.

## Media blobs / capacity (live)

| Knob | Public testnet value |
|------|----------------------|
| Admission | `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1200` |
| PHASE7 cap | `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=1306` → **256 MiB** consensus |
| BLS flip | unset (Ed25519 tx **32** multi-pin default) |
| VM size | **performance-2x / 8 GiB** in `fly.testnet-1.toml` (interim cost cut; live already 8 GiB — do not restore 16 GiB on redeploy; 4 GiB OOMs under tip load) |
| Large Put body | `BOING_RPC_MAX_BODY_MB=600` on the Fly secret (gateway Worker still **8 MiB** request cap) |

Phases **1–33** media + Phase **34** download ACL are documented in [BOING-L1-MEDIA-BLOBS.md](BOING-L1-MEDIA-BLOBS.md). Block JSON uses **`0x` hex** byte fields; explorers should pass **`include_code: false`** for slim stubs under the gateway response cap.

## Ops

| Action | Command |
|---|---|
| Logs | `fly logs -a boing-testnet-1` |
| SSH | `fly ssh console -a boing-testnet-1` |
| Restart | `fly machines restart -a boing-testnet-1` |
| Scale (keep 1) | Do not autostop; validators must stay up |
| Secret rotate | `fly secrets set BOING_OPERATOR_RPC_TOKEN=… -a boing-testnet-1` |
