/**
 * Types for Boing RPC results and params.
 */

/** Chain height (block number). */
export type ChainHeight = number;

/** Result of `boing_getSyncState` — committed tip; `finalized_height` matches `head_height` until the node exposes pre-commit data. */
export interface SyncState {
  head_height: number;
  finalized_height: number;
  /** Tip block hash (32-byte hex with `0x`). */
  latest_block_hash: string;
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
  /** Per-index topic matchers: `null` = wildcard (same as Ethereum `eth_getLogs`). Max 4 entries. */
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
