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

export { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from './canonicalTestnet.js';

import { BoingClient } from './client.js';
export { BoingClient } from './client.js';
export type { BoingClientConfig } from './client.js';
export {
  BoingRpcError,
  explainBoingRpcError,
  isBoingRpcMethodNotFound,
  isRetriableBoingRpcError,
} from './errors.js';
export { parseRetryAfterMs } from './retryAfter.js';
export {
  probeBoingRpcCapabilities,
  countAvailableBoingRpcMethods,
  explainBoingRpcProbeGaps,
  type BoingRpcCapabilities,
  type BoingRpcMethodProbe,
  type BoingRpcProbeBundle,
} from './rpcCapabilities.js';
export {
  doctorBoingRpcEnvironment,
  formatBoingRpcDoctorReport,
  doctorErrorMessage,
  type BoingRpcDoctorOptions,
  type BoingRpcDoctorResult,
} from './rpcDoctor.js';
export {
  BoingRpcPreflightError,
  assertBoingRpcEnvironment,
  isBoingRpcPreflightError,
} from './preflightGate.js';
export {
  displayChainTitle,
  explorerBaseUrl,
  faucetUrl,
  explorerTxUrl,
  explorerAccountUrl,
  formatSupportHint,
} from './dappUiHelpers.js';
export {
  mergeRpcSurface,
  clampGetLogsBlockSpan,
  suggestedReadSpacingMs,
  describeGetLogsLimits,
  websocketHasConnectionCap,
} from './rpcSurfaceUi.js';
export {
  BoingConnectionMonitor,
  type BoingConnectionSnapshot,
  type BoingConnectionMonitorOptions,
} from './connectionMonitor.js';
export {
  boingRpcWebSocketUrl,
  BoingNewHeadsWs,
  type BoingNewHeadsWsState,
  type BoingNewHeadsWsOptions,
} from './newHeadsWs.js';
export { captureBoingNetworkProfile } from './networkProfile.js';
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
  NetworkInfo,
  BoingHealth,
  BoingHealthRpcSurface,
  BoingRpcMetrics,
  ChainNativeAggregates,
  NetworkDeveloperHints,
  NetworkEndUserHints,
  BoingNetworkProfile,
  JsonRpcBatchResponseItem,
  RpcMethodCatalog,
  RpcOpenApiDocument,
  BoingRpcPreflightResult,
  AccessListJson,
  ContractStorageWord,
  GetLogsFilter,
  RpcLogEntry,
} from './types.js';
export {
  type BoingCalldataWord,
  assertBoingCalldataWord,
  boingWordSelector,
  boingWordU128,
  boingWordAccount,
  boingWordFixed,
  encodeBoingCall,
  calldataSelectorLastByte,
  calldataU128BeWord,
  calldataAccountIdWord,
  calldataFixedWord32,
  concatCalldata,
} from './calldata.js';
export {
  type BoingAbiParamKind,
  type BoingAbiArg,
  type BoingCallDescriptor,
  abiArgU128,
  abiArgAccount,
  abiArgBytes32,
  abiArgBool,
  encodeBoingAbiArgToWord,
  encodeBoingCallFromAbiArgs,
  encodeBoingCallTyped,
  encodeBoingCallFromDescriptor,
  BoingReferenceCallDescriptors,
} from './callAbi.js';
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
  SELECTOR_NATIVE_AMM_SET_TOKENS,
  SELECTOR_NATIVE_AMM_SET_SWAP_FEE_BPS,
  NATIVE_CP_SWAP_FEE_BPS,
  NATIVE_AMM_TOPIC_SWAP_HEX,
  NATIVE_AMM_TOPIC_ADD_LIQUIDITY_HEX,
  NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY_HEX,
  nativeAmmLogTopic0Utf8,
  encodeNativeAmmSwapCalldata,
  encodeNativeAmmAddLiquidityCalldata,
  encodeNativeAmmRemoveLiquidityCalldata,
  encodeNativeAmmSetSwapFeeBpsCalldata,
  encodeNativeAmmSwapCalldataHex,
  encodeNativeAmmAddLiquidityCalldataHex,
  encodeNativeAmmRemoveLiquidityCalldataHex,
  encodeNativeAmmSetSwapFeeBpsCalldataHex,
  constantProductAmountOut,
  constantProductAmountOutNoFee,
  constantProductAmountOutWithFeeBps,
} from './nativeAmm.js';
export {
  buildNativeConstantProductPoolAccessList,
  buildNativeConstantProductContractCallTx,
  mergeNativePoolAccessListWithSimulation,
  type NativePoolAccessListOptions,
  NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_TOKEN_A_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_TOKEN_B_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_TOKENS_CONFIGURED_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX,
  nativeAmmLpBalanceStorageKeyHex,
  decodeBoingStorageWordU128,
  decodeNativeAmmLogDataU128Triple,
  fetchNativeConstantProductReserves,
  fetchNativeConstantProductTotalLpSupply,
  fetchNativeConstantProductSwapFeeBps,
  fetchNativeAmmSignerLpBalance,
  fetchNativeConstantProductPoolSnapshot,
  type NativeConstantProductPoolSnapshot,
} from './nativeAmmPool.js';
export {
  type NativeAmmLog2Kind,
  type NativeAmmLog2Event,
  type NativeAmmRpcLogParsed,
  isNativeAmmLog2Topic0,
  isNativeAmmLog2Shape,
  tryParseNativeAmmLog2,
  tryParseNativeAmmRpcLogEntry,
  filterMapNativeAmmRpcLogs,
  collectNativeAmmLog2FromReceipt,
} from './nativeAmmLogs.js';
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
  predictCreate2ContractAddress,
  predictNativeCpPoolCreate2Address,
  predictNativeCpPoolV2Create2Address,
  predictNativeCpPoolV3Create2Address,
  predictNativeCpPoolV4Create2Address,
  nativeCpPoolCreate2SaltV1Hex,
  nativeCpPoolCreate2SaltV2Hex,
  nativeCpPoolCreate2SaltV3Hex,
  nativeCpPoolCreate2SaltV4Hex,
  NATIVE_CP_POOL_CREATE2_SALT_V1,
  NATIVE_CP_POOL_CREATE2_SALT_V2,
  NATIVE_CP_POOL_CREATE2_SALT_V3,
  NATIVE_CP_POOL_CREATE2_SALT_V4,
} from './create2.js';
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
export {
  createNativeContractSubmitter,
  type NativeContractSubmitterConfig,
} from './nativeContractSubmit.js';
export {
  DEFAULT_GET_LOGS_MAX_BLOCK_SPAN,
  mapWithConcurrencyLimit,
  flattenReceiptsFromBundles,
  planLogBlockChunks,
  getLogsChunked,
  fetchReceiptsForBlockHeight,
  fetchReceiptsForHeightRange,
  fetchBlocksWithReceiptsForHeightRange,
  type LogChunkFilter,
  type BlockReceiptsBundle,
  type BlockWithReceiptsBundle,
  type FetchReceiptsHeightRangeMissing,
  type FetchReceiptsForHeightRangeOptions,
  type FetchBlocksWithReceiptsForHeightRangeOptions,
  type GetLogsChunkedOptions,
  type MapWithConcurrencyLimitOptions,
  summarizeIndexerFetchGaps,
  type IndexerFetchGapSummary,
} from './indexerBatch.js';
export {
  getIndexerChainTips,
  clampIndexerHeightRange,
  planIndexerChainTipsWithFallback,
  planIndexerCatchUp,
  type IndexerChainTips,
  type IndexerTipsSource,
  type IndexerCatchUpPlan,
  type PlanIndexerCatchUpOptions,
} from './indexerSync.js';
export {
  mergeInclusiveHeightRanges,
  unionInclusiveHeightRanges,
  subtractInclusiveRangeFromRanges,
  blockHeightGapRowsForInsert,
  nextContiguousIndexedHeightAfterOmittedFetch,
  type InclusiveHeightRange,
  type BlockHeightGapInsertRow,
} from './indexerGaps.js';
export { DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX } from './defaultReferenceFungibleTemplateBytecodeHex.js';
export {
  REFERENCE_FUNGIBLE_TEMPLATE_ARTIFACT_ID,
  REFERENCE_FUNGIBLE_TEMPLATE_VERSION,
  REFERENCE_NFT_COLLECTION_TEMPLATE_ARTIFACT_ID,
  REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION,
  ensure0xHex,
  resolveReferenceFungibleTemplateBytecodeHex,
  resolveReferenceNftCollectionTemplateBytecodeHex,
  buildContractDeployMetaTx,
  type ContractDeployMetaTxObject,
} from './canonicalDeployArtifacts.js';

/**
 * Create a Boing RPC client.
 * @param config - Node URL string (e.g. "http://localhost:8545") or config object (baseUrl, fetch?, timeoutMs?, maxRetries?, …).
 */
export function createClient(config: string | import('./client.js').BoingClientConfig): BoingClient {
  return new BoingClient(config);
}
