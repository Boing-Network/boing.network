# Environment variables (boing-node, boing-cli, deploy)

Single reference for the `BOING_*` environment variables read by the Rust workspace and the Fly.io image. CLI flags take precedence over the matching variable where both exist (`boing-node --help`). Defaults below are taken from the code; "unset" means the feature is off or the compiled default applies.

> Source of truth is the code. A variable that is added to `crates/` should be added here in the same change.

## Chain identity, signing and activation heights

| Variable | Default | Meaning |
|----------|---------|---------|
| `BOING_CHAIN_ID` | unset (local dev: legacy signatures only) | Chain id. Testnet **6913**, mainnet **6914**. Enables v2 chain-bound signature verification ([BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md)) and is advertised by `boing_getNetworkInfo` / `boing_health.tx_signing`. |
| `BOING_CHAIN_NAME` | unset | Display name returned by `boing_getNetworkInfo`. |
| `BOING_NETWORK_PRESET` | auto (known chain id => preset) | `none` / `off` / `0` disables the built-in preset (e.g. a private chain that reuses chain id 6913). |
| `BOING_TX_V2_FROM` | testnet history (900) / 0 on new chains | First height at which legacy (domain-less) tx signatures are rejected. |
| `BOING_BLOCK_SIGS_FROM` | testnet history (920) / 0 on new chains | First height whose blocks must carry verified per-tx signatures. |
| `BOING_MICRO_FEES_ACTIVATION_HEIGHT` | none | Height from which `execution_gas_fee_bps` from the fee policy applies ([OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md)). |
| `BOING_NAME_REGISTRY_ACTIVATION_HEIGHT` | disabled | Enables the `.boing` name registry from this height. |
| `BOING_NAME_BLOCKS_PER_YEAR`, `BOING_NAME_GRACE_BLOCKS`, `BOING_NAME_COMMIT_MIN_AGE`, `BOING_NAME_COMMIT_MAX_AGE` | `NameRegistryParams::default()` | Name registry timing (blocks). Commit max age must be >= min age. |
| `BOING_STORAGE_TREE_V2_ACTIVATION_HEIGHT` (CLI `--storage-tree-v2-activation-height`) | unset = **off** | Consensus gate: from this height per-contract storage trees route on all 256 key bits (closes the 64-bit-prefix collision). **All nodes must agree; set only in the future.** See [STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md). |
| `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`, `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT`, `BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT`, `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`, `BOING_BLOB_ACL_ACTIVATION_HEIGHT` | see [BOING-L1-MEDIA-BLOBS.md](BOING-L1-MEDIA-BLOBS.md) | Media-blob phase gates. |

## Validators and consensus

| Variable | Default | Meaning |
|----------|---------|---------|
| `BOING_VALIDATOR_KEY` | unset | Node/validator Ed25519 secret (hex). Prefer a secret store; never commit it. (`--validator-key`.) |
| `BOING_VALIDATORS` | preset anchors on known networks | Comma-separated validator ids. |
| `BOING_VALIDATOR_SET` | `static` | `static`, `stake` or `open`. |
| `BOING_LEADER_ELECTION` | `round_robin` | `round_robin` or `vrf`. |
| `BOING_STAKE_VALIDATOR_ACTIVATION_HEIGHT`, `BOING_STAKE_VALIDATOR_TOP_N` (21), `BOING_STAKE_VALIDATOR_EPOCH_LEN`, `BOING_STAKE_VALIDATOR_MIN_STAKE` | see `validator_config.rs` | Stake-weighted set parameters. |
| `BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT` | required for `open` mode | Height at which open registration starts. |
| `BOING_OPEN_VALIDATOR_BOND`, `_DEREGISTER_DELAY`, `_EPOCH_LEN`, `_MAX`, `_LIVENESS_WINDOW`, `_MISS_THRESHOLD_BPS`, `_MIN_SAMPLES`, `_JAIL_COOLDOWN` | `OpenValidatorParams::default()` | Open-validator economics and liveness/jailing. |
| `BOING_OPEN_VALIDATOR_FALLBACK_MS` | `3000` | Fallback proposer timeout (ms). |

## RPC, mempool and abuse limits

| Variable | Default | Meaning |
|----------|---------|---------|
| `BOING_RATE_PROFILE` | `mainnet` | `dev` relaxes JSON-RPC and mempool limits (also `--dev-rate-limits`). Explicit `mainnet` wins over the flag. |
| `BOING_RPC_MAX_BODY_MB` | `8` (min 1) | Max JSON-RPC request body. |
| `BOING_RPC_MAX_BATCH` | `32` (hard cap 256) | Max requests in a JSON-RPC batch. |
| `BOING_RPC_WS_MAX_CONNECTIONS` | `512`; `0` = unlimited | Max concurrent `GET /ws` connections. |
| `BOING_RPC_READY_MIN_PEERS` | unset | `GET /ready` returns 503 below this many connected peers. |
| `BOING_RPC_CORS_EXTRA_ORIGINS` | unset | Extra comma-separated browser origins allowed by CORS. |
| `BOING_MEMPOOL_MAX_TXS` | `10000` | Global pending-tx cap (per-sender cap: 16 on the `mainnet` rate profile, 64 on `dev`; override with `--pending-txs-per-sender`; free-tx cap 4 per sender). |
| `BOING_OPERATOR_RPC_TOKEN` | unset | Shared secret for operator-only RPC methods (header `X-Boing-Operator`). **When unset, operator methods are open to anyone who can reach the RPC port - always set it on any node whose RPC is reachable beyond localhost.** **Mainnet (`BOING_CHAIN_ID=6914`): the node refuses to start without it** (testnet / devnet behaviour is unchanged: unset = open). |
| `BOING_BLOB_GET_REQUIRE_AUTH` | off (`1`/`true` enables) | Require operator/download credentials for `boing_getBlob`. |
| `BOING_BLOB_DOWNLOAD_TOKENS` | unset | Comma-separated shared secrets accepted by `GET /v1/media/blob`. |
| `BOING_FAUCET_MAX_PER_MINUTE` | `30` (`0` = unlimited) | Testnet faucet rate limit (faucet itself needs `--faucet-enable`; **never on mainnet** - the faucet key is public; with chain id 6914 `--faucet-enable` makes the node refuse to start). |
| `BOING_DEX_TOKEN_METADATA_SCAN_BLOCKS`, `BOING_DEX_DISCOVERY_MAX_RECEIPT_SCANS` (`0` = unlimited), `BOING_DEX_TOKEN_DECIMALS_JSON`, `BOING_CANONICAL_NATIVE_CP_POOL`, `_DEX_FACTORY`, `_AMM_LP_VAULT`, `_LP_SHARE_TOKEN`, `_DEX_LEDGER_ROUTER_V2`, `_DEX_LEDGER_ROUTER_V3`, `_DEX_MULTIHOP_SWAP_ROUTER` (all `BOING_CANONICAL_NATIVE_*`; 32-byte hex AccountIds) | see `native_dex_discovery.rs` | Native DEX discovery RPC tuning / overrides. |
| `BOING_DEVELOPER_REPOSITORY_URL`, `BOING_DEVELOPER_RPC_SPEC_URL`, `BOING_DEVELOPER_DAPP_DOC_URL`, `BOING_EXPLORER_URL`, `BOING_FAUCET_URL`, `BOING_CHAIN_DISPLAY_NAME` | repo / boing.network defaults | Hints returned in `boing_getNetworkInfo.end_user` / `.developer`. |

## CLI and tooling

| Variable | Used by | Meaning |
|----------|---------|---------|
| `BOING_SECRET_HEX` | `boing-cli` | Signer secret when `--secret-hex` is not given. Prefer a file/secret manager over shell history. |
| `BOING_OPERATOR_RPC_TOKEN` | `boing-cli qa apply` | Same token as the node. |
| `BOING_WRITE_POOL_VECTORS` | `boing-cli` tests | Rewrite generated pool vector files. |
| `BOING_STRICT_CP_POOL_CREATE2` | example `verify_canonical_cp_pool_create2_drift` | `1` = exit non-zero on drift. |
| `BOING_REPLAY_BLOCKS` | `testnet_replay` test | Path to a block JSONL fixture. |
| `BOING_BENCH_ACCOUNTS`, `BOING_BENCH_ITERS`, `BOING_BENCH_TOUCH` | benches | See [PERFORMANCE.md](PERFORMANCE.md). |

## Fly.io image (`deploy/fly/entrypoint.sh`)

`BOING_DATA_DIR` (`/data`), `BOING_P2P_LISTEN` (`/ip4/0.0.0.0/tcp/4001`), `BOING_RPC_PORT` (`8545`), `BOING_VALIDATOR` (`0`/`1`), `BOING_FAUCET_ENABLE` (`1` enables `--faucet-enable`), `BOING_BOOTNODES` (secret) and `BOING_EXTRA_BOOTNODES`, `BOING_PENDING_TXS_PER_SENDER` (`--pending-txs-per-sender`). See [FLY-IO.md](FLY-IO.md).

## Default ports

JSON-RPC / WebSocket: **8545** (`--rpc-port`), libp2p: **4001/tcp** (Fly image; `--p2p-listen`). Liveness: `GET /live` (`/live.json`), readiness: `GET /ready` (`/ready.json`); JSON-RPC health: `boing_health`. Also `GET /openapi.json` and `GET /.well-known/boing-rpc`.
