# Public RPC node — upgrade checklist (operators)

**Routing:** [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) — where this checklist sits relative to **infra** and **testnet** guides.

Use this when you are about to **replace or restart** the `boing-node` process behind **public testnet JSON-RPC** (direct or Cloudflare Tunnel). It complements [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) and [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md).

---

## Before the change

1. **Note current height** (for sanity after upgrade):  
   `curl` **`boing_chainHeight`** against the public URL or use **`npm run check-testnet-rpc`** with **`BOING_RPC_URL`**.
2. **Build from the commit** you intend to ship: `cargo build --release -p boing-node` (or use CI artifacts).
3. **Run tests** on that tree: `cargo test` (and `boing-sdk` **`npm test`** if RPC or SDK behavior changed).

---

## Deploy / restart

1. **Same genesis / data dir** as the rest of the testnet (unless this is an intentional reset — coordinate if so).
2. **Flags** aligned with prior ops: **`--validator`** (if this node produces blocks), **`--faucet-enable`**, **`--p2p-listen`**, **`--bootnodes`**, **`BOING_RATE_PROFILE`**, **`--pending-txs-per-sender`**, **`--max-connections-per-ip`** (P2P), etc. See [RUNBOOK.md](RUNBOOK.md) §8.1 for **signed tx gossip** and per-IP caps when P2P is enabled.
3. **Chain metadata for dApps (required for public testnet):** set **`BOING_CHAIN_ID=6913`** and **`BOING_CHAIN_NAME=Boing Testnet`** on the **`boing-node`** process so **`boing_getNetworkInfo`** returns them ([RPC-API-SPEC.md](RPC-API-SPEC.md) § **boing_getNetworkInfo**). Copy-paste template: [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example). **Infra snippets (same env semantics):** [`tools/boing-node-public-testnet.systemd.example`](../tools/boing-node-public-testnet.systemd.example) (**`EnvironmentFile=`**), [`tools/boing-node-public-testnet.docker-compose.yml`](../tools/boing-node-public-testnet.docker-compose.yml), [`tools/boing-node-public-testnet.kubernetes.example.yaml`](../tools/boing-node-public-testnet.kubernetes.example.yaml). **systemd one-liners:** `Environment=BOING_CHAIN_ID=6913` and `Environment=BOING_CHAIN_NAME=Boing Testnet` in **`[Service]`** if you prefer not to use **`EnvironmentFile=`**.
4. **DEX discovery env (recommended for public RPC):** set **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** with the pair-directory **`AccountId`**, then optional tuning from the same template — **`BOING_DEX_TOKEN_METADATA_SCAN_BLOCKS`**, **`BOING_DEX_DISCOVERY_MAX_RECEIPT_SCANS`**, **`BOING_DEX_TOKEN_DECIMALS_JSON`** — so **`boing_listDexPools`** / **`boing_listDexTokens`** labeling and scan limits are explicit ([RPC-API-SPEC.md](RPC-API-SPEC.md) § DEX discovery).
5. **Tunnel** — if using Cloudflare: confirm **`cloudflared`** still points to the RPC port where `boing-node` listens ([RUNBOOK.md](RUNBOOK.md) §8.3).

### Replace `boing-node` from the official GitHub zip (CLI only, on the tunnel origin)

**Important:** No Cloudflare **API** or **`cloudflared`** subcommand installs or upgrades **`boing-node`**. The tunnel only forwards to a **local** TCP port; the binary lives on **that host**. You avoid the Zero Trust **dashboard** by using **`cloudflared tunnel run`** with a local **`config.yml`** (see [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md)); upgrading RPC still means **replacing the process binary on the same machine**, then restarting **`boing-node`**.

From a clone of **`Boing-Network/boing.network`** on the **primary** machine (same layout as [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) — `target/release/boing-node` / `boing-node.exe`):

1. **Stop** the running node (and free the exe on Windows if needed).
2. **Windows (PowerShell):**  
   `.\scripts\upgrade-boing-node-from-release.ps1`  
   Optional: `-WithCli`, `-Force`, or another `-Tag` with `-ExpectedSha256` (zip SHA256 from `website/scripts/network-listings-release-sql.mjs <tag>`).
3. **Linux x86_64 / macOS Apple Silicon (bash):**  
   `chmod +x scripts/upgrade-boing-node-from-release.sh && ./scripts/upgrade-boing-node-from-release.sh`  
   Optional: `BOING_WITH_CLI=1 BOING_FORCE=1` or `BOING_EXPECT_SHA256=…` for tags without a built-in pin in the script.
4. **Restart** the node (e.g. **`scripts/start-bootnode-1.bat`** / **`.sh`**). **`cloudflared`** does not need a restart if the RPC port is still **8545**.

Alternatively: **`cargo build --release -p boing-node`** (and **`--no-default-features`** on Windows per bootnode scripts) instead of the zip flow.

---

## After the change (from the internet)

1. **`npm run preflight-rpc`** from repo root (or **`examples/native-boing-tutorial`**) with **`BOING_RPC_URL=https://your-public-rpc/`** — runs **`check-testnet-rpc`** plus a one-shot **`boing_chainHeight` / `boing_getSyncState`** sample. Optional **`BOING_PROBE_FULL=1`**. Or run **`npm run check-testnet-rpc`** only.
2. **`boing_getNetworkInfo`** — confirm **`result.chain_id`** is **6913** and **`chain_name`** matches your operator string ( **`curl`** JSON-RPC or **`npm run rpc-endpoint-check`** from repo root).
3. **Smoke methods** you rely on (faucet, **`boing_getLogs`**, receipts) — see [RPC-API-SPEC.md](RPC-API-SPEC.md).
4. If **HTTP 530** / **error code 1033** appears, the problem is **tunnel or origin**, not clients — [RUNBOOK.md](RUNBOOK.md) §8.3.

---

## Rollback

Keep the **previous binary** (or Docker image tag) available; restore the prior **`boing-node`** version and restart. Re-run **After the change** checks.

---

## Repo CI parity

**`.github/workflows/boing-sdk-rpc-integration.yml`** runs (against a local **`boing-node`** on **8545** with **`BOING_CHAIN_ID=6913`** / **`BOING_CHAIN_NAME=Boing Testnet`**): **`check-testnet-rpc`**, **`boing-sdk`** **`verify`** with **`BOING_EXPECT_CHAIN_ID`**, **`indexer-ingest-tick`**, then **`BOING_POLL_ONCE=1 node scripts/observer-chain-tip-poll.mjs`** and **`node scripts/native-amm-print-contract-call-tx.mjs`** with dummy accounts (JSON **`ok`** check). Match that flow on your staging host when possible.
