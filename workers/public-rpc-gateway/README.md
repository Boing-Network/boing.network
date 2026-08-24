# Public RPC gateway

Cloudflare Worker that serves **`https://testnet-rpc.boing.network/`** and proxies JSON-RPC to the hosted Fly cluster with `/live` health checks and failover.

```
Clients (observer, website, SDK)
  → testnet-rpc.boing.network
    → this Worker
      → boing-testnet-1.fly.dev (validator + faucet)
      → boing-testnet-2.fly.dev (full node)
```

This replaces a home **Cloudflare Tunnel** connector as the origin. The public hostname stays stable so explorer, wallet, and website env do not need to change.

**Writes:** `boing_submitTransaction` is posted to **every** configured backend in parallel (validator `boing-testnet-1` first in `RPC_BACKENDS`). Sequential read failover to the full node can mempool-accept a deploy that the single validator never includes — finance then shows success while `boing.observer` stays empty. Reads still fail over first-healthy.

## Deploy

```bash
cd workers/public-rpc-gateway
npm install
npm run deploy
```

Confirm:

```bash
curl -fsS https://testnet-rpc.boing.network/live
curl -fsS https://testnet-rpc.boing.network/__gateway/health
curl -fsS -A boing-sdk/json-rpc -X POST https://testnet-rpc.boing.network/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_health","params":[]}'
```

Override backends with Worker var **`RPC_BACKENDS`** (comma-separated origins, no trailing slash).

## Ops

| Check | Meaning |
|-------|---------|
| `GET /live` | Passed through to a healthy Fly node |
| `GET /__gateway/health` | Which backends the Worker currently marks live |
| HTTP **530** on the public hostname | Worker route is not attached, or Cloudflare still points at a down tunnel |

Related: [FLY-IO.md](../../docs/FLY-IO.md), [TESTNET-RPC-INFRA.md](../../docs/TESTNET-RPC-INFRA.md).
