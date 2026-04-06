/**
 * Boing JSON-RPC client — typed methods for all node RPCs.
 */
import type { AccountBalance, AccountProof, AccountState, Block, ExecutionReceipt, GetLogsFilter, RpcLogEntry, FaucetResult, QaCheckResponse, QaPoolConfigResult, QaPoolListResult, QaPoolVoteResult, RegisterDappResult, SimulateResult, SubmitIntentResult, SubmitTransactionResult, SyncState, NetworkInfo, BoingHealth, RpcMethodCatalog, RpcOpenApiDocument, BoingRpcPreflightResult, ContractStorageWord, VerifyProofResult, OperatorApplyQaPolicyResult, QaRegistryResult, JsonRpcBatchResponseItem } from './types.js';
export interface BoingClientConfig {
    baseUrl: string;
    /** Optional fetch implementation (e.g. for Node or custom headers). */
    fetch?: typeof fetch;
    /** Request timeout in ms. Default 30000. Set 0 to disable. */
    timeoutMs?: number;
    /** Merged into every JSON-RPC request (e.g. `{ 'X-Boing-Operator': token }`). */
    extraHeaders?: Record<string, string>;
    /**
     * Extra attempts after the first failure for transient errors (HTTP 429/502/503/504, RPC -32016,
     * timeouts, network). Default **0** (no retries).
     */
    maxRetries?: number;
    /** Backoff base in ms before each retry: wait `retryBaseDelayMs * 2^attempt`. Default 250. */
    retryBaseDelayMs?: number;
    /**
     * When true, sends **`X-Request-Id`** on each HTTP call (fresh UUID per request). Nodes echo it for log correlation.
     */
    generateRequestId?: boolean;
}
/**
 * HTTP JSON-RPC client for a Boing node.
 * All methods return typed results; on RPC error they throw BoingRpcError (with code, message, method, and optional data).
 */
export declare class BoingClient {
    private readonly baseUrl;
    private readonly fetchImpl;
    private readonly timeoutMs;
    private readonly extraHeaders;
    private readonly maxRetries;
    private readonly retryBaseDelayMs;
    private readonly generateRequestId;
    private id;
    /** From the last JSON-RPC **`POST /`** response header **`x-request-id`** (when present). */
    private lastXRequestId;
    constructor(config: string | BoingClientConfig);
    /** Normalized RPC origin (no trailing slash). */
    getBaseUrl(): string;
    /**
     * Correlation id from the most recent JSON-RPC **`POST /`** response (**`x-request-id`**).
     * Useful for support UI when CORS exposes this header on the response.
     */
    getLastXRequestId(): string | undefined;
    private recordRequestIdFromResponse;
    private rpcExtraHeaders;
    private requestOnce;
    private request;
    private requestBatchOnce;
    /**
     * POST a JSON-RPC **batch** (JSON array of requests). Assigns monotonic numeric `id`s; returns the response array in order.
     */
    requestBatch(calls: ReadonlyArray<{
        method: string;
        params?: unknown[];
    }>): Promise<JsonRpcBatchResponseItem[]>;
    /**
     * `GET {baseUrl}/live` — process is up (no chain read; for orchestrator liveness).
     * Uses the same `extraHeaders` as JSON-RPC (e.g. auth in front of the node).
     */
    checkHttpLive(): Promise<boolean>;
    /**
     * `GET {baseUrl}/ready` — node can serve RPC (read lock on state). Returns false on **503** when
     * the node enforces **`BOING_RPC_READY_MIN_PEERS`** and peer count is too low.
     */
    checkHttpReady(): Promise<boolean>;
    /**
     * One round-trip sanity check for dashboards and CI: `health`, supported method count,
     * optional catalog size, whether OpenAPI JSON is exposed, plain HTTP `/live` + `/ready`,
     * and a small JSON-RPC **batch** probe (`jsonrpcBatchOk`).
     */
    preflightRpc(): Promise<BoingRpcPreflightResult>;
    /** `GET {baseUrl}/openapi.json` — same document as `boing_getRpcOpenApi` when the node exposes it. */
    checkHttpOpenApiJson(): Promise<boolean>;
    /** `GET {baseUrl}/.well-known/boing-rpc` — path hints for HTTP discovery. */
    checkWellKnownBoingRpc(): Promise<boolean>;
    /** `GET {baseUrl}/live.json` — JSON liveness probe. */
    checkHttpLiveJson(): Promise<boolean>;
    /**
     * **`GET {baseUrl}/openapi.json`** — same OpenAPI document as **`boing_getRpcOpenApi`**, without a JSON-RPC round-trip.
     * Prefer this in browser devtools panels; throws **`BoingRpcError`** when the response is not OK or not JSON.
     */
    fetchOpenApiViaHttp(): Promise<RpcOpenApiDocument>;
    /** Current chain height (tip block number). */
    chainHeight(): Promise<number>;
    /** Build identity string for this node (e.g. `boing-node/0.1.0`). Params: `[]`. */
    clientVersion(): Promise<string>;
    /**
     * Alphabetically sorted `boing_*` method names implemented by this binary (discovery). Params: `[]`.
     */
    rpcSupportedMethods(): Promise<string[]>;
    /** Embedded JSON Schema-style catalog for codegen (params `[]`). See `boing_getNetworkInfo.developer`. */
    getRpcMethodCatalog(): Promise<RpcMethodCatalog>;
    /** Minimal OpenAPI 3.1 document for `POST /` JSON-RPC and `GET /ws` (params `[]`). */
    getRpcOpenApi(): Promise<RpcOpenApiDocument>;
    /**
     * Liveness and build identity (params `[]`). Prefer this over `boing_chainHeight` for load balancers:
     * includes `client_version`, optional `chain_id` / `chain_name` from node env, and `head_height`.
     */
    health(): Promise<BoingHealth>;
    /**
     * Committed chain tip: `head_height`, `finalized_height` (same as head today), and tip `latest_block_hash`.
     * See RPC-API-SPEC.md — finality semantics.
     */
    getSyncState(): Promise<SyncState>;
    /**
     * Network + tip snapshot for dApps (params `[]`). Includes `chain_native` (sums over committed accounts) and
     * `rpc.not_available` for capabilities this surface does not expose (e.g. staking APY).
     * See RPC-API-SPEC.md — `chain_id` / `chain_name` require node env `BOING_CHAIN_ID` / `BOING_CHAIN_NAME`.
     */
    getNetworkInfo(): Promise<NetworkInfo>;
    /** Get spendable balance for an account. Params: 32-byte account ID (hex). */
    getBalance(hexAccountId: string): Promise<AccountBalance>;
    /** Get full account state (balance, nonce, stake). Params: 32-byte account ID (hex). */
    getAccount(hexAccountId: string): Promise<AccountState>;
    /** Get block by height. Returns null if not found. */
    getBlockByHeight(height: number, includeReceipts?: boolean): Promise<Block | null>;
    /** Receipt for an included tx (`Transaction.id` hex), or `null` if unknown. */
    getTransactionReceipt(hexTxId: string): Promise<ExecutionReceipt | null>;
    /**
     * Bounded log query over committed blocks (see RPC-API-SPEC — max block span and result cap on the node).
     * Optional `address` is normalized to 32-byte hex when provided.
     */
    getLogs(filter: GetLogsFilter): Promise<RpcLogEntry[]>;
    /** Get block by hash. Params: 32-byte block hash (hex). */
    getBlockByHash(hexBlockHash: string, includeReceipts?: boolean): Promise<Block | null>;
    /** Get Merkle proof for an account. Params: 32-byte account ID (hex). */
    getAccountProof(hexAccountId: string): Promise<AccountProof>;
    /** Verify an account Merkle proof. Params: hex proof, hex state root. */
    verifyAccountProof(hexProof: string, hexStateRoot: string): Promise<VerifyProofResult>;
    /** Read one 32-byte VM storage slot for a contract (`SLOAD` semantics; missing → zero word). */
    getContractStorage(hexContractId: string, hexKey32: string): Promise<ContractStorageWord>;
    /** Simulate a transaction without applying it. Params: hex-encoded signed transaction. */
    simulateTransaction(hexSignedTx: string): Promise<SimulateResult>;
    /**
     * Submit a signed transaction to the mempool.
     * The hex_signed_tx must be hex-encoded bincode-serialized SignedTransaction (from Rust/CLI or future signer).
     */
    submitTransaction(hexSignedTx: string): Promise<SubmitTransactionResult>;
    /** Register a dApp for incentive tracking. Params: 32-byte hex contract id, 32-byte hex owner id. */
    registerDappMetrics(hexContract: string, hexOwner: string): Promise<RegisterDappResult>;
    /** Submit a signed intent. Params: hex-encoded signed intent. */
    submitIntent(hexSignedIntent: string): Promise<SubmitIntentResult>;
    /**
     * Pre-flight QA check for a deployment (no submit). Matches node param order:
     * [hex_bytecode, purpose_category?, description_hash?, asset_name?, asset_symbol?].
     * When passing asset_name, include description_hash (or use a 32-byte placeholder) per RPC-API-SPEC.
     * Returns allow | reject | unsure; when reject, rule_id and message are set.
     */
    /** List pending governance QA pool items. */
    qaPoolList(): Promise<QaPoolListResult>;
    /** Read effective QA pool governance config and `pending_count`. */
    qaPoolConfig(): Promise<QaPoolConfigResult>;
    /** Read-only: effective QA rule registry JSON (same shape as `qa_registry.json`). No auth. */
    getQaRegistry(): Promise<QaRegistryResult>;
    /**
     * Vote on a pooled Unsure deploy. `voter` must be a governance administrator unless the node uses dev_open_voting.
     * Params: tx_hash hex, voter account hex, `allow` | `reject` | `abstain`.
     */
    qaPoolVote(txHashHex: string, voterHex: string, vote: 'allow' | 'reject' | 'abstain'): Promise<QaPoolVoteResult>;
    /**
     * Apply QA registry and pool governance config on the node (operator RPC).
     * Params are full JSON documents as strings (same format as `qa_registry.json` / `qa_pool_config.json`).
     * Requires `X-Boing-Operator` when the node has `BOING_OPERATOR_RPC_TOKEN` set.
     */
    operatorApplyQaPolicy(registryJson: string, qaPoolConfigJson: string): Promise<OperatorApplyQaPolicyResult>;
    qaCheck(hexBytecode: string, purposeCategory?: string, descriptionHash?: string, assetName?: string, assetSymbol?: string): Promise<QaCheckResponse>;
    /** Request testnet BOING (only when node is started with --faucet-enable). Params: 32-byte account ID (hex). Rate limited per account. */
    faucetRequest(hexAccountId: string): Promise<FaucetResult>;
}
//# sourceMappingURL=client.d.ts.map