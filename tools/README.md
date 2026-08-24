# Boing network tooling

## Public testnet `boing-node` operator snippets

| File | Use |
|------|-----|
| [`boing-node-public-testnet.env.example`](./boing-node-public-testnet.env.example) | Process env: **`BOING_CHAIN_*`**, **`BOING_CANONICAL_NATIVE_*`**, **`BOING_DEX_*`** (DEX discovery RPC tuning). |
| [`boing-node-public-testnet.systemd.example`](./boing-node-public-testnet.systemd.example) | **`systemd`** unit: **`EnvironmentFile=`** for `/etc/boing-node/testnet.env` + **`ExecStart`** with bootnodes/RPC flags. |
| [`boing-node-public-testnet.docker-compose.yml`](./boing-node-public-testnet.docker-compose.yml) | **Docker Compose** (Linux **`network_mode: host`**) + **`env_file: ./.env`**. |
| [`boing-node-public-testnet.kubernetes.example.yaml`](./boing-node-public-testnet.kubernetes.example.yaml) | **Kubernetes** `ConfigMap` + **`Deployment`** with **`envFrom`**. |
| [Fly.io hosted testnet](../docs/FLY-IO.md) | **`Dockerfile`** + [`deploy/fly/`](../deploy/fly/) + [`scripts/fly-deploy-testnet.sh`](../scripts/fly-deploy-testnet.sh) + [`workers/public-rpc-gateway`](../workers/public-rpc-gateway/). |

**VibeMiner / public testnet checks (repo root `package.json`):** **`npm run vibeminer-public-testnet-preflight`**, **`npm run compare-local-public-tip`** — see [docs/VIBEMINER-INTEGRATION.md](../docs/VIBEMINER-INTEGRATION.md).

Docs: [docs/PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](../docs/PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md), [docs/RUNBOOK.md](../docs/RUNBOOK.md) § public RPC.

## `boing-vm-assemble.mjs`

Assembles line-oriented mnemonics into Boing VM bytecode (see `crates/boing-execution/src/bytecode.rs`). Does **not** assign gas; use the technical spec for opcode costs.

```bash
node tools/boing-vm-assemble.mjs tools/examples/stop-only.asm
# → 0x00
```

**Source map (debug hook):** emit a JSON sidecar mapping source line numbers to bytecode ranges (for tests or a future simulator UI):

```bash
node tools/boing-vm-assemble.mjs --map=tools/examples/stop-only.boing.map.json tools/examples/stop-only.asm
```

See header comment in `boing-vm-assemble.mjs` for mnemonic list and `PUSH` forms.

## `boing-vm-transpile-ir.mjs`

JSON **mini-IR** (versioned document + `ops` array) → bytecode hex. Supports labels and `push_jumpdest` for jumps. Spec: [`docs/BOING-MINI-IR.md`](../docs/BOING-MINI-IR.md).

```bash
node tools/boing-vm-transpile-ir.mjs tools/examples/mini-ir-stop.json
node tools/boing-vm-transpile-ir.mjs --self-test
```

## `observer-indexer-schema.sql`

SQLite / **D1**-oriented DDL for a hosted observer: **`ingest_cursor`**, **`block_height_gaps`** (pruned ranges), and minimal **`blocks` / `transactions` / `receipts` / `logs`**. Spec: [`docs/OBSERVER-HOSTED-SERVICE.md`](../docs/OBSERVER-HOSTED-SERVICE.md). Pair with **`boing-sdk`** gap helpers (`summarizeIndexerFetchGaps`, `mergeInclusiveHeightRanges`, `nextContiguousIndexedHeightAfterOmittedFetch`, `subtractInclusiveRangeFromRanges`, `blockHeightGapRowsForInsert`). Runnable loops: [`examples/observer-ingest-reference`](../examples/observer-ingest-reference/) — JSON (`npm run ingest-tick`) + SQLite (`npm run ingest-sqlite-tick` + **`BOING_SQLITE_PATH`**); [`examples/observer-d1-worker`](../examples/observer-d1-worker/) — scheduled D1 ingest. Repo root: **`npm run observer-ingest-ref-tick`**, **`npm run observer-ingest-sqlite-tick`** (install that example first).
