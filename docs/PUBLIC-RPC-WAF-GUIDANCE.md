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
6. **Method allow-list for public hostnames.** Public RPC only needs the read methods plus `boing_submitTransaction`, and `boing_simulate*`. Block `boing_operator*`, `boing_submitSlashAppeal`, `boing_resolveSlashAppeal`, `boing_registerBlsDualKey` and `boing_putBlobSampleOffer` at the edge; operators use the origin. This is defence in depth: those methods are already gated by `X-Boing-Operator`. `boing_qaPoolVote` is **not** blocked on the testnet profile because the observer's vote route calls it; it sits in the tightest allowed class instead. Items 1-6 are implemented below.
7. **Strip or set `X-Boing-Operator` at the edge.** Do not let arbitrary clients choose it; only operator tooling should send it, ideally over a separate hostname or Cloudflare Access policy.
8. **WebSockets.** Keep `BOING_RPC_WS_MAX_CONNECTIONS` finite and add a per-IP connection limit at the edge.
9. **Origin lock-down.** Only the gateway should reach the validator's RPC port. On Fly, expose only the gateway-facing service; keep P2P (4001) public but RPC private where possible. Gossip has its own per-peer limits ([ENVIRONMENT-VARIABLES.md](ENVIRONMENT-VARIABLES.md): `BOING_P2P_GOSSIP_*`).
10. **Retry-After.** Make the edge pass through `429` and `Retry-After` unchanged. The SDK client retries transient 429/502/503/504 responses.

## 2a. Implemented edge policy (config-as-code)

Everything below is generated from one file, [`ops/cloudflare/public-rpc/policy.json`](../ops/cloudflare/public-rpc/policy.json). **Nothing is applied by merging**: the Cloudflare rules need an explicit apply, and the Worker is deployed manually.

| Artefact | Purpose |
|---|---|
| `ops/cloudflare/public-rpc/policy.json` | Source of truth: the class of every RPC method, the limits, the host |
| `ops/cloudflare/public-rpc/rulesets.generated.json` | Cloudflare zone rules for the selected plan profile (default `free`): the plan's rate-limit rule(s) + two custom WAF rules |
| `workers/public-rpc-gateway/src/methodPolicy.generated.ts` | Method table and limits compiled into the gateway Worker |
| `workers/public-rpc-gateway/wrangler.toml` | Generated `[[ratelimits]]` bindings between the BEGIN/END markers; `EDGE_POLICY_MODE` var |
| `scripts/public-rpc-edge.mjs` | `generate`, `check`, `apply` (dry run unless `--apply`) |
| `boing-sdk/tests/publicRpcPolicy.test.ts` | Drift test, runs in CI with the SDK tests |

**No drift.** The method set is `BOING_RPC_SUPPORTED_METHODS` (`crates/boing-node/src/rpc.rs`) unioned with `method_catalog` in `crates/boing-node/schemas/developer_api.json`. A method that is in neither an allowed class nor the explicit `blocked` list fails `node scripts/public-rpc-edge.mjs check` and the vitest drift test, so adding an RPC method forces a decision. After classifying it run `node scripts/public-rpc-edge.mjs generate`.

### Method classes and numbers (per client IP)

| Class | Methods | Limit | Reasoning |
|---|---|---|---|
| `read` | 42 cheap reads (balances, blocks, receipts, health, names, catalogs, ...) | **600 calls / 60 s** | 10 calls/s sustained is far above a wallet or explorer page (a handful per second) and is 10% of the node's global 100 req/s budget, so one IP cannot starve others |
| `heavy_read` | `boing_getLogs`, blob and DAS bytes, simulations, BLS/DAS verification, `boing_qaCheck`, ... (13) | **30 / 60 s** | CPU, memory or egress heavy; a UI needs a few per minute |
| `submit` | `boing_submitTransaction` | **30 / 60 s** | Tighter than reads. 30 per minute (one every 2 s) covers normal wallet use and small bulk sends; a bulk transfer to more than about 30 recipients in a minute from one IP will see `-32016` and must pace itself. A flood stays well inside the mempool caps (16 txs per sender on mainnet) |
| `write` | `boing_qaPoolVote`, `boing_registerDappMetrics`, `boing_submitIntent` | **10 / 60 s** | Other state-changing calls a normal user makes rarely |
| `faucet` | `boing_faucetRequest` | **3 / 60 s** | The node already allows one request per account per 60 s and 30/min globally; this stops one IP rotating accounts. Move it to `blocked` for any mainnet profile |
| `blocked` | `boing_operatorApplyFeePolicy`, `boing_operatorApplyQaPolicy`, `boing_submitSlashAppeal`, `boing_resolveSlashAppeal`, `boing_registerBlsDualKey`, `boing_putBlobSampleOffer` | never | Operator-authenticated on the node (`-32057`); not reachable from the open internet at all. Unknown (non-catalog) methods are rejected too |

Batches are counted **per call**, so a 32-call batch cannot dodge a limit, and batches above **32** calls are rejected (the node's own cap is also 32).

#### Who enforces what (division of labour)

| Layer | Enforces | Plan-dependent? |
|---|---|---|
| **Gateway Worker** (`workers/public-rpc-gateway`) | Per-IP **per-method-class** limits from the table above, the method allow-list, the batch cap | No. Works on any Cloudflare plan, and is the only layer that can see the JSON-RPC method. **Active only once the Worker is deployed** (`npx wrangler deploy`; not done by merging) |
| **Cloudflare zone rate limit** (`http_ratelimit`) | A coarse per-IP request-volume guard, before the Worker runs | Yes: the number of rules, the usable fields and the periods depend on the plan (below) |
| **Cloudflare custom WAF rules** (`http_request_firewall_custom`) | Block unusual HTTP methods and the `X-Boing-Operator` header on the public host | Available on Free (host and method fields are allowed in custom rules) |

#### Zone rate-limit rule per Cloudflare plan

Pick the profile with `zone_rules.plan` in `policy.json` (default `free`) or `--plan free|pro|business|enterprise` on `apply`. `apply` refuses to call the API when the plan's rule budget would be exceeded (existing non-Boing rate-limit rules count too).

| Plan | Rules | Expression fields allowed (Cloudflare docs) | Boing rule(s) |
|---|---|---|---|
| **Free** (live today) | 1; period 10 s and mitigation 10 s only; `log` action is Enterprise-only | Path only (no host, no method) | **1 rule `boing_public_rpc_rl_guard`**: **100 requests / 10 s per IP** to path `/`, **block**, 10 s, HTTP 429 with a JSON-RPC `-32016` body |
| **Pro** | 2 | Host and Path (no method) | burst **60 / 10 s** (mitigate 60 s) + sustained **300 / 60 s** (mitigate 120 s) on host + path `/`, action from `rollout.zone_rule_action` (`log` first) |
| **Business** | 5 | Host, Path, Method | same two rules, matching `POST` on the host |
| **Enterprise** | per contract | everything | same two rules, matching `POST` on the host |

Why the Free guard looks the way it does:

- **100 / 10 s (10 requests/s)** is roughly 3-5x a heavy page load (20-40 calls at once) and far above polling dashboards or a wallet. It is an abuse guard only; the fine-grained limits stay in the Worker. A false positive self-heals after 10 s (the Free minimum and maximum).
- **`block` from the start**: `log` is not offered on Free, and a log-only rule gives no protection. The loose threshold is what keeps blocking low-risk.
- **Path `/` and no host**: on Free the expression can only use the path. A rule that also named the host and `POST` was accepted by the API but never matched (verified: large POST bursts against the host were never limited), so the Free profile deliberately uses only supported fields. JSON-RPC is POSTed to `/` on the public host, so it is covered. The rule also covers `/` on the other proxied hostnames of the zone (`boing.network`, `www`, `pudgymon-api`): 100 homepage requests in 10 s from one IP is not human use, and the 10 s block is harmless.
- **Verified on the live zone:** with a deliberately low test threshold the rule returned 429 after the threshold and recovered after 10 s. The final rule (100 / 10 s) was burst-tested on `https://boing.network/`: 105 of 220 parallel requests returned 200, 115 returned 429, and normal requests recovered within about 10 s.

Rules that do not depend on the plan:

| Rule | Value | Reasoning |
|---|---|---|
| Non-RPC HTTP methods | block everything except POST, GET, HEAD, OPTIONS | The Worker would answer 405 anyway; this drops them before the Worker runs |
| `X-Boing-Operator` header | block on the public host | Operator RPC must use the origin; the header cannot even be tried against the public hostname |

Limits and responses: a rate-limited call gets HTTP 429 with `Retry-After` and JSON-RPC error `-32016` (the same code the node uses for rate limits, so SDK retry logic applies); a non-public method gets HTTP 403 and `-32601` with `data.reason`.

Caveats to know before applying:

- **Cloudflare plan limits.** See the plan table above (source: <https://developers.cloudflare.com/waf/rate-limiting-rules/>). Free allows one rate-limit rule, so the Free profile ships a single generous guard. **If you upgrade the plan:** set `zone_rules.plan` in `policy.json` to `pro` (or `business` / `enterprise`), run `node scripts/public-rpc-edge.mjs generate`, then `apply` and `apply --apply`. The tool removes `boing_public_rpc_rl_guard` and creates the burst and sustained rules; the other rules in the zone are untouched. The per-method limits live in the Worker precisely so they do not depend on the plan.
- **Counters are per Cloudflare data centre** (zone rules) and per location (Workers binding), so a client spread across regions gets proportionally more. This is a protective ceiling, not a billing meter.
- **Shared egress IPs.** If the observer's server-side routes (Vercel) call the public RPC, many users share a few IPs. Watch log mode first. `zone_rules.exempt_expression` exempts matching traffic from the zone rate limits (never from the blocklist), and the Worker skips its rate limits for requests presenting `X-Boing-Edge-Trusted: <EDGE_TRUSTED_TOKEN>` (set the secret with `wrangler secret put EDGE_TRUSTED_TOKEN`). Wiring the observer to send that header is not done yet.
- **Bot Fight Mode** must stay off (next section).

### Rollout order (nothing is applied by merging)

1. `node scripts/public-rpc-edge.mjs check` (must say ok), then `node scripts/public-rpc-edge.mjs apply`: a **dry run**, offline unless `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ZONE_ID` are set, in which case it also reads (never writes) the zone's current rules and shows what would be created or updated. Other rules in the zone are always kept.
2. Deploy the Worker in **log mode** (`EDGE_POLICY_MODE = "log"` is the committed default): `cd workers/public-rpc-gateway; npx wrangler deploy`. Denials are only logged (`wrangler tail`, lines with `edge_policy`) and tagged with an `X-Boing-Edge-Policy: would-deny:...` response header.
3. Apply the zone rules for your plan (the Free profile blocks from the start because Free has no `log`; paid-plan profiles start in **log** action): `CLOUDFLARE_API_TOKEN=... CLOUDFLARE_ZONE_ID=... node scripts/public-rpc-edge.mjs apply --apply`. The token needs Zone WAF edit permission on `boing.network`.
4. Watch Security Events and `wrangler tail` for a few days. Confirm that no legitimate wallet, explorer or SDK traffic would have been denied.
5. Enforce (paid plans): set `rollout.zone_rule_action` to `"block"` in `policy.json` and `EDGE_POLICY_MODE` to `"enforce"` in `wrangler.toml`, run `node scripts/public-rpc-edge.mjs generate`, commit, re-run step 2's deploy and step 3's apply.
6. **Rollback:** `node scripts/public-rpc-edge.mjs apply --remove --apply` removes only the rules this tool owns (refs starting `boing_public_rpc_`); set `EDGE_POLICY_MODE = "off"` and redeploy (or `wrangler rollback`) for the Worker.

Cloudflare validates only the syntax of the generated expressions when they are applied; it does not reject fields the plan cannot use (a Free rule using host and method was accepted but never matched). After every apply, burst-test from one IP and confirm a 429 (the explorer's and gateway's own egress IPs may behave differently, so test from an ordinary client).

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