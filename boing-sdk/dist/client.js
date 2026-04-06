/**
 * Boing JSON-RPC client — typed methods for all node RPCs.
 */
import { BoingRpcError, isRetriableBoingRpcError } from './errors.js';
import { parseRetryAfterMs } from './retryAfter.js';
import { ensureHex, validateHex32 } from './hex.js';
const DEFAULT_RPC_ID = 1;
const DEFAULT_TIMEOUT_MS = 30000;
const DEFAULT_RETRY_BASE_MS = 250;
function sleepMs(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
function generateClientRequestId() {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
        return crypto.randomUUID();
    }
    return `boing-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}
/**
 * HTTP JSON-RPC client for a Boing node.
 * All methods return typed results; on RPC error they throw BoingRpcError (with code, message, method, and optional data).
 */
export class BoingClient {
    constructor(config) {
        this.id = DEFAULT_RPC_ID;
        if (typeof config === 'string') {
            this.baseUrl = config.replace(/\/$/, '');
            this.fetchImpl = globalThis.fetch;
            this.timeoutMs = DEFAULT_TIMEOUT_MS;
            this.extraHeaders = {};
            this.maxRetries = 0;
            this.retryBaseDelayMs = DEFAULT_RETRY_BASE_MS;
            this.generateRequestId = false;
        }
        else {
            this.baseUrl = config.baseUrl.replace(/\/$/, '');
            this.fetchImpl = config.fetch ?? globalThis.fetch;
            this.timeoutMs = config.timeoutMs ?? DEFAULT_TIMEOUT_MS;
            this.extraHeaders = { ...(config.extraHeaders ?? {}) };
            this.maxRetries = Math.max(0, config.maxRetries ?? 0);
            this.retryBaseDelayMs = Math.max(0, config.retryBaseDelayMs ?? DEFAULT_RETRY_BASE_MS);
            this.generateRequestId = config.generateRequestId === true;
        }
    }
    /** Normalized RPC origin (no trailing slash). */
    getBaseUrl() {
        return this.baseUrl;
    }
    /**
     * Correlation id from the most recent JSON-RPC **`POST /`** response (**`x-request-id`**).
     * Useful for support UI when CORS exposes this header on the response.
     */
    getLastXRequestId() {
        return this.lastXRequestId;
    }
    recordRequestIdFromResponse(res) {
        const v = res.headers.get('x-request-id')?.trim();
        this.lastXRequestId = v && v.length > 0 ? v : undefined;
    }
    rpcExtraHeaders() {
        const h = { ...this.extraHeaders };
        if (this.generateRequestId) {
            h['X-Request-Id'] = generateClientRequestId();
        }
        return h;
    }
    async requestOnce(method, params = []) {
        const body = {
            jsonrpc: '2.0',
            id: this.id++,
            method,
            params,
        };
        const controller = this.timeoutMs > 0 ? new AbortController() : null;
        const timeoutId = controller
            ? setTimeout(() => controller.abort(), this.timeoutMs)
            : null;
        try {
            const headers = {
                'Content-Type': 'application/json',
                ...this.rpcExtraHeaders(),
            };
            const res = await this.fetchImpl(this.baseUrl, {
                method: 'POST',
                headers,
                body: JSON.stringify(body),
                signal: controller?.signal,
            });
            this.recordRequestIdFromResponse(res);
            if (!res.ok) {
                const retryAfterMs = parseRetryAfterMs(res.headers.get('retry-after'));
                let detail = (res.statusText ?? '').trim();
                let bodyText = '';
                try {
                    bodyText = (await res.text()).trim().slice(0, 400);
                    if (bodyText.length > 0) {
                        detail = detail.length > 0 ? `${detail} — ${bodyText}` : bodyText;
                    }
                }
                catch {
                    /* ignore body read errors */
                }
                let msg = detail.length > 0 ? `HTTP ${res.status}: ${detail}` : `HTTP ${res.status}`;
                if (res.status === 413) {
                    msg = `HTTP 413: JSON-RPC body exceeds this node limit (operators can raise BOING_RPC_MAX_BODY_MB). ${msg}`;
                }
                if (res.status === 429 && bodyText.length > 0) {
                    try {
                        const j = JSON.parse(bodyText);
                        if (j.error != null) {
                            throw new BoingRpcError(j.error.code, j.error.message, j.error.data, method, retryAfterMs);
                        }
                    }
                    catch (e) {
                        if (e instanceof BoingRpcError)
                            throw e;
                    }
                }
                throw new BoingRpcError(-32000, msg, undefined, method, retryAfterMs);
            }
            const json = (await res.json());
            if (json.error != null) {
                throw new BoingRpcError(json.error.code, json.error.message, json.error.data, method);
            }
            if (!('result' in json)) {
                throw new BoingRpcError(-32000, 'Invalid RPC response: no result or error', undefined, method);
            }
            return json.result;
        }
        catch (err) {
            if (err instanceof BoingRpcError)
                throw err;
            if (err instanceof Error && err.name === 'AbortError') {
                throw new BoingRpcError(-32000, `Request timed out after ${this.timeoutMs}ms`, undefined, method);
            }
            throw new BoingRpcError(-32000, err instanceof Error ? err.message : String(err), undefined, method);
        }
        finally {
            if (timeoutId != null)
                clearTimeout(timeoutId);
        }
    }
    async request(method, params = []) {
        let lastErr;
        const attempts = 1 + this.maxRetries;
        for (let attempt = 0; attempt < attempts; attempt++) {
            try {
                return await this.requestOnce(method, params);
            }
            catch (e) {
                lastErr = e;
                if (attempt >= attempts - 1 || !isRetriableBoingRpcError(e)) {
                    throw e;
                }
                const backoff = this.retryBaseDelayMs * Math.pow(2, attempt);
                const ra = e instanceof BoingRpcError ? e.retryAfterMs : undefined;
                const delay = ra != null && ra > 0 ? Math.max(backoff, ra) : backoff;
                if (delay > 0)
                    await sleepMs(delay);
            }
        }
        throw lastErr;
    }
    async requestBatchOnce(calls) {
        const BATCH = 'jsonrpc.batch';
        const batchBody = calls.map((c) => ({
            jsonrpc: '2.0',
            id: this.id++,
            method: c.method,
            params: c.params ?? [],
        }));
        const controller = this.timeoutMs > 0 ? new AbortController() : null;
        const timeoutId = controller
            ? setTimeout(() => controller.abort(), this.timeoutMs)
            : null;
        try {
            const headers = {
                'Content-Type': 'application/json',
                ...this.rpcExtraHeaders(),
            };
            const res = await this.fetchImpl(this.baseUrl, {
                method: 'POST',
                headers,
                body: JSON.stringify(batchBody),
                signal: controller?.signal,
            });
            this.recordRequestIdFromResponse(res);
            if (!res.ok) {
                const retryAfterMs = parseRetryAfterMs(res.headers.get('retry-after'));
                let detail = (res.statusText ?? '').trim();
                let bodyText = '';
                try {
                    bodyText = (await res.text()).trim().slice(0, 400);
                    if (bodyText.length > 0) {
                        detail = detail.length > 0 ? `${detail} — ${bodyText}` : bodyText;
                    }
                }
                catch {
                    /* ignore body read errors */
                }
                const msg = detail.length > 0 ? `HTTP ${res.status}: ${detail}` : `HTTP ${res.status}`;
                if (res.status === 429 && bodyText.length > 0) {
                    try {
                        const j = JSON.parse(bodyText);
                        if (j.error != null) {
                            throw new BoingRpcError(j.error.code, j.error.message, j.error.data, BATCH, retryAfterMs);
                        }
                    }
                    catch (e) {
                        if (e instanceof BoingRpcError)
                            throw e;
                    }
                }
                throw new BoingRpcError(-32000, msg, undefined, BATCH, retryAfterMs);
            }
            const json = await res.json();
            if (!Array.isArray(json)) {
                throw new BoingRpcError(-32000, 'Invalid RPC response: expected JSON array for batch', undefined, BATCH);
            }
            return json;
        }
        catch (err) {
            if (err instanceof BoingRpcError)
                throw err;
            if (err instanceof Error && err.name === 'AbortError') {
                throw new BoingRpcError(-32000, `Request timed out after ${this.timeoutMs}ms`, undefined, BATCH);
            }
            throw new BoingRpcError(-32000, err instanceof Error ? err.message : String(err), undefined, BATCH);
        }
        finally {
            if (timeoutId != null)
                clearTimeout(timeoutId);
        }
    }
    /**
     * POST a JSON-RPC **batch** (JSON array of requests). Assigns monotonic numeric `id`s; returns the response array in order.
     */
    async requestBatch(calls) {
        let lastErr;
        const attempts = 1 + this.maxRetries;
        for (let attempt = 0; attempt < attempts; attempt++) {
            try {
                return await this.requestBatchOnce(calls);
            }
            catch (e) {
                lastErr = e;
                if (attempt >= attempts - 1 || !isRetriableBoingRpcError(e)) {
                    throw e;
                }
                const backoff = this.retryBaseDelayMs * Math.pow(2, attempt);
                const ra = e instanceof BoingRpcError ? e.retryAfterMs : undefined;
                const delay = ra != null && ra > 0 ? Math.max(backoff, ra) : backoff;
                if (delay > 0)
                    await sleepMs(delay);
            }
        }
        throw lastErr;
    }
    /**
     * `GET {baseUrl}/live` — process is up (no chain read; for orchestrator liveness).
     * Uses the same `extraHeaders` as JSON-RPC (e.g. auth in front of the node).
     */
    async checkHttpLive() {
        try {
            const res = await this.fetchImpl(`${this.baseUrl}/live`, {
                method: 'GET',
                headers: this.rpcExtraHeaders(),
            });
            return res.ok;
        }
        catch {
            return false;
        }
    }
    /**
     * `GET {baseUrl}/ready` — node can serve RPC (read lock on state). Returns false on **503** when
     * the node enforces **`BOING_RPC_READY_MIN_PEERS`** and peer count is too low.
     */
    async checkHttpReady() {
        try {
            const res = await this.fetchImpl(`${this.baseUrl}/ready`, {
                method: 'GET',
                headers: this.rpcExtraHeaders(),
            });
            return res.ok;
        }
        catch {
            return false;
        }
    }
    /**
     * One round-trip sanity check for dashboards and CI: `health`, supported method count,
     * optional catalog size, whether OpenAPI JSON is exposed, plain HTTP `/live` + `/ready`,
     * and a small JSON-RPC **batch** probe (`jsonrpcBatchOk`).
     */
    async preflightRpc() {
        const [health, supported, httpLiveOk, httpReadyOk, httpOpenApiJsonOk, wellKnownBoingRpcOk, httpLiveJsonOk,] = await Promise.all([
            this.health(),
            this.rpcSupportedMethods(),
            this.checkHttpLive(),
            this.checkHttpReady(),
            this.checkHttpOpenApiJson(),
            this.checkWellKnownBoingRpc(),
            this.checkHttpLiveJson(),
        ]);
        let catalogMethodCount = null;
        try {
            const cat = await this.getRpcMethodCatalog();
            catalogMethodCount = cat.methods?.length ?? 0;
        }
        catch {
            catalogMethodCount = null;
        }
        let openApiPresent = false;
        try {
            const oa = await this.getRpcOpenApi();
            openApiPresent = typeof oa === 'object' && oa !== null && 'openapi' in oa;
        }
        catch {
            openApiPresent = false;
        }
        let jsonrpcBatchOk = false;
        try {
            const batch = await this.requestBatch([
                { method: 'boing_chainHeight', params: [] },
                { method: 'boing_clientVersion', params: [] },
            ]);
            jsonrpcBatchOk =
                batch.length === 2 &&
                    batch[0]?.error == null &&
                    batch[1]?.error == null &&
                    typeof batch[0]?.result === 'number' &&
                    typeof batch[1]?.result === 'string';
        }
        catch {
            jsonrpcBatchOk = false;
        }
        return {
            health,
            supportedMethodCount: supported.length,
            catalogMethodCount,
            openApiPresent,
            httpLiveOk,
            httpReadyOk,
            jsonrpcBatchOk,
            httpOpenApiJsonOk,
            wellKnownBoingRpcOk,
            httpLiveJsonOk,
        };
    }
    /** `GET {baseUrl}/openapi.json` — same document as `boing_getRpcOpenApi` when the node exposes it. */
    async checkHttpOpenApiJson() {
        try {
            const res = await this.fetchImpl(`${this.baseUrl}/openapi.json`, {
                method: 'GET',
                headers: this.rpcExtraHeaders(),
            });
            if (!res.ok)
                return false;
            const ct = res.headers.get('content-type') ?? '';
            if (!ct.includes('application/json'))
                return false;
            const j = await res.json();
            return typeof j === 'object' && j !== null && 'openapi' in j;
        }
        catch {
            return false;
        }
    }
    /** `GET {baseUrl}/.well-known/boing-rpc` — path hints for HTTP discovery. */
    async checkWellKnownBoingRpc() {
        try {
            const res = await this.fetchImpl(`${this.baseUrl}/.well-known/boing-rpc`, {
                method: 'GET',
                headers: this.rpcExtraHeaders(),
            });
            if (!res.ok)
                return false;
            const j = await res.json();
            return typeof j === 'object' && j !== null && 'schema_version' in j;
        }
        catch {
            return false;
        }
    }
    /** `GET {baseUrl}/live.json` — JSON liveness probe. */
    async checkHttpLiveJson() {
        try {
            const res = await this.fetchImpl(`${this.baseUrl}/live.json`, {
                method: 'GET',
                headers: this.rpcExtraHeaders(),
            });
            if (!res.ok)
                return false;
            const j = await res.json();
            return typeof j === 'object' && j !== null && j.ok === true;
        }
        catch {
            return false;
        }
    }
    /**
     * **`GET {baseUrl}/openapi.json`** — same OpenAPI document as **`boing_getRpcOpenApi`**, without a JSON-RPC round-trip.
     * Prefer this in browser devtools panels; throws **`BoingRpcError`** when the response is not OK or not JSON.
     */
    async fetchOpenApiViaHttp() {
        const res = await this.fetchImpl(`${this.baseUrl}/openapi.json`, {
            method: 'GET',
            headers: this.rpcExtraHeaders(),
        });
        if (!res.ok) {
            throw new BoingRpcError(-32000, `HTTP ${res.status}: could not load OpenAPI from /openapi.json`, undefined, 'GET /openapi.json');
        }
        const ct = res.headers.get('content-type') ?? '';
        if (!ct.includes('application/json')) {
            throw new BoingRpcError(-32000, 'GET /openapi.json: expected application/json Content-Type', undefined, 'GET /openapi.json');
        }
        const j = await res.json();
        if (typeof j !== 'object' || j === null || !('openapi' in j)) {
            throw new BoingRpcError(-32000, 'GET /openapi.json: response is not a valid OpenAPI root object', undefined, 'GET /openapi.json');
        }
        return j;
    }
    /** Current chain height (tip block number). */
    async chainHeight() {
        return this.request('boing_chainHeight', []);
    }
    /** Build identity string for this node (e.g. `boing-node/0.1.0`). Params: `[]`. */
    async clientVersion() {
        return this.request('boing_clientVersion', []);
    }
    /**
     * Alphabetically sorted `boing_*` method names implemented by this binary (discovery). Params: `[]`.
     */
    async rpcSupportedMethods() {
        return this.request('boing_rpcSupportedMethods', []);
    }
    /** Embedded JSON Schema-style catalog for codegen (params `[]`). See `boing_getNetworkInfo.developer`. */
    async getRpcMethodCatalog() {
        return this.request('boing_getRpcMethodCatalog', []);
    }
    /** Minimal OpenAPI 3.1 document for `POST /` JSON-RPC and `GET /ws` (params `[]`). */
    async getRpcOpenApi() {
        return this.request('boing_getRpcOpenApi', []);
    }
    /**
     * Liveness and build identity (params `[]`). Prefer this over `boing_chainHeight` for load balancers:
     * includes `client_version`, optional `chain_id` / `chain_name` from node env, and `head_height`.
     */
    async health() {
        return this.request('boing_health', []);
    }
    /**
     * Committed chain tip: `head_height`, `finalized_height` (same as head today), and tip `latest_block_hash`.
     * See RPC-API-SPEC.md — finality semantics.
     */
    async getSyncState() {
        return this.request('boing_getSyncState', []);
    }
    /**
     * Network + tip snapshot for dApps (params `[]`). Includes `chain_native` (sums over committed accounts) and
     * `rpc.not_available` for capabilities this surface does not expose (e.g. staking APY).
     * See RPC-API-SPEC.md — `chain_id` / `chain_name` require node env `BOING_CHAIN_ID` / `BOING_CHAIN_NAME`.
     */
    async getNetworkInfo() {
        return this.request('boing_getNetworkInfo', []);
    }
    /** Get spendable balance for an account. Params: 32-byte account ID (hex). */
    async getBalance(hexAccountId) {
        const hex = validateHex32(hexAccountId);
        return this.request('boing_getBalance', [hex]);
    }
    /** Get full account state (balance, nonce, stake). Params: 32-byte account ID (hex). */
    async getAccount(hexAccountId) {
        const hex = validateHex32(hexAccountId);
        return this.request('boing_getAccount', [hex]);
    }
    /** Get block by height. Returns null if not found. */
    async getBlockByHeight(height, includeReceipts) {
        const params = includeReceipts === true ? [height, true] : includeReceipts === false ? [height, false] : [height];
        return this.request('boing_getBlockByHeight', params);
    }
    /** Receipt for an included tx (`Transaction.id` hex), or `null` if unknown. */
    async getTransactionReceipt(hexTxId) {
        return this.request('boing_getTransactionReceipt', [validateHex32(hexTxId)]);
    }
    /**
     * Bounded log query over committed blocks (see RPC-API-SPEC — max block span and result cap on the node).
     * Optional `address` is normalized to 32-byte hex when provided.
     */
    async getLogs(filter) {
        const payload = {
            fromBlock: filter.fromBlock,
            toBlock: filter.toBlock,
        };
        if (filter.address != null && filter.address !== '') {
            payload.address = validateHex32(filter.address);
        }
        if (filter.topics != null) {
            payload.topics = filter.topics;
        }
        return this.request('boing_getLogs', [payload]);
    }
    /** Get block by hash. Params: 32-byte block hash (hex). */
    async getBlockByHash(hexBlockHash, includeReceipts) {
        const hex = validateHex32(hexBlockHash);
        const params = includeReceipts === true ? [hex, true] : includeReceipts === false ? [hex, false] : [hex];
        return this.request('boing_getBlockByHash', params);
    }
    /** Get Merkle proof for an account. Params: 32-byte account ID (hex). */
    async getAccountProof(hexAccountId) {
        const hex = validateHex32(hexAccountId);
        return this.request('boing_getAccountProof', [hex]);
    }
    /** Verify an account Merkle proof. Params: hex proof, hex state root. */
    async verifyAccountProof(hexProof, hexStateRoot) {
        return this.request('boing_verifyAccountProof', [
            ensureHex(hexProof),
            ensureHex(hexStateRoot),
        ]);
    }
    /** Read one 32-byte VM storage slot for a contract (`SLOAD` semantics; missing → zero word). */
    async getContractStorage(hexContractId, hexKey32) {
        return this.request('boing_getContractStorage', [
            validateHex32(hexContractId),
            validateHex32(hexKey32),
        ]);
    }
    /** Simulate a transaction without applying it. Params: hex-encoded signed transaction. */
    async simulateTransaction(hexSignedTx) {
        const hex = ensureHex(hexSignedTx);
        return this.request('boing_simulateTransaction', [hex]);
    }
    /**
     * Submit a signed transaction to the mempool.
     * The hex_signed_tx must be hex-encoded bincode-serialized SignedTransaction (from Rust/CLI or future signer).
     */
    async submitTransaction(hexSignedTx) {
        const hex = ensureHex(hexSignedTx);
        return this.request('boing_submitTransaction', [hex]);
    }
    /** Register a dApp for incentive tracking. Params: 32-byte hex contract id, 32-byte hex owner id. */
    async registerDappMetrics(hexContract, hexOwner) {
        return this.request('boing_registerDappMetrics', [
            validateHex32(hexContract),
            validateHex32(hexOwner),
        ]);
    }
    /** Submit a signed intent. Params: hex-encoded signed intent. */
    async submitIntent(hexSignedIntent) {
        return this.request('boing_submitIntent', [ensureHex(hexSignedIntent)]);
    }
    /**
     * Pre-flight QA check for a deployment (no submit). Matches node param order:
     * [hex_bytecode, purpose_category?, description_hash?, asset_name?, asset_symbol?].
     * When passing asset_name, include description_hash (or use a 32-byte placeholder) per RPC-API-SPEC.
     * Returns allow | reject | unsure; when reject, rule_id and message are set.
     */
    /** List pending governance QA pool items. */
    async qaPoolList() {
        return this.request('boing_qaPoolList', []);
    }
    /** Read effective QA pool governance config and `pending_count`. */
    async qaPoolConfig() {
        return this.request('boing_qaPoolConfig', []);
    }
    /** Read-only: effective QA rule registry JSON (same shape as `qa_registry.json`). No auth. */
    async getQaRegistry() {
        return this.request('boing_getQaRegistry', []);
    }
    /**
     * Vote on a pooled Unsure deploy. `voter` must be a governance administrator unless the node uses dev_open_voting.
     * Params: tx_hash hex, voter account hex, `allow` | `reject` | `abstain`.
     */
    async qaPoolVote(txHashHex, voterHex, vote) {
        return this.request('boing_qaPoolVote', [
            validateHex32(txHashHex),
            validateHex32(voterHex),
            vote,
        ]);
    }
    /**
     * Apply QA registry and pool governance config on the node (operator RPC).
     * Params are full JSON documents as strings (same format as `qa_registry.json` / `qa_pool_config.json`).
     * Requires `X-Boing-Operator` when the node has `BOING_OPERATOR_RPC_TOKEN` set.
     */
    async operatorApplyQaPolicy(registryJson, qaPoolConfigJson) {
        return this.request('boing_operatorApplyQaPolicy', [registryJson, qaPoolConfigJson]);
    }
    async qaCheck(hexBytecode, purposeCategory, descriptionHash, assetName, assetSymbol) {
        const hex = ensureHex(hexBytecode);
        const params = [hex];
        if (purposeCategory != null) {
            params.push(purposeCategory);
            if (descriptionHash != null) {
                params.push(ensureHex(descriptionHash));
                if (assetName != null) {
                    params.push(assetName);
                    if (assetSymbol != null)
                        params.push(assetSymbol);
                }
            }
        }
        return this.request('boing_qaCheck', params);
    }
    /** Request testnet BOING (only when node is started with --faucet-enable). Params: 32-byte account ID (hex). Rate limited per account. */
    async faucetRequest(hexAccountId) {
        const hex = validateHex32(hexAccountId);
        return this.request('boing_faucetRequest', [hex]);
    }
}
