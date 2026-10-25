# Public RPC: WAF and rate-limit guidance

For operators who expose a `boing-node` JSON-RPC port to the internet (for example `https://testnet-rpc.boing.network/`, served by the Cloudflare Worker in [`workers/public-rpc-gateway`](../workers/public-rpc-gateway/README.md) in front of the Fly validator). It says what the node already enforces, what it deliberately does **not** enforce, and what to configure at the edge. Nothing here changes node behaviour.

## 1. What the node enforces

| Control | Where | Default | Notes |
|---------|-------|---------|-------|
| Global HTTP JSON-RPC rate limit | `RateLimitConfig.requests_per_sec` | `100` req/s (`mainnet` rate profile), `1000` on `dev` (`BOING_RATE_PROFILE`, `--dev-rate-limits`) | **One shared bucket for the whole process, not per client IP.** Excess requests get HTTP 429 with `Retry-After: 1`. The active value is in `boing_health.rpc_surface.http_rate_limit_requests_per_sec`. |
| Request body size | `BOING_RPC_MAX_BODY_MB` | 8 MiB (min 1) | Larger bodies are rejected before parsing. |
| Batch size | `BOING_RPC_MAX_BATCH` | 32 (hard cap 256) | |
| WebSocket connections | `BOING_RPC_WS_MAX_CONNECTIONS` | 512 (`0` = unlimited) | `GET /ws` newHeads subscribers. |
| Mempool per-sender and global caps | `BOING_MEMPOOL_MAX_TXS`, `--pending-txs-per-sender` | 10000 global; 16 per sender on `mainnet`, 64 on `dev`; 4 free txs per sender | Bounds what a flood of valid submits can hold. |
| Cheap admission | `submit_transaction` | about 66 us flat | Signature check, then O(1) account gates, so invalid submits are cheap to reject ([PERFORMANCE.md](PERFORMANCE.md)). |
| Operator-only methods | `BOING_OPERATOR_RPC_TOKEN`, header `X-Boing-Operator` | **required on mainnet (chain 6914); the node refuses to start without it** | Optional but strongly recommended on testnet. |
| Faucet | `--faucet-enable`, `BOING_FAUCET_MAX_PER_MINUTE` | off; 30/min when on | **Refused on mainnet.** Testnet only. |
| Blob reads | `BOING_BLOB_GET_REQUIRE_AUTH`, `BOING_BLOB_DOWNLOAD_TOKENS` | off | Enable if blob egress is a cost concern. |

Because the node's limiter is global, a single abusive client can consume the whole 100 req/s budget and starve everyone else. **Per-client limiting must happen at the edge.**

## 2. Recommended edge configuration (Cloudflare)

Adjust numbers to the traffic you actually see (`boing_health` and your edge analytics). Start in **Log** mode, review for a day, then switch to **Block** / **Managed Challenge**.

1. **Per-IP rate limit, reads.** `POST /` to the RPC hostname: about 30 requests per 10 seconds per IP, action Block for 60 s. A wallet or explorer page makes a handful of calls per second at most.
2. **Stricter limit on writes.** Match request bodies containing `boing_submitTransaction` (or use the gateway Worker to tag them): about 10 per minute per IP. The mempool per-sender caps then become a second line of defence rather than the first.
3. **Faucet.** If the faucet is exposed through the same host, limit it per IP (for example 5 per hour) in addition to `BOING_FAUCET_MAX_PER_MINUTE`, which is global. Never enable it on mainnet.
4. **Heavy reads.** `boing_getLogs`, `boing_getBlobDasSample`, `boing_getBlobDasLightClientBatch` and blob downloads cost more than a header read. Give them a separate, lower per-IP bucket, or cache `GET`-able content at the edge. DAS sample results are content-addressed, so they never go stale.
5. **Body and header caps.** Cap the request body at the edge to the same or a lower value than `BOING_RPC_MAX_BODY_MB`; block bodies that are not JSON (`Content-Type: application/json` on `POST`).
6. **Method allow-list for public hostnames.** Public RPC only needs the read methods plus `boing_submitTransaction`, and `boing_simulate*`. Block `boing_operator*`, `boing_qaPoolVote`, `boing_submitSlashAppeal`, `boing_resolveSlashAppeal`, `boing_registerBlsDualKey` and `boing_putBlobSampleOffer` at the edge unless the caller is an operator. This is defence in depth: those methods are already gated by `X-Boing-Operator`.
7. **Strip or set `X-Boing-Operator` at the edge.** Do not let arbitrary clients choose it; only operator tooling should send it, ideally over a separate hostname or Cloudflare Access policy.
8. **WebSockets.** Keep `BOING_RPC_WS_MAX_CONNECTIONS` finite and add a per-IP connection limit at the edge.
9. **Origin lock-down.** Only the gateway should reach the validator's RPC port. On Fly, expose only the gateway-facing service; keep P2P (4001) public but RPC private where possible. Gossip has its own per-peer limits ([ENVIRONMENT-VARIABLES.md](ENVIRONMENT-VARIABLES.md): `BOING_P2P_GOSSIP_*`).
10. **Retry-After.** Make the edge pass through `429` and `Retry-After` unchanged. The SDK client retries transient 429/502/503/504 responses.

### Bot Fight Mode

JSON-RPC clients are not browsers. **Bot Fight Mode** and "Browser Integrity Check" challenge them with `403` and `cf-mitigated: challenge`. Leave `fight_mode` off for the RPC hostname and use the rate-limit and allow-list rules above instead. See the 403 section of [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md).

## 3. Gateway Worker notes

- `boing_submitTransaction` is posted to every configured backend in parallel; reads use the first healthy backend. Per-IP limits therefore have to be in front of the Worker (zone rules) or inside it (Cloudflare Rate Limiting binding), not on the backends.
- Responses over about 6 MiB are replaced by a JSON-RPC `-32000` error rather than truncated. Large tip blocks with deploy bytecode can hit that.
- Use `RPC_BACKENDS` to change origins; never put an operator token in Worker vars that are returned to clients.

## 4. Monitoring

- `GET /live` and `GET /ready` (set `BOING_RPC_READY_MIN_PEERS` to fail readiness when isolated).
- `boing_health`: rate limit setting, `rpc_surface`, peer count, `client_hints`.
- Watch for a rising share of HTTP 429 from the node itself. If the node is returning 429 for legitimate traffic, raise the edge cache hit rate or scale out; do not simply raise the global limit while per-IP limits are absent.

## 5. Mainnet checklist

- `BOING_OPERATOR_RPC_TOKEN` set (enforced), stored as a secret.
- `--faucet-enable` absent (enforced).
- `BOING_RATE_PROFILE=mainnet`.
- Per-IP limits and the method allow-list live at the edge **before** announcing the endpoint.
- RPC origin reachable only from the gateway.