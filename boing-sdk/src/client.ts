/**
 * Boing JSON-RPC client — typed methods for all node RPCs.
 */

import { BoingRpcError } from './errors.js';
import type {
  AccountBalance,
  AccountProof,
  AccountState,
  Block,
  ExecutionReceipt,
  GetLogsFilter,
  RpcLogEntry,
  FaucetResult,
  QaCheckResponse,
  QaPoolConfigResult,
  QaPoolListResult,
  QaPoolVoteResult,
  RegisterDappResult,
  SimulateResult,
  SubmitIntentResult,
  SubmitTransactionResult,
  SyncState,
  ContractStorageWord,
  VerifyProofResult,
  OperatorApplyQaPolicyResult,
  QaRegistryResult,
} from './types.js';
import { ensureHex, validateHex32 } from './hex.js';

const DEFAULT_RPC_ID = 1;
const DEFAULT_TIMEOUT_MS = 30_000;

export interface BoingClientConfig {
  baseUrl: string;
  /** Optional fetch implementation (e.g. for Node or custom headers). */
  fetch?: typeof fetch;
  /** Request timeout in ms. Default 30000. Set 0 to disable. */
  timeoutMs?: number;
  /** Merged into every JSON-RPC request (e.g. `{ 'X-Boing-Operator': token }`). */
  extraHeaders?: Record<string, string>;
}

/**
 * HTTP JSON-RPC client for a Boing node.
 * All methods return typed results; on RPC error they throw BoingRpcError (with code, message, method, and optional data).
 */
