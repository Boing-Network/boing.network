# boing-origin-access-proxy

Reverse proxy for **`https://origin-testnet.boing.network/`** → **`https://boing-testnet-1.fly.dev`**.

Cloudflare Access (service token + operator email) sits on the custom hostname. This Worker exists so TLS terminates at Cloudflare without waiting on a Fly ACME cert for the custom name (orange-cloud CNAME to `*.workers.dev`).

**Not on the public RPC hot path.** `boing-public-rpc-gateway` talks to Fly directly with `X-Boing-Origin-Gate` (single Worker hop). Use this Worker for Access/human break-glass only — routing every `testnet-rpc` request through the service binding double-counts Free-plan invocations.

## Deploy

```bash
cd workers/origin-access-proxy
npm install
npx wrangler deploy
# After node companion gate is live:
npx wrangler secret put BOING_ORIGIN_GATE_TOKEN
```

DNS: proxied CNAME `origin-testnet` → `boing-origin-access-proxy.<workers-subdomain>.workers.dev`.

TCP **4001** is unaffected (not HTTP, not this Worker).
