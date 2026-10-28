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

## Method allow-list and per-IP limits

`src/edgePolicy.ts` enforces the public method allow-list (operator and unknown methods are refused), a 32-call batch cap and per-IP, per-class rate limits (Workers Rate Limiting bindings `RL_READ`, `RL_HEAVY`, `RL_SUBMIT`, `RL_WRITE`, `RL_FAUCET`). The table and numbers are generated from `ops/cloudflare/public-rpc/policy.json` (`node scripts/public-rpc-edge.mjs generate`; never edit `methodPolicy.generated.ts` or the generated block in `wrangler.toml` by hand).

- `EDGE_POLICY_MODE`: `log` (default: never blocks, logs `edge_policy` lines and sets `X-Boing-Edge-Policy`), `enforce`, or `off`.
- Optional secret `EDGE_TRUSTED_TOKEN`: callers sending it as `X-Boing-Edge-Trusted` skip the rate limits (never the blocklist).
- Merging does not deploy the Worker; run `npm run deploy` yourself. The Cloudflare zone rate-limit rule depends on the plan (Free: one coarse per-IP guard; the per-class limits here do not depend on the plan). Rollout order and numbers: [PUBLIC-RPC-WAF-GUIDANCE.md](../../docs/PUBLIC-RPC-WAF-GUIDANCE.md#2a-implemented-edge-policy-config-as-code).

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

## Tests and typecheck

```bash
cd workers/public-rpc-gateway
npm ci
npm run typecheck   # tsc --noEmit
npm test            # vitest: gateway.test.ts drives the Worker fetch handler (log / enforce / off modes)
```

Policy logic itself (generated tables, Cloudflare rules) is covered by `boing-sdk/tests/publicRpcPolicy.test.ts`. CI runs both in `.github/workflows/boing-sdk.yml`.
