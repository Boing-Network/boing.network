# Operations watchlist

Short, checkable lists for things operators must watch. Nothing here changes any node or Cloudflare setting by itself.
Longer background lives in [STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md),
[RUNBOOK.md](RUNBOOK.md), [PUBLIC-RPC-WAF-GUIDANCE.md](PUBLIC-RPC-WAF-GUIDANCE.md) and [FLY-IO.md](FLY-IO.md).

## 1. Testnet 4400 crossing watch (storage-tree v2, chain 6913)

Storage-tree v2 changes how contract storage is committed in the **state root**. From the activation height `H`
(proposed **4400**, recompute on the day with `node scripts/compute-activation-height.mjs`) every node must compute
the new root, or it rejects the first v2 block and forks off. **After `H` there is no rollback.**

### Before the head gets close to H

- [ ] Every validator and RPC node runs the build that knows the feature (`boing_clientVersion`).
- [ ] Every node has `BOING_STORAGE_TREE_V2_ACTIVATION_HEIGHT=<H>` (or `--storage-tree-v2-activation-height <H>`),
      restarted one at a time.
- [ ] Every node reports the same value (and none reports `null`):
      `curl -s -X POST <rpc> -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"boing_getNetworkInfo","params":[]}'`
      then read `result.storage_tree_v2_activation_height` and `result.head_height`.
      Check every public RPC, `boing-testnet-1` and any operator-run nodes (`boing-testnet-2` was retired on 2026-10-24).
- [ ] Blocks are still being produced and every node's `head_height` agrees within a block or two.
- [ ] Note the head, the time, and the last good block's `result.header.state_root`
      (`boing_getBlockByHeight`) in your ops notes as a baseline.
- [ ] Rollback plan is ready (below) and someone who can restart every node is reachable.
- [ ] Do not rehearse on live nodes. Rehearse on a throw-away devnet only.

### Deadline for rolling back

**Roll back only while the head is below H.** Procedure: unset the variable (or the flag) on every node, restart, and
confirm `storage_tree_v2_activation_height` is `null` everywhere. No state was changed, so nothing else is needed.
The chain at `H - 1` and below is byte-for-byte the legacy algorithm. If the head is within a few blocks of `H` and a
node still reports the wrong value, deciding between "fix the node" and "unset everywhere" must happen **before** the
head reaches `H`. Once the head is at or past `H`, the only fix for a misconfigured node is to restart it with the same `H`.

### At the crossing (heads H-1, H, H+1 ... H+10)

