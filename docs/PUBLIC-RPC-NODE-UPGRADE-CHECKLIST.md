# Public RPC node — upgrade checklist (operators)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


**Routing:** [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) — where this checklist sits relative to **infra** and **testnet** guides.

Use this when you are about to **replace or restart** the `boing-node` process behind **public testnet JSON-RPC**. Canonical hosted path: Fly **`boing-testnet-1`** behind `https://testnet-rpc.boing.network/` ([FLY-IO.md](FLY-IO.md), [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md)).

---

## Before the change

1. **Note current height** (for sanity after upgrade):  
   `curl` **`boing_chainHeight`** against the public URL or use **`npm run check-testnet-rpc`** with **`BOING_RPC_URL`**.
2. **Build from the commit** you intend to ship: `cargo build --release -p boing-node` (or use CI / Fly remote builder).
3. **Run tests** on that tree: `cargo test` (and `boing-sdk` **`npm test`** if RPC or SDK behavior changed).

---

## Deploy / restart (hosted Fly — preferred)

1. Keep **`fly.testnet-1.toml`** VM size at **performance-2x / 16 GiB** (smaller sizes OOM on large tip blocks / Puts).
2. Preserve secrets: operator token, **`BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1200`**, **`BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=1306`**, `BOING_RPC_MAX_BODY_MB` (large Puts), blob download tokens if set.
3. Deploy: `fly deploy --config fly.testnet-1.toml --remote-only --ha=false` (see [FLY-IO.md](FLY-IO.md)).
4. Confirm `GET /live` on Fly and `GET https://testnet-rpc.boing.network/__gateway/health` (single backend).

Env/flag reminders that still apply on Fly (`[env]` / secrets):

- **`BOING_CHAIN_ID=6913`**, **`BOING_CHAIN_NAME=Boing Testnet`**
- DEX discovery: **`BOING_CANONICAL_NATIVE_DEX_FACTORY`**, scan/decimals knobs ([RPC-API-SPEC.md](RPC-API-SPEC.md))
- Micro fees: **`BOING_MICRO_FEES_ACTIVATION_HEIGHT`** + apply policy ([OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md))

Template: [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example).

---

## Self-hosted / lab restart (optional)

1. **Same genesis / data dir** as the rest of your lab net (unless this is an intentional reset).
2. **Flags** aligned with prior ops: **`--validator`**, **`--faucet-enable`**, **`--p2p-listen`**, **`--bootnodes`**, **`BOING_RATE_PROFILE`**, etc. ([RUNBOOK.md](RUNBOOK.md) §8.1).
3. If you expose a **private** hostname via Cloudflare Tunnel, confirm **`cloudflared`** still points at the RPC port — **do not** retarget production `testnet-rpc` ([INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md)).

### Replace `boing-node` from the official GitHub zip (CLI, on the lab host)

No Cloudflare **API** or **`cloudflared`** subcommand installs **`boing-node`**. Upgrade means replacing the binary on the host that listens on **8545**, then restarting the process.

From a clone of **`Boing-Network/boing.network`**:

1. **Stop** the running node (and free the exe on Windows if needed).
2. **Windows (PowerShell):** `.\scripts\upgrade-boing-node-from-release.ps1` (optional `-WithCli`, `-Force`, `-Tag` + `-ExpectedSha256`).
3. **Linux x86_64 / macOS Apple Silicon:** `./scripts/upgrade-boing-node-from-release.sh` (optional `BOING_WITH_CLI=1`, `BOING_FORCE=1`, `BOING_EXPECT_SHA256=…`).
4. **Restart** (e.g. **`scripts/start-bootnode-1.bat`** / **`.sh`**).

Alternatively: **`cargo build --release -p boing-node`**.

---

## After the change (from the internet)

1. **`npm run preflight-rpc`** from repo root (or **`examples/native-boing-tutorial`**) with **`BOING_RPC_URL=https://your-public-rpc/`** — runs **`check-testnet-rpc`** plus a one-shot **`boing_chainHeight` / `boing_getSyncState`** sample. Optional **`BOING_PROBE_FULL=1`**. Or run **`npm run check-testnet-rpc`** only.
2. **`boing_getNetworkInfo`** — confirm **`result.chain_id`** is **6913** and **`chain_name`** matches your operator string ( **`curl`** JSON-RPC or **`npm run rpc-endpoint-check`** from repo root).
3. **Smoke methods** you rely on (faucet, **`boing_getLogs`**, receipts) — see [RPC-API-SPEC.md](RPC-API-SPEC.md).
4. If **HTTP 530** / **error code 1033** appears, check Worker DNS + Fly `/live` (not clients) — [RUNBOOK.md](RUNBOOK.md) §8.3.

---

## Rollback

Keep the **previous binary** (or Docker image tag) available; restore the prior **`boing-node`** version and restart. Re-run **After the change** checks.

---

## Repo CI parity

**`.github/workflows/boing-sdk-rpc-integration.yml`** runs (against a local **`boing-node`** on **8545** with **`BOING_CHAIN_ID=6913`** / **`BOING_CHAIN_NAME=Boing Testnet`**): **`check-testnet-rpc`**, **`boing-sdk`** **`verify`** with **`BOING_EXPECT_CHAIN_ID`**, **`indexer-ingest-tick`**, then **`BOING_POLL_ONCE=1 node scripts/observer-chain-tip-poll.mjs`** and **`node scripts/native-amm-print-contract-call-tx.mjs`** with dummy accounts (JSON **`ok`** check). Match that flow on your staging host when possible.
