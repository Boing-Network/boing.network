# Fly.io — hosted Boing testnet

Run **`boing-node`** on [Fly.io](https://fly.io): a validator with faucet and public JSON-RPC, plus a second full node that peers over P2P.

This is the **canonical hosted testnet**. Public JSON-RPC at `https://testnet-rpc.boing.network/` is served by the Cloudflare Worker in [`workers/public-rpc-gateway`](../workers/public-rpc-gateway/) (health-checked failover to both Fly apps). The first Fly deploy creates **new chain state** on volumes — it is not the old home-lab / tunnel ledger unless you restore that volume.

**Related:** [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md), [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md), [RUNBOOK.md](RUNBOOK.md), [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example).

## What gets deployed

| Fly app | Role | HTTPS RPC | P2P |
|---|---|---|---|
| **`boing-testnet-1`** | Validator + faucet + public RPC | `https://boing-testnet-1.fly.dev/` | TCP **4001** |
| **`boing-testnet-2`** | Full node (sync / extra bootnode) | `https://boing-testnet-2.fly.dev/` | TCP **4001** |

Both apps:

- Use the repo-root [`Dockerfile`](../Dockerfile) (release `boing-node`)
- Persist `--data-dir` on a Fly volume (`boing_data` → `/data`)
- Stay running (`auto_stop_machines = "off"`, `min_machines_running = 1`)
- Set **`BOING_CHAIN_ID=6913`**, **`BOING_CHAIN_NAME=Boing Testnet`**, browser CORS for the official sites
- Require **`BOING_OPERATOR_RPC_TOKEN`** for operator QA RPCs

Only **testnet-1** runs `--validator`, so the two machines do not fork as independent block producers. Both enable `--faucet-enable` so the deterministic faucet account exists on every node (otherwise peers cannot execute faucet transactions). Promote a second validator later with a shared `--validators` / `BOING_VALIDATOR_KEY` set ([TESTNET.md](TESTNET.md) §1, [RUNBOOK.md](RUNBOOK.md) §8.1).

## Prerequisites

- [flyctl](https://fly.io/docs/flyctl/install/) authenticated (`fly auth whoami`)
- A Fly organization (default script org: **`personal`**)
- From this repository root

## One-command deploy

```bash
./scripts/fly-deploy-testnet.sh
```

The script creates apps, 10 GB volumes in **`iad`**, IPs, an operator token secret, builds on Fly’s remote builder, deploys **testnet-1**, then **testnet-2** with bootnodes pointing at node 1.

On Windows Git Bash, the script sets **`MSYS_NO_PATHCONV=1`** so `/ip4/...` bootnode multiaddrs are not rewritten as filesystem paths.

Override names/region:

```bash
FLY_ORG=personal FLY_REGION=iad FLY_APP_1=boing-testnet-1 FLY_APP_2=boing-testnet-2 \
  ./scripts/fly-deploy-testnet.sh
```

## Manual deploy

```bash
fly apps create boing-testnet-1 --org personal
fly apps create boing-testnet-2 --org personal
fly volumes create boing_data --region iad --size 10 -a boing-testnet-1 -y
fly volumes create boing_data --region iad --size 10 -a boing-testnet-2 -y
fly secrets set BOING_OPERATOR_RPC_TOKEN="$(openssl rand -hex 24)" -a boing-testnet-1
fly secrets set BOING_OPERATOR_RPC_TOKEN="(same value)" -a boing-testnet-2

fly deploy --config fly.testnet-1.toml --remote-only --ha=false
# Discover node-1 P2P addresses, then:
fly secrets set BOING_BOOTNODES='/ip4/A.B.C.D/tcp/4001' -a boing-testnet-2
fly deploy --config fly.testnet-2.toml --remote-only --ha=false
```

Configs live at the repo root so the Docker context includes `Cargo.toml` and `crates/`: [`fly.testnet-1.toml`](../fly.testnet-1.toml), [`fly.testnet-2.toml`](../fly.testnet-2.toml). Entrypoint: [`deploy/fly/entrypoint.sh`](../deploy/fly/entrypoint.sh).

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
- Nodes also peer over Fly 6PN (`/ip6/…/tcp/4001`) when `BOING_BOOTNODES` includes private IPs.

Publish dedicated IPv4 multiaddrs in `PUBLIC_BOOTNODES` / [TESTNET.md](TESTNET.md) §6 when you want community nodes to join this hosted net.

## Public RPC edge

`https://testnet-rpc.boing.network/` is a Cloudflare Worker ([`workers/public-rpc-gateway`](../workers/public-rpc-gateway/)) that:

1. Probes **`GET /live`** on each Fly origin.
2. Proxies JSON-RPC **POST /** (and node probe paths) to a live backend.
3. Failsover **testnet-1 → testnet-2** on HTTP **5xx/530** or timeout.

Do **not** point that hostname at a home Cloudflare Tunnel. A down laptop or `cloudflared` process is what produced explorer HTTP **530**.

```bash
cd workers/public-rpc-gateway && npm run deploy
curl -fsS https://testnet-rpc.boing.network/__gateway/health
export BOING_RPC_URL=https://testnet-rpc.boing.network/
# from examples/native-boing-tutorial after boing-sdk build:
npm run preflight-rpc
```

Keep **`BOING_CANONICAL_NATIVE_*`** in sync if you bootstrap DEX contracts on this chain. `boing.observer` also failsover to the Fly origins if the public hostname is down.

## Ops

| Action | Command |
|---|---|
| Logs | `fly logs -a boing-testnet-1` |
| SSH | `fly ssh console -a boing-testnet-1` |
| Restart | `fly machines restart -a boing-testnet-1` |
| Scale (keep 1) | Do not autostop; validators must stay up |
| Secret rotate | `fly secrets set BOING_OPERATOR_RPC_TOKEN=… -a boing-testnet-1` (and `-2`) |
