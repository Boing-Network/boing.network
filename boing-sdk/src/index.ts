/**
 * Boing SDK — TypeScript/JavaScript client for Boing Network.
 *
 * Provides a typed RPC client for all node methods (chain height, balance, account,
 * blocks, proofs, simulation, submit, QA check, faucet, etc.) and hex utilities.
 *
 * Submit txs with `client.submitTransaction(hexSignedTx)` where `hexSignedTx` is
 * 0x + bincode(`SignedTransaction`). Build and sign in JS via `signTransactionInput` /
 * `buildTransferTransaction` (see `transactionBuilder.ts`), or use Boing Express
 * `boing_sendTransaction`, or the Rust CLI.
 */

export const SDK_VERSION = '0.1.0';

import { BoingClient } from './client.js';
export { BoingClient } from './client.js';
export type { BoingClientConfig } from './client.js';
export { BoingRpcError, explainBoingRpcError } from './errors.js';
export {
  ensureHex,
  bytesToHex,
  hexToBytes,
  accountIdToHex,
  hexToAccountId,
  validateHex32,
} from './hex.js';
export type {
  AccountBalance,
  AccountState,
  AccountProof,
  Block,
  BlockHeader,
  ExecutionLog,
  ExecutionReceipt,
  VerifyProofResult,
  SimulateResult,
  SubmitTransactionResult,
  RegisterDappResult,
  SubmitIntentResult,
  QaCheckResult,
  QaCheckResponse,
  QaPoolConfigResult,
  QaPoolItemSummary,
  QaPoolListResult,
  QaPoolVoteResult,
  OperatorApplyQaPolicyResult,
  QaRegistryResult,
  FaucetResult,
  JsonRpcResponse,
  SyncState,
  AccessListJson,
  ContractStorageWord,
  GetLogsFilter,
  RpcLogEntry,
} from './types.js';
export {
  SELECTOR_TRANSFER,
  SELECTOR_MINT_FIRST,
  encodeReferenceTransferCalldata,
  encodeReferenceMintFirstCalldata,
  encodeReferenceTransferCalldataHex,
} from './referenceToken.js';
export {
  SELECTOR_NATIVE_AMM_SWAP,
  SELECTOR_NATIVE_AMM_ADD_LIQUIDITY,
  SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY,
  encodeNativeAmmSwapCalldata,
  encodeNativeAmmAddLiquidityCalldata,
  encodeNativeAmmRemoveLiquidityCalldata,
  encodeNativeAmmSwapCalldataHex,
  encodeNativeAmmAddLiquidityCalldataHex,
  constantProductAmountOut,
} from './nativeAmm.js';
export {
  SELECTOR_OWNER_OF,
  SELECTOR_TRANSFER_NFT,
  SELECTOR_SET_METADATA_HASH,
  encodeReferenceOwnerOfCalldata,
  encodeReferenceTransferNftCalldata,
  encodeReferenceSetMetadataHashCalldata,
  encodeReferenceOwnerOfCalldataHex,
} from './referenceNft.js';
export {
  accountsFromSuggestedAccessList,
  mergeAccessListWithSimulation,
  accessListFromSimulation,
  simulationCoversSuggestedAccessList,
} from './accessList.js';
export {
  normalizeTopicWord,
  normalizeExecutionLog,
  logTopic0,
  iterReceiptLogs,
  logMatchesTopicFilter,
  filterReceiptLogsByTopic0,
  iterBlockReceiptLogs,
} from './receiptLogs.js';
export type { ReceiptLogRef } from './receiptLogs.js';
export {
  PayloadVariant,
  concatBytes,
  writeU32Le,
  writeU64Le,
  writeU128Le,
  encodeAccessList,
  encodeByteVec,
  encodeBincodeString,
  encodeOptionFixed32,
  encodeOptionByteVec,
  encodeOptionString,
  encodeTransactionPayload,
  encodeTransaction,
  encodeSignature,
  encodeSignedTransaction,
  signableTransactionHash,
} from './bincode.js';
export type { TransactionInput, TransactionPayloadInput } from './bincode.js';
export {
  buildTransferTransaction,
  buildContractCallTransaction,
  buildDeployWithPurposeTransaction,
  fetchNextNonce,
  senderHexFromSecretKey,
  signTransactionInput,
  signTransactionInputWithSigner,
} from './transactionBuilder.js';
export type { BuildTransferInput, BuildContractCallInput, BuildDeployWithPurposeInput, Ed25519SecretKey32 } from './transactionBuilder.js';
export {
  submitTransferWithSimulationRetry,
  submitContractCallWithSimulationRetry,
  submitDeployWithPurposeFlow,
  SimulationFailedError,
} from './submitFlow.js';
export type {
  SubmitTransferWithSimulationOptions,
  SubmitContractCallWithSimulationOptions,
  SubmitFlowResult,
} from './submitFlow.js';

/**
 * Create a Boing RPC client.
 * @param config - Node URL string (e.g. "http://localhost:8545") or config object (baseUrl, fetch?, timeoutMs?).
 */
export function createClient(config: string | import('./client.js').BoingClientConfig): BoingClient {
  return new BoingClient(config);
}
