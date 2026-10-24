# Public RPC gateway

Cloudflare Worker that serves **`https://testnet-rpc.boing.network/`** and proxies JSON-RPC to the hosted Fly validator with `/live` health checks.

```
Clients (observer, website, SDK)
  → testnet-rpc.boing.network
    → this Worker
      → boing-testnet-1.fly.dev (validator + faucet)
```

This replaces a home **Cloudflare Tunnel** connector as the origin. The public hostname stays stable so explorer, wallet, and website env do not need to change.

**Writes:** `boing_submitTransaction` is posted to **every** configured backend in parallel (validator `boing-testnet-1` first in `RPC_BACKENDS`). Extra backends are optional; today the Worker is pinned to testnet-1 only. Reads pick the first healthy origin.

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
| HTTP **530** on the public hostname | DNS still CNAMEs to a dead Cloudflare Tunnel, or the Worker route is missing |
| HTTP **403** + `cf-mitigated: challenge` | Zone **Bot Fight Mode** is challenging non-browser clients — turn `fight_mode` **off** for JSON-RPC |
| DNS for `testnet-rpc` | Must be a **proxied** CNAME to this Worker (`*.workers.dev`), **not** `*.cfargotunnel.com` |
| JSON-RPC body **> ~6 MiB** | Worker returns JSON-RPC `-32000` (“response too large / truncated”) instead of forwarding a cut mid-array body. Tip blocks with large deploy bytecode can exceed this until the node hex-encodes bytecode |

Related: [FLY-IO.md](../../docs/FLY-IO.md), [TESTNET-RPC-INFRA.md](../../docs/TESTNET-RPC-INFRA.md).
