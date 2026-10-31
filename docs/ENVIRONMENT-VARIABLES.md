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
| `BOING_MICRO_FEES_ACTIVATION_HEIGHT` | none (testnet 6913: preset **1188**, together with a 100 bps execution-gas policy) | Height from which `execution_gas_fee_bps` from the fee policy applies ([OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md)). Consensus: needs the height and the policy on every node. |
| `BOING_VM_HARDENING_ACTIVATION_HEIGHT` | none (testnet 6913: preset **1500**; chains without a preset: from genesis) | Height from which the VM hardening rules apply: QA of the runtime bytecode returned by init code, memory-expansion gas, operand stack capped at 1024. CLI: `--vm-hardening-activation-height`. Consensus: every node must agree; below it execution is unchanged. |
| `BOING_NAME_REGISTRY_ACTIVATION_HEIGHT` | disabled | Enables the `.boing` name registry from this height. |
| `BOING_NAME_BLOCKS_PER_YEAR`, `BOING_NAME_GRACE_BLOCKS`, `BOING_NAME_COMMIT_MIN_AGE`, `BOING_NAME_COMMIT_MAX_AGE` | `NameRegistryParams::default()` | Name registry timing (blocks). Commit max age must be >= min age. |
| `BOING_P2P_MAX_GOSSIP_MESSAGE_BYTES` | `17825792` (17 MiB; clamped 64 KiB..256 MiB) | Largest gossipsub frame accepted/published. libp2p's default is 64 KiB, which cannot carry a full block (`max_txs_per_block` = 1000) or a typical inline media blob. Consensus may allow up to **256 MiB** (PHASE7); large Puts still need block-sync / sidecar paths — gossip alone is not enough. Peers still on 64 KiB drop larger frames, so roll the new build to all nodes before relying on big frames. |
| `BOING_P2P_BLOCK_SYNC_MAX_RESPONSE_BYTES` | `536870912` (512 MiB; clamped 10 MiB..2 GiB) | Largest block-sync response (one block, CBOR) this node accepts when it requests a block. libp2p's default of 10 MiB stalls a fresh testnet node at block 1312 (blocks 1313..1316 carry up to ~160 MiB of `PutBlob` data). Receive side only; not consensus. |
| `BOING_P2P_BLOCK_SYNC_MAX_REQUEST_BYTES` | `65536` (64 KiB; clamped 1 KiB..1 MiB) | Largest block-sync *request* this node accepts (hash/height enum is tiny; the cap stops a peer from forcing a large allocation on the request path). |
| `BOING_P2P_BLOCK_SYNC_TIMEOUT_SECS` | `120` (clamped 10..3600) | How long one block-sync request may take before it fails (libp2p's default is 10 s, too short for the large testnet blocks). Not consensus. |
| `BOING_P2P_GOSSIP_MAX_MSGS_PER_SEC_PER_PEER`, `BOING_P2P_GOSSIP_MAX_BYTES_PER_SEC_PER_PEER` | `500`, `67108864` (`0` = unlimited) | Per-peer token buckets (burst = 2 s) on gossip received from a forwarding peer; excess is dropped (`Ignore`, not relayed, log `gossip_peer_rate_limited`). |
| `BOING_P2P_PEER_SCORING` | on (`0`/`false`/`off` disables) | Gossipsub peer scoring + application-level validation: undecodable / badly signed gossip is `Reject`ed (not relayed) and lowers the sender's score (graylist at -80). Mesh-delivery and IP-colocation penalties are off. |
| `BOING_RECEIPT_RETENTION_BLOCKS` | `1000000` (`0` = keep all) | In-memory receipt window. Older receipts stay in the on-disk store but `boing_getTransactionReceipt` returns `null` for them; startup loads only the window. Pruned every 256 blocks. Published as `receipt_retention_blocks` (`null` = unbounded) in `boing_health` / `boing_getNetworkInfo`. |
| `BOING_MIN_WALLET_VERSION` | `0.1.18` | Advisory `client_hints.min_wallet_version` in `boing_health` / `boing_getNetworkInfo` (oldest Boing Express release that signs correctly and shows height-aware 18-decimal amounts). Never enforced. |
| `BOING_RECOMMENDED_WALLET_VERSION` | `0.1.22` | Advisory `client_hints.recommended_wallet_version`. |
| `BOING_RECOMMENDED_SDK_VERSION` | `0.5.38` | Advisory `client_hints.recommended_sdk` (`@boing-network/boing-sdk`). Update when a new SDK is published. |
| `BOING_DAS_CACHE_MAX_BYTES` | `67108864` (64 MiB; `0` disables) | Byte budget of the in-memory LRU of chunked blob bundles used by `boing_getBlobDasSample`, `boing_getBlobDasLightClientBatch` and `boing_verifyBlobDasSuccinctProof`. Blobs are content-addressed so entries never go stale; removed/expired blobs still return `null`. |
| `BOING_PERSIST_MAX_STATE_BYTES` | `34359738368` (32 GiB) | Refuse to load an `accounts.bin` larger than this (other local persistence files have fixed caps: 64 MiB per-height/meta, 256 MiB per block, 512 MiB registries). Decoding is byte-limited so a corrupt data dir fails fast instead of exhausting memory. |
| `BOING_STORAGE_TREE_V2_ACTIVATION_HEIGHT` (CLI `--storage-tree-v2-activation-height`) | unset = **off** (testnet 6913: preset **1500**) | Consensus gate: from this height per-contract storage trees route on all 256 key bits (closes the 64-bit-prefix collision). **All nodes must agree; set only in the future.** Published as `storage_tree_v2_activation_height` (`null` = not configured) in `boing_health` / `boing_getNetworkInfo`. See [STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md). |
| `BOING_ACCOUNT_TREE_V3_ACTIVATION_HEIGHT` (CLI `--account-tree-v3-activation-height`) | unset = **off** (testnet 6913: preset **1500**) | Consensus gate: from this height the global account tree routes on all 256 key bits (closes the 64-bit-prefix collision between account ids, audit #61 item 30). **All nodes must agree; set only in the future.** See [ACCOUNT-TREE-V3-ACTIVATION.md](ACCOUNT-TREE-V3-ACTIVATION.md). |
| `BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT` | unset = **never** | Reserved consensus gate for governance propose/vote/cooling **TransactionPayload** variants (planned indices 37–39). Unset/`never`/`off` keeps them rejected. See [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md). Wire variants are not shipped yet; the env documents the future switch. |
| `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`, `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT`, `BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT`, `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`, `BOING_BLOB_ACL_ACTIVATION_HEIGHT` | see [BOING-L1-MEDIA-BLOBS.md](BOING-L1-MEDIA-BLOBS.md) | Media-blob phase gates. Testnet 6913 presets: blobs **1200**, Phase 7 cap **1306** (consensus). `BOING_BLOB_ACL_ACTIVATION_HEIGHT` (live 1317) is not consensus (HTTP blob download privacy only) and is not in the preset. |

## Validators and consensus

| Variable | Default | Meaning |
|----------|---------|---------|
| `BOING_VALIDATOR_KEY` | unset | Node/validator Ed25519 secret (hex). Prefer a secret store; never commit it. (`--validator-key`.) |
| `BOING_VALIDATORS` | preset anchors on known networks | Comma-separated validator ids. |
| `BOING_VALIDATOR_SET` | `static` | `static`, `stake` or `open`. |
| `BOING_LEADER_ELECTION` | `round_robin` | `round_robin` or `vrf`. |
| `BOING_ALLOW_STUB_VRF` | `1` (allow) | When `0`/`false`, [`verify_vrf_output`] rejects 32-byte BLAKE3 stub proofs so only ECVRF verifies (audit #61 item 36). Leave enabled for local/testnet leader fallback; disable on production validators that require real ECVRF. |
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
| `BOING_OPERATOR_RPC_TOKEN` | unset | Shared secret for operator-only RPC methods (header `X-Boing-Operator`). **When unset, operator methods are rejected (fail-closed, `-32057`) and the node logs a startup warning - set it on any node where you need these methods.** **Mainnet (`BOING_CHAIN_ID=6914`): the node refuses to start without it.** Breaking change: earlier releases treated unset as open on testnet / devnet. Compared in constant time. Generate with `openssl rand -hex 32` and store it as a secret: `fly secrets set` on Fly, `.env` (mode 600) for `tools/*docker-compose.yml`, the systemd `EnvironmentFile`, or the `boing-node-operator` Secret in the Kubernetes example. |
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