export class BoingClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;
  private readonly timeoutMs: number;
  private readonly extraHeaders: Record<string, string>;
  private id = DEFAULT_RPC_ID;

  constructor(config: string | BoingClientConfig) {
    if (typeof config === 'string') {
      this.baseUrl = config.replace(/\/$/, '');
      this.fetchImpl = globalThis.fetch;
      this.timeoutMs = DEFAULT_TIMEOUT_MS;
      this.extraHeaders = {};
    } else {
      this.baseUrl = config.baseUrl.replace(/\/$/, '');
      this.fetchImpl = config.fetch ?? globalThis.fetch;
      this.timeoutMs = config.timeoutMs ?? DEFAULT_TIMEOUT_MS;
      this.extraHeaders = { ...(config.extraHeaders ?? {}) };
    }
  }

  private async request<T>(method: string, params: unknown[] = []): Promise<T> {
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
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        ...this.extraHeaders,
      };
      const res = await this.fetchImpl(this.baseUrl, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
        signal: controller?.signal,
      });
      if (!res.ok) {
        throw new BoingRpcError(
          -32000,
          `HTTP ${res.status}: ${res.statusText}`,
          undefined,
          method
        );
      }
      const json = (await res.json()) as {
        jsonrpc?: string;
        id?: number;
        result?: T;
        error?: { code: number; message: string; data?: unknown };
      };
      if (json.error != null) {
        throw new BoingRpcError(
          json.error.code,
          json.error.message,
          json.error.data,
          method
        );
      }
      if (!('result' in json)) {
        throw new BoingRpcError(
          -32000,
          'Invalid RPC response: no result or error',
          undefined,
          method
        );
      }
      return json.result as T;
    } catch (err) {
      if (err instanceof BoingRpcError) throw err;
      if (err instanceof Error && err.name === 'AbortError') {
        throw new BoingRpcError(
          -32000,
          `Request timed out after ${this.timeoutMs}ms`,
          undefined,
          method
        );
      }
      throw new BoingRpcError(
        -32000,
        err instanceof Error ? err.message : String(err),
        undefined,
        method
      );
    } finally {
      if (timeoutId != null) clearTimeout(timeoutId);
    }
  }

  /** Current chain height (tip block number). */
  async chainHeight(): Promise<number> {
    return this.request<number>('boing_chainHeight', []);
  }

  /**
   * Committed chain tip: `head_height`, `finalized_height` (same as head today), and tip `latest_block_hash`.
   * See RPC-API-SPEC.md — finality semantics.
   */
  async getSyncState(): Promise<SyncState> {
    return this.request<SyncState>('boing_getSyncState', []);
  }

  /** Get spendable balance for an account. Params: 32-byte account ID (hex). */
  async getBalance(hexAccountId: string): Promise<AccountBalance> {
    const hex = validateHex32(hexAccountId);
    return this.request<AccountBalance>('boing_getBalance', [hex]);
  }

  /** Get full account state (balance, nonce, stake). Params: 32-byte account ID (hex). */
  async getAccount(hexAccountId: string): Promise<AccountState> {
    const hex = validateHex32(hexAccountId);
    return this.request<AccountState>('boing_getAccount', [hex]);
  }

  /** Get block by height. Returns null if not found. */
  async getBlockByHeight(height: number, includeReceipts?: boolean): Promise<Block | null> {
    const params =
      includeReceipts === true ? [height, true] : includeReceipts === false ? [height, false] : [height];
    return this.request<Block | null>('boing_getBlockByHeight', params);
  }

  /** Receipt for an included tx (`Transaction.id` hex), or `null` if unknown. */
  async getTransactionReceipt(hexTxId: string): Promise<ExecutionReceipt | null> {
    return this.request<ExecutionReceipt | null>('boing_getTransactionReceipt', [validateHex32(hexTxId)]);
  }

  /**
   * Bounded log query over committed blocks (see RPC-API-SPEC — max block span and result cap on the node).
   * Optional `address` is normalized to 32-byte hex when provided.
   */
  async getLogs(filter: GetLogsFilter): Promise<RpcLogEntry[]> {
    const payload: Record<string, unknown> = {
      fromBlock: filter.fromBlock,
      toBlock: filter.toBlock,
    };
    if (filter.address != null && filter.address !== '') {
      payload.address = validateHex32(filter.address);
    }
    if (filter.topics != null) {
      payload.topics = filter.topics;
    }
    return this.request<RpcLogEntry[]>('boing_getLogs', [payload]);
  }

  /** Get block by hash. Params: 32-byte block hash (hex). */
  async getBlockByHash(hexBlockHash: string, includeReceipts?: boolean): Promise<Block | null> {
    const hex = validateHex32(hexBlockHash);
    const params =
      includeReceipts === true ? [hex, true] : includeReceipts === false ? [hex, false] : [hex];
    return this.request<Block | null>('boing_getBlockByHash', params);
  }

  /** Get Merkle proof for an account. Params: 32-byte account ID (hex). */
  async getAccountProof(hexAccountId: string): Promise<AccountProof> {
    const hex = validateHex32(hexAccountId);
    return this.request<AccountProof>('boing_getAccountProof', [hex]);
  }

  /** Verify an account Merkle proof. Params: hex proof, hex state root. */
  async verifyAccountProof(hexProof: string, hexStateRoot: string): Promise<VerifyProofResult> {
    return this.request<VerifyProofResult>('boing_verifyAccountProof', [
      ensureHex(hexProof),
      ensureHex(hexStateRoot),
    ]);
  }

  /** Read one 32-byte VM storage slot for a contract (`SLOAD` semantics; missing → zero word). */
  async getContractStorage(hexContractId: string, hexKey32: string): Promise<ContractStorageWord> {
    return this.request<ContractStorageWord>('boing_getContractStorage', [
      validateHex32(hexContractId),
      validateHex32(hexKey32),
    ]);
  }

  /** Simulate a transaction without applying it. Params: hex-encoded signed transaction. */
  async simulateTransaction(hexSignedTx: string): Promise<SimulateResult> {
    const hex = ensureHex(hexSignedTx);
    return this.request<SimulateResult>('boing_simulateTransaction', [hex]);
  }

  /**
   * Submit a signed transaction to the mempool.
   * The hex_signed_tx must be hex-encoded bincode-serialized SignedTransaction (from Rust/CLI or future signer).
   */
  async submitTransaction(hexSignedTx: string): Promise<SubmitTransactionResult> {
    const hex = ensureHex(hexSignedTx);
    return this.request<SubmitTransactionResult>('boing_submitTransaction', [hex]);
  }

  /** Register a dApp for incentive tracking. Params: 32-byte hex contract id, 32-byte hex owner id. */
  async registerDappMetrics(hexContract: string, hexOwner: string): Promise<RegisterDappResult> {
    return this.request<RegisterDappResult>('boing_registerDappMetrics', [
      validateHex32(hexContract),
      validateHex32(hexOwner),
    ]);
  }

  /** Submit a signed intent. Params: hex-encoded signed intent. */
  async submitIntent(hexSignedIntent: string): Promise<SubmitIntentResult> {
    return this.request<SubmitIntentResult>('boing_submitIntent', [ensureHex(hexSignedIntent)]);
  }

  /**
   * Pre-flight QA check for a deployment (no submit). Matches node param order:
   * [hex_bytecode, purpose_category?, description_hash?, asset_name?, asset_symbol?].
   * When passing asset_name, include description_hash (or use a 32-byte placeholder) per RPC-API-SPEC.
   * Returns allow | reject | unsure; when reject, rule_id and message are set.
   */
  /** List pending governance QA pool items. */
  async qaPoolList(): Promise<QaPoolListResult> {
    return this.request<QaPoolListResult>('boing_qaPoolList', []);
  }

  /** Read effective QA pool governance config and `pending_count`. */
  async qaPoolConfig(): Promise<QaPoolConfigResult> {
    return this.request<QaPoolConfigResult>('boing_qaPoolConfig', []);
  }

  /** Read-only: effective QA rule registry JSON (same shape as `qa_registry.json`). No auth. */
  async getQaRegistry(): Promise<QaRegistryResult> {
    return this.request<QaRegistryResult>('boing_getQaRegistry', []);
  }

  /**
   * Vote on a pooled Unsure deploy. `voter` must be a governance administrator unless the node uses dev_open_voting.
   * Params: tx_hash hex, voter account hex, `allow` | `reject` | `abstain`.
   */
  async qaPoolVote(
    txHashHex: string,
    voterHex: string,
    vote: 'allow' | 'reject' | 'abstain'
  ): Promise<QaPoolVoteResult> {
    return this.request<QaPoolVoteResult>('boing_qaPoolVote', [
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
  async operatorApplyQaPolicy(registryJson: string, qaPoolConfigJson: string): Promise<OperatorApplyQaPolicyResult> {
    return this.request<OperatorApplyQaPolicyResult>('boing_operatorApplyQaPolicy', [registryJson, qaPoolConfigJson]);
  }

  async qaCheck(
    hexBytecode: string,
    purposeCategory?: string,
    descriptionHash?: string,
    assetName?: string,
    assetSymbol?: string
  ): Promise<QaCheckResponse> {
    const hex = ensureHex(hexBytecode);
    const params: string[] = [hex];
    if (purposeCategory != null) {
      params.push(purposeCategory);
      if (descriptionHash != null) {
        params.push(ensureHex(descriptionHash));
        if (assetName != null) {
          params.push(assetName);
          if (assetSymbol != null) params.push(assetSymbol);
        }
      }
    }
    return this.request<QaCheckResponse>('boing_qaCheck', params);
  }

  /** Request testnet BOING (only when node is started with --faucet-enable). Params: 32-byte account ID (hex). Rate limited per account. */
  async faucetRequest(hexAccountId: string): Promise<FaucetResult> {
    const hex = validateHex32(hexAccountId);
    return this.request<FaucetResult>('boing_faucetRequest', [hex]);
  }
}