Logs (Fly: `fly logs -a boing-testnet-1`; other hosts: the node's stdout/journal):

- [ ] Blocks keep being committed past `H`; the height keeps increasing on every node.
- [ ] No `Invalid state root: expected ..., computed ...` (the `InvalidStateRoot` block-validation error) on any node.
- [ ] No repeated block-import failures, no validator that stopped voting, no peer count collapse.
- [ ] `P2P: block publish error: InsufficientPeers` is unrelated noise unless it is new and persistent
      (see RUNBOOK 8.5).

State root agreement: for heights `H-1`, `H`, `H+1` ... `H+10`, call `boing_getBlockByHeight` on **at least two
independent nodes** and compare `result.header.state_root`. They must be identical at every height. Also compare the
boing.observer recomputation if it is available.

### What a fork looks like

- One or more nodes **stop advancing** at `H - 1` while others continue.
- Their logs show `Invalid state root: expected ..., computed ...` on the first block at `H` and again on every
  following block (they cannot import the chain until restarted with the right `H`).
- `state_root` for the same height differs between nodes, or `head_height` diverges permanently.
- `storage_tree_v2_activation_height` differs between nodes or is `null` on the lagging one.

### If something looks wrong after the crossing

1. Do **not** unset or lower `H` on the healthy nodes. That would fork them off.
2. Restart the lagging node with the **same** `H` (fix the env var or flag first). It re-imports and catches up.
3. If healthy nodes themselves disagree on the root, stop and collect: the height, both roots, both
   `boing_clientVersion` values and the log lines. That is a consensus bug, not an operations problem.
4. Keep `H` in every node's config permanently. Restarts must never drop it.

### After the crossing

- [ ] Sample roots on two nodes at a few more heights over the next days, including blocks that write contract storage.
- [ ] Verify a storage proof for a slot that used to collide (`boing_getContractStorage` is unchanged; proofs now verify).
- [ ] boing.observer and indexers use 256-bit routing at `height >= H`.
- [ ] Record the dates in STORAGE-TREE-V2-ACTIVATION.md; the 14-day / 1,000-block soak clock only starts here.

## 2. Public RPC gateway (`boing-public-rpc-gateway` Worker)

While `EDGE_POLICY_MODE = "log"`, the Worker only **observes**: a request that would be refused still succeeds, and
the response carries `X-Boing-Edge-Policy: would-deny:<reason>`.

- Workers Logs (Cloudflare dashboard, Workers & Pages, the Worker, Observability): search for the `edge_policy` field.
  Each would-deny is a JSON line `{edge_policy, reason, class, method, denial, ip}`.
- Live tail without storing: `npx wrangler tail boing-public-rpc-gateway --format json` (needs `wrangler login`).
- Before switching to `enforce`: review a few days of would-deny lines. Legitimate wallets, boing.finance and the
  observer must not appear. Switching modes is a deliberate redeploy (change `EDGE_POLICY_MODE`), never an emergency edit.
- Typecheck and tests: `cd workers/public-rpc-gateway && npm ci && npm run typecheck && npm test`.

## 3. Cloudflare Free plan limits and what to do on upgrade

The edge is built to work on the **Free** plan. These limits matter (source: Cloudflare docs, checked 2026-10-26;
re-check, plans change):

| Area | Free | What it means here |
|---|---|---|
| Workers requests | 100,000 per day per account (reset 00:00 UTC) | Every JSON-RPC call to `testnet-rpc.boing.network` is one Worker request. Over the cap, Cloudflare returns error 1027 (or bypasses the Worker if the route is fail-open). Watch the daily count in the Worker's metrics. |
| Workers CPU | 10 ms per request | The gateway does a small amount of JSON parsing and policy checks; batches are capped at 32 calls. Large or malformed bodies are rejected early. Watch "exceeded CPU" in invocation statuses. |
| Subrequests | 50 per request | `boing_submitTransaction` fan-out to every origin must stay well below 50 origins (it is a handful). |
| Workers Logs | 200,000 log events per day, 3 days retention | Each request emits an invocation log, plus one line per would-deny. High traffic exhausts the budget and drops later events. Lower `head_sampling_rate` in `[observability]` if needed. For longer retention, copy samples out by hand or tail them. |
| Zone rate-limiting rules | 1 rule; period and mitigation 10 s only; no `log` action; path-only expression | One generous guard (`boing_public_rpc_rl_guard`, 100 req / 10 s per IP, block). The fine-grained per-method limits live in the Worker (rate-limit bindings RL_READ, RL_HEAVY, RL_SUBMIT, RL_WRITE, RL_FAUCET) so they do not depend on the plan. |
| Custom WAF rules | Few; fields limited | Used for blocking unusual HTTP methods and `X-Boing-Operator` on the public host. |

Rules of thumb on Free: keep `EDGE_POLICY_MODE` at `log` until reviewed, do not add another zone rate-limit rule (the API
refuses and the tool checks the budget), and do not expect multi-day logs.

### If you upgrade the plan

1. Set `zone_rules.plan` in `ops/cloudflare/public-rpc/policy.json` to `pro` (or `business` / `enterprise`).
2. `node scripts/public-rpc-edge.mjs generate`, then `check`, and review the diff of `rulesets.generated.json`.
3. Dry run, then apply with a scoped token (Zone WAF edit): `node scripts/public-rpc-edge.mjs apply` and later
   `apply --apply`. The tool replaces `boing_public_rpc_rl_guard` with the burst and sustained rules; other rules in the zone are untouched.
   Paid profiles start in **log** action; look at the matches before switching to block.
4. If you move the Worker to the **Workers Paid** plan (a separate subscription from the zone plan): the daily request cap
   and 10 ms CPU limit go away, Workers Logs retention becomes 7 days, and the log-event budget grows. Consider
   `head_sampling_rate` and `[limits] cpu_ms` only if you actually hit a limit.
5. After applying, burst-test from one ordinary client and confirm HTTP 429 (see PUBLIC-RPC-WAF-GUIDANCE.md).