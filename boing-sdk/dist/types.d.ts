/**
 * Types for Boing RPC results and params.
 */
/** Effective limits advertised in `boing_health` / `boing_getNetworkInfo.rpc_surface` (mirrors node env / config). */
export interface BoingHealthRpcSurface {
    /** Max JSON-RPC batch length (`BOING_RPC_MAX_BATCH`, capped). */
    jsonrpc_batch_max: number;
    /** `BOING_RPC_WS_MAX_CONNECTIONS` (0 = unlimited). */
    websocket_max_connections: number;
    /** Node HTTP JSON-RPC rate limit RPS (0 = disabled). */
    http_rate_limit_requests_per_sec: number;
    /** `BOING_RPC_READY_MIN_PEERS` when set; otherwise `null`. */
    ready_min_peers: number | null;
    /** Effective max POST body size in MiB (`BOING_RPC_MAX_BODY_MB`). */
    http_max_body_megabytes: number;
    /** Max inclusive block span for `boing_getLogs` on this node. */
    get_logs_max_block_range: number;
    /** Max log rows returned per `boing_getLogs` call. */
    get_logs_max_results: number;
    /** Max topic filter entries for `boing_getLogs`. */
    max_log_topic_filters: number;
}
/** Cumulative counters since node process start (`boing_health.rpc_metrics`). */
export interface BoingRpcMetrics {
    rate_limited_total: number;
    json_parse_errors_total: number;
    batch_too_large_total: number;
    method_not_found_total: number;
    websocket_cap_rejects_total: number;
}
/** Result of `boing_health` — cheap liveness / version probe for load balancers and scripts. */
export interface BoingHealth {
    ok: boolean;
    client_version: string;
    /** From `BOING_CHAIN_ID` when set on the node process; otherwise `null`. */
    chain_id: number | null;
    /** From `BOING_CHAIN_NAME` when set; otherwise `null`. */
    chain_name: string | null;
    head_height: number;
    /** Present on current `boing-node`; omit on older binaries. */
    rpc_surface?: BoingHealthRpcSurface;
    /** Present on current `boing-node`; omit on older binaries. */
    rpc_metrics?: BoingRpcMetrics;
}
/** Result of `BoingClient.preflightRpc()` — one-shot readiness for dashboards and CI. */
export interface BoingRpcPreflightResult {
    health: BoingHealth;
    supportedMethodCount: number;
    /** Length of embedded catalog `methods` when `boing_getRpcMethodCatalog` exists; otherwise `null`. */
    catalogMethodCount: number | null;
    /** True when `boing_getRpcOpenApi` returns an object with an `openapi` field. */
    openApiPresent: boolean;
    /** `GET {baseUrl}/live` returned HTTP 200 when checked. */
    httpLiveOk: boolean;
    /** `GET {baseUrl}/ready` returned HTTP 200 when checked. */
    httpReadyOk: boolean;
    /** True when a two-call JSON-RPC batch returned a well-formed array (current `boing-node`). */
    jsonrpcBatchOk: boolean;
    /** `GET {baseUrl}/openapi.json` returned HTTP 200 with JSON (current `boing-node`). */
    httpOpenApiJsonOk: boolean;
    /** `GET {baseUrl}/.well-known/boing-rpc` returned HTTP 200 with JSON. */
    wellKnownBoingRpcOk: boolean;
    /** `GET {baseUrl}/live.json` parsed as `{ ok: true }` (or compatible). */
    httpLiveJsonOk: boolean;
}
/** Result of `boing_getSyncState` — committed tip; `finalized_height` matches `head_height` until the node exposes pre-commit data. */
export interface SyncState {
    head_height: number;
    finalized_height: number;
    /** Tip block hash (32-byte hex with `0x`). */
    latest_block_hash: string;
}
/** Chain-wide sums from the committed account table (`boing_getNetworkInfo.chain_native`). */
export interface ChainNativeAggregates {
    account_count: number;
    /** Sum of `balance` over all accounts (u128 decimal string). */
    total_balance: string;
    /** Sum of `stake` over all accounts (u128 decimal string). */
    total_stake: string;
    /** `total_balance + total_stake` (u128 decimal string). Not “circulating” or treasury supply. */
    total_native_held: string;
    /** Committed chain height these aggregates match. */
    as_of_height: number;
}
/** `boing_getNetworkInfo.developer` — links and discovery hints (env-overridable on the node). */
export interface NetworkDeveloperHints {
    repository_url: string;
    rpc_spec_url: string;
    dapp_integration_doc_url: string;
    sdk_npm_package: string;
    websocket: {
        path: string;
        handshake: {
            type: string;
            channel: string;
        };
        event_types: string[];
    };
    api_discovery_methods: string[];
    /** Plain-HTTP probes for orchestration (Kubernetes, Docker, load balancers). */
    http: {
        live_path: string;
        ready_path: string;
        jsonrpc_post_path: string;
        response_header_rpc_version: string;
        /** Response header for request correlation (optional on request; server may generate). */
        request_id_header: string;
        /** `POST /` accepts a JSON array of JSON-RPC requests (batch). */
        supports_jsonrpc_batch: boolean;
        /** Env var on the node controlling max batch length (default 32, cap 256). */
        jsonrpc_batch_max_env: string;
        /** Env var for optional WebSocket subscriber cap (0 = unlimited). */
        websocket_max_connections_env: string;
        /** Env var for optional `/ready` minimum P2P peer count. */
        ready_min_peers_env: string;
        /** Env var for max JSON-RPC POST body size (MiB); present on current `boing-node`. */
        jsonrpc_max_body_mb_env?: string;
        /** Plain-HTTP OpenAPI (`GET`); present on current `boing-node`. */
        openapi_http_path?: string;
        /** Discovery document (`GET`); present on current `boing-node`. */
        well_known_boing_rpc_path?: string;
        live_json_path?: string;
        ready_json_path?: string;
    };
}
/** Optional wallet-facing hints from node env (`boing_getNetworkInfo.end_user`). */
export interface NetworkEndUserHints {
    chain_display_name: string | null;
    explorer_url: string | null;
    faucet_url: string | null;
    /** Set on the node via **`BOING_CANONICAL_NATIVE_CP_POOL`** — canonical native CP pool `AccountId` (32-byte hex). */
    canonical_native_cp_pool?: string | null;
    /** Set on the node via **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** — pair-directory contract `AccountId`. */
    canonical_native_dex_factory?: string | null;
    /** **`BOING_CANONICAL_NATIVE_DEX_MULTIHOP_SWAP_ROUTER`** — native multihop (2–4 pool) swap router. */
    canonical_native_dex_multihop_swap_router?: string | null;
    /** **`BOING_CANONICAL_NATIVE_DEX_LEDGER_ROUTER_V2`** — ledger forwarder for 160-byte inner calldata (e.g. v5 `swap_to`). */
    canonical_native_dex_ledger_router_v2?: string | null;
    /** **`BOING_CANONICAL_NATIVE_DEX_LEDGER_ROUTER_V3`** — ledger forwarder for 192-byte inner calldata (e.g. v5 `remove_liquidity_to`). */
    canonical_native_dex_ledger_router_v3?: string | null;
    /** **`BOING_CANONICAL_NATIVE_AMM_LP_VAULT`** — optional LP vault contract `AccountId`. */
    canonical_native_amm_lp_vault?: string | null;
    /** **`BOING_CANONICAL_NATIVE_LP_SHARE_TOKEN`** — optional LP share token `AccountId`. */
    canonical_native_lp_share_token?: string | null;
}
/** Serializable snapshot for pinning “this is network X” in CI or local dev. */
export interface BoingNetworkProfile {
    captured_at_ms: number;
    base_url: string;
    health: BoingHealth;
    network_info: NetworkInfo | null;
    supported_methods: string[] | null;
    preflight: BoingRpcPreflightResult;
}
/** One entry in a JSON-RPC batch response array. */
export interface JsonRpcBatchResponseItem {
    jsonrpc?: string;
    id?: unknown;
    result?: unknown;
    error?: {
        code: number;
        message: string;
        data?: unknown;
    };
}
/** Result of `boing_getRpcMethodCatalog` — embedded JSON Schema-style entries per method. */
export interface RpcMethodCatalog {
    description?: string;
    methods: Array<{
        name: string;
        summary?: string;
        params?: unknown;
        result?: unknown;
    }>;
}
/** Result of `boing_getRpcOpenApi` — minimal OpenAPI 3.1 for `POST /`, `GET /ws`, `GET /live`, `GET /ready`. */
export type RpcOpenApiDocument = Record<string, unknown>;
/** Result of `boing_getNetworkInfo` — dApp discovery; see RPC-API-SPEC.md. */
export interface NetworkInfo {
    /** Present when the node process sets `BOING_CHAIN_ID` (decimal). */
    chain_id: number | null;
    /** Present when `BOING_CHAIN_NAME` is set. */
    chain_name: string | null;
    head_height: number;
    finalized_height: number;
    latest_block_hash: string;
    target_block_time_secs: number;
    client_version: string;
    consensus: {
        validator_count: number;
        model: string;
    };
    native_currency: {
        symbol: string;
        decimals: number;
    };
    chain_native: ChainNativeAggregates;
    developer: NetworkDeveloperHints;
    /** Same shape as `boing_health.rpc_surface` on current `boing-node`; omit on older binaries. */
    rpc_surface?: BoingHealthRpcSurface;
    /** Optional display URLs / names from node env (`BOING_CHAIN_DISPLAY_NAME`, etc.). */
    end_user?: NetworkEndUserHints;
    rpc: {
        not_available: string[];
        not_available_note: string;
    };
}
/** Balance and stake are u128 as decimal strings. */
export interface AccountBalance {
    balance: string;
}
export interface AccountState {
    balance: string;
    nonce: number;
    stake: string;
}
export interface BlockHeader {
    parent_hash: string;
    height: number;
    timestamp: number;
    proposer: string;
    tx_root: string;
    /** Merkle root over serialized receipts (see protocol spec). */
    receipts_root: string;
    state_root: string;
}
/** One log entry from contract execution (`LOG0`..`LOG4`). */
export interface ExecutionLog {
    topics: string[];
    data: string;
}
/** Params for `boing_getLogs` — block numbers as JSON numbers or decimal / `0x` hex strings. */
export interface GetLogsFilter {
    fromBlock: number | string;
    toBlock: number | string;
    /** When set, only logs attributed to this contract (32-byte account id hex). */
    address?: string;
    /** Per-index topic matchers: `null` = wildcard (same topic-slot rules as typical `*_getLogs` JSON-RPC). Max 4 entries. */
    topics?: (string | null)[];
}
/** One log row from `boing_getLogs` (flattened; includes block / tx placement). */
export interface RpcLogEntry {
    block_height: number;
    tx_index: number;
    tx_id: string;
    log_index: number;
    /** Emitting contract when the node can attribute it (`ContractCall` / deploy address). */
    address: string | null;
    topics: string[];
    data: string;
}
/** On-chain execution result for an included transaction (`boing_getTransactionReceipt`). */
export interface ExecutionReceipt {
    tx_id: string;
    block_height: number;
    tx_index: number;
    success: boolean;
    gas_used: number;
    return_data: string;
    logs: ExecutionLog[];
    error?: string | null;
}
export interface Block {
    /** BLAKE3 block hash (32-byte hex); RPC includes this on `boing_getBlockByHeight` / `boing_getBlockByHash`. */
    hash?: string;
    header: BlockHeader;
    transactions: unknown[];
    /** Present when fetched with `include_receipts: true` on `boing_getBlockByHeight`. */
    receipts?: (ExecutionReceipt | null)[];
}
export interface AccountProof {
    proof: string;
    root: string;
    value_hash: string;
}
export interface VerifyProofResult {
    valid: boolean;
}
/** Shape of `suggested_access_list` on `boing_simulateTransaction`. */
export interface AccessListJson {
    read: string[];
    write: string[];
}
export interface SimulateResult {
    gas_used: number;
    success: boolean;
    /** Hex-encoded contract return buffer when `success` is true. */
    return_data?: string;
    /** Emitted logs when `success` is true (contract calls). */
    logs?: ExecutionLog[];
    error?: string;
    /** Heuristic minimum accounts for parallel scheduling (Track A). */
    suggested_access_list?: AccessListJson;
    /** Whether the simulated tx’s declared access list includes every suggested account. */
    access_list_covers_suggestion?: boolean;
}
/** One 32-byte contract storage word (`boing_getContractStorage`). */
export interface ContractStorageWord {
    value: string;
}
export interface SubmitTransactionResult {
    tx_hash: string;
}
export interface RegisterDappResult {
    registered: true;
    contract: string;
    owner: string;
}
export interface SubmitIntentResult {
    intent_id: string;
}
/** QA pre-flight result. */
export type QaCheckResult = 'allow' | 'reject' | 'unsure';
export interface QaCheckResponse {
    result: QaCheckResult;
    rule_id?: string;
    message?: string;
}
export interface FaucetResult {
    ok: true;
    amount: number;
    to: string;
    message: string;
}
/** Row from `boing_qaPoolList`. */
export interface QaPoolItemSummary {
    tx_hash: string;
    bytecode_hash: string;
    deployer: string;
    allow_votes: number;
    reject_votes: number;
    age_secs: number;
}
export interface QaPoolListResult {
    items: QaPoolItemSummary[];
}
/** Result of `boing_qaPoolConfig`. */
export interface QaPoolConfigResult {
    max_pending_items: number;
    max_pending_per_deployer: number;
    review_window_secs: number;
    quorum_fraction: number;
    allow_threshold_fraction: number;
    reject_threshold_fraction: number;
    default_on_expiry: 'reject' | 'allow';
    dev_open_voting: boolean;
    administrator_count: number;
    accepts_new_pending: boolean;
    pending_count: number;
}
/** Result of `boing_qaPoolVote`. */
export interface QaPoolVoteResult {
    outcome: 'pending' | 'reject' | 'allow';
    mempool?: boolean;
    duplicate?: boolean;
    error?: string;
}
/** Result of `boing_operatorApplyQaPolicy`. */
export interface OperatorApplyQaPolicyResult {
    ok: true;
}
/**
 * Effective protocol QA rule registry from `boing_getQaRegistry` (read-only).
 * `blocklist` entries are 32-byte arrays; `scam_patterns` are byte arrays.
 */
export interface QaRegistryResult {
    max_bytecode_size: number;
    blocklist: number[][];
    scam_patterns: number[][];
    always_review_categories: string[];
    content_blocklist: string[];
}
/** JSON-RPC 2.0 response. */
export interface JsonRpcResponse<T = unknown> {
    jsonrpc: '2.0';
    id: number | string | null;
    result?: T;
    error?: {
        code: number;
        message: string;
        data?: unknown;
    };
}
//# sourceMappingURL=types.d.ts.map