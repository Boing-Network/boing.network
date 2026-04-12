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
export {
  CANONICAL_BOING_TESTNET_NATIVE_DEX_DEPLOYER_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX,
} from './canonicalTestnetDex.js';

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
  isBoingNativeAccountIdHex,
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
  SELECTOR_NATIVE_AMM_SWAP_TO,
  SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO,
  NATIVE_CP_SWAP_FEE_BPS,
  NATIVE_AMM_TOPIC_SWAP_HEX,
  NATIVE_AMM_TOPIC_ADD_LIQUIDITY_HEX,
  NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY_HEX,
  nativeAmmLogTopic0Utf8,
  encodeNativeAmmSwapCalldata,
  encodeNativeAmmSwapToCalldata,
  encodeNativeAmmAddLiquidityCalldata,
  encodeNativeAmmRemoveLiquidityCalldata,
  encodeNativeAmmRemoveLiquidityToCalldata,
  encodeNativeAmmSetTokensCalldata,
  encodeNativeAmmSetSwapFeeBpsCalldata,
  encodeNativeAmmSwapCalldataHex,
  encodeNativeAmmSwapToCalldataHex,
  encodeNativeAmmAddLiquidityCalldataHex,
  encodeNativeAmmRemoveLiquidityCalldataHex,
  encodeNativeAmmRemoveLiquidityToCalldataHex,
  encodeNativeAmmSetTokensCalldataHex,
  encodeNativeAmmSetSwapFeeBpsCalldataHex,
  constantProductAmountOut,
  constantProductAmountOutNoFee,
  constantProductAmountOutWithFeeBps,
} from './nativeAmm.js';
export {
  SELECTOR_NATIVE_DEX_REGISTER_PAIR,
  SELECTOR_NATIVE_DEX_PAIRS_COUNT,
  SELECTOR_NATIVE_DEX_GET_PAIR_AT,
  NATIVE_DEX_FACTORY_MAX_PAIRS,
  NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX,
  NATIVE_DEX_FACTORY_CREATE2_SALT_V1_HEX,
  encodeNativeDexRegisterPairCalldata,
  encodeNativeDexRegisterPairCalldataHex,
  encodeNativeDexPairsCountCalldata,
  encodeNativeDexPairsCountCalldataHex,
  encodeNativeDexGetPairAtCalldata,
  encodeNativeDexGetPairAtCalldataHex,
} from './nativeDexFactory.js';
export {
  NATIVE_DEX_FACTORY_COUNT_KEY_HEX,
  buildNativeDexFactoryAccessList,
  buildNativeDexFactoryContractCallTx,
  mergeNativeDexFactoryAccessListWithSimulation,
  decodeNativeDexFactoryPairsCountReturnData,
  decodeNativeDexFactoryGetPairAtReturnData,
  fetchNativeDexFactoryPairsCount,
  findNativeDexFactoryPoolByTokens,
  type FindNativeDexFactoryPoolOptions,
} from './nativeDexFactoryPool.js';
export {
  type NativeDexFactoryRegisterEvent,
  type NativeDexFactoryRegisterRpcParsed,
  isNativeDexFactoryRegisterTopic0,
  isNativeDexFactoryRegisterLog3Shape,
  tryParseNativeDexFactoryRegisterLog3,
  tryParseNativeDexFactoryRegisterRpcLogEntry,
  collectNativeDexFactoryRegisterLogsFromReceipt,
} from './nativeDexFactoryLogs.js';
export {
  SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD,
  SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V2,
  SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V3,
  encodeNativeDexLedgerRouterForwardCalldata,
  encodeNativeDexLedgerRouterForwardCalldataHex,
  encodeNativeDexLedgerRouterForwardCalldataV2,
  encodeNativeDexLedgerRouterForwardCalldataV2Hex,
  encodeNativeDexLedgerRouterForwardCalldataV3,
  encodeNativeDexLedgerRouterForwardCalldataV3Hex,
  buildNativeDexLedgerRouterAccessList,
  buildNativeDexLedgerRouterContractCallTx,
  buildNativeDexLedgerRouterV2ContractCallTx,
  buildNativeDexLedgerRouterV3ContractCallTx,
  mergeNativeDexLedgerRouterAccessListWithSimulation,
} from './nativeDexLedgerRouter.js';
export {
  SELECTOR_NATIVE_DEX_SWAP2_ROUTER_128,
  SELECTOR_NATIVE_DEX_SWAP2_ROUTER_160,
  SELECTOR_NATIVE_DEX_SWAP3_ROUTER_128,
  SELECTOR_NATIVE_DEX_SWAP3_ROUTER_160,
  SELECTOR_NATIVE_DEX_SWAP4_ROUTER_128,
  SELECTOR_NATIVE_DEX_SWAP4_ROUTER_160,
  encodeNativeDexSwap2RouterCalldata128,
  encodeNativeDexSwap2RouterCalldata128Hex,
  encodeNativeDexSwap2RouterCalldata160,
  encodeNativeDexSwap2RouterCalldata160Hex,
  encodeNativeDexSwap3RouterCalldata128,
  encodeNativeDexSwap3RouterCalldata128Hex,
  encodeNativeDexSwap3RouterCalldata160,
  encodeNativeDexSwap3RouterCalldata160Hex,
  encodeNativeDexSwap4RouterCalldata128,
  encodeNativeDexSwap4RouterCalldata128Hex,
  encodeNativeDexSwap4RouterCalldata160,
  encodeNativeDexSwap4RouterCalldata160Hex,
} from './nativeDexSwap2Router.js';
export {
  SELECTOR_LP_SHARE_TRANSFER,
  SELECTOR_LP_SHARE_MINT,
  SELECTOR_LP_SHARE_SET_MINTER_ONCE,
  LP_SHARE_MINTER_KEY_U8,
  buildLpShareTokenAccessList,
  buildLpShareTokenContractCallTx,
  encodeLpShareTransferCalldata,
  encodeLpShareTransferCalldataHex,
  encodeLpShareMintCalldata,
  encodeLpShareMintCalldataHex,
  encodeLpShareSetMinterOnceCalldata,
  encodeLpShareSetMinterOnceCalldataHex,
  mergeLpShareTokenAccessListWithSimulation,
  lpShareTokenBalanceStorageKeyHex,
  fetchLpShareTokenBalanceRaw,
} from './nativeLpShareToken.js';
export {
  SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE,
  SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD,
  buildNativeAmmLpVaultConfigureAccessList,
  buildNativeAmmLpVaultConfigureContractCallTx,
  buildNativeAmmLpVaultDepositAddAccessList,
  buildNativeAmmLpVaultDepositAddContractCallTx,
  encodeNativeAmmLpVaultConfigureCalldata,
  encodeNativeAmmLpVaultConfigureCalldataHex,
  encodeNativeAmmLpVaultDepositAddCalldata,
  encodeNativeAmmLpVaultDepositAddCalldataHex,
  mergeNativeAmmLpVaultConfigureAccessListWithSimulation,
  mergeNativeAmmLpVaultDepositAddAccessListWithSimulation,
} from './nativeAmmLpVault.js';
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
  decodeNativeAmmAddLiquidityReturnLpMinted,
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
  type NativeAmmVaultPoolResolution,
  type NativeDexLpVaultSharePositionForOwner,
  resolveNativeAmmVaultPoolMapping,
  resolveNativeAmmVaultPoolMappings,
  fetchNativeDexLpVaultSharePositionForOwner,
} from './nativeDexLpPositions.js';
export {
  type NativeDexMaterializedPoolEvent,
  type CollectNativeDexPoolEventsOptions,
  materializeNativeAmmPoolEvent,
  collectNativeDexPoolEventsForPools,
  hydrateNativeDexPoolEventsWithBlockHashes,
} from './nativeDexPoolHistory.js';
export {
  ERC721_TRANSFER_TOPIC0_HEX,
  tryParseErc721TransferRpcLog,
  filterMapErc721TransferRpcLogs,
  type Erc721TransferParsed,
} from './erc721Logs.js';
export {
  type NativeDexIndexedNftOwnerRow,
  type CollectNftOwnersFromErc721TransfersOptions,
  collectNftOwnersFromErc721Transfers,
} from './nativeDexNftIndexer.js';
export {
  type NativeDexArchivedReceiptLogRow,
  type CollectArchivedReceiptLogRowsOptions,
  collectArchivedReceiptLogRows,
} from './nativeDexReceiptArchive.js';
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
  transactionIdFromSignedTransactionHex,
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
  predictNonceDerivedContractAddress,
  predictNativeCpPoolCreate2Address,
  predictNativeCpPoolV2Create2Address,
  predictNativeCpPoolV3Create2Address,
  predictNativeCpPoolV4Create2Address,
  predictNativeCpPoolV5Create2Address,
  predictNativeDexFactoryCreate2Address,
  predictNativeDexLedgerRouterCreate2Address,
  predictNativeDexLedgerRouterV2Create2Address,
  predictNativeDexLedgerRouterV3Create2Address,
  predictNativeDexMultihopSwapRouterCreate2Address,
  predictNativeDexSwap2RouterCreate2Address,
  predictNativeLpShareTokenCreate2Address,
  predictNativeAmmLpVaultCreate2Address,
  nativeCpPoolCreate2SaltV1Hex,
  nativeCpPoolCreate2SaltV2Hex,
  nativeCpPoolCreate2SaltV3Hex,
  nativeCpPoolCreate2SaltV4Hex,
  nativeCpPoolCreate2SaltV5Hex,
  nativeDexFactoryCreate2SaltV1Hex,
  nativeDexLedgerRouterCreate2SaltV1Hex,
  nativeDexLedgerRouterCreate2SaltV2Hex,
  nativeDexLedgerRouterCreate2SaltV3Hex,
  nativeDexMultihopSwapRouterCreate2SaltV1Hex,
  nativeDexSwap2RouterCreate2SaltV1Hex,
  nativeLpShareTokenCreate2SaltV1Hex,
  nativeAmmLpVaultCreate2SaltV1Hex,
  NATIVE_CP_POOL_CREATE2_SALT_V1,
  NATIVE_CP_POOL_CREATE2_SALT_V2,
  NATIVE_CP_POOL_CREATE2_SALT_V3,
  NATIVE_CP_POOL_CREATE2_SALT_V4,
  NATIVE_CP_POOL_CREATE2_SALT_V5,
  NATIVE_DEX_FACTORY_CREATE2_SALT_V1,
  NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1,
  NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2,
  NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3,
  NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1,
  NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1,
  NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1,
  NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1,
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
export { DEFAULT_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX } from './defaultReferenceFungibleSecuredTemplateBytecodeHex.js';
export { DEFAULT_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX } from './defaultReferenceNftCollectionTemplateBytecodeHex.js';
export {
  REFERENCE_FUNGIBLE_TEMPLATE_ARTIFACT_ID,
  REFERENCE_FUNGIBLE_TEMPLATE_VERSION,
  REFERENCE_FUNGIBLE_SECURED_TEMPLATE_ARTIFACT_ID,
  REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION,
  REFERENCE_NFT_COLLECTION_TEMPLATE_ARTIFACT_ID,
  REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION,
  ensure0xHex,
  resolveReferenceFungibleTemplateBytecodeHex,
  resolveReferenceFungibleSecuredTemplateBytecodeHex,
  resolveReferenceNftCollectionTemplateBytecodeHex,
  buildContractDeployMetaTx,
  buildReferenceFungibleDeployMetaTx,
  buildReferenceFungibleSecuredDeployMetaTx,
  buildReferenceNftCollectionDeployMetaTx,
  resolveNativeConstantProductPoolBytecodeHex,
  buildNativeConstantProductPoolDeployMetaTx,
  type ContractDeployMetaTxObject,
  type BuildReferenceFungibleDeployMetaTxInput,
  type BuildReferenceFungibleSecuredDeployMetaTxInput,
  type BuildReferenceNftCollectionDeployMetaTxInput,
  type BuildNativeConstantProductPoolDeployMetaTxInput,
} from './canonicalDeployArtifacts.js';
export {
  normalizeNativeTokenSecurity,
  descriptionHashHexFromNativeTokenSecurity,
  type NativeTokenSecurityFeaturesInput,
  type NativeTokenSecurityFeaturesNormalized,
} from './nativeTokenSecurity.js';
export {
  FLAG_ANTI_BOT,
  FLAG_COOLDOWN,
  FLAG_DENYLIST,
  FLAG_MAX_TX,
  FLAG_MAX_WALLET,
  FLAG_NO_MINT,
  FLAG_TRANSFER_UNLOCK,
  buildReferenceFungibleSecuredDeployBytecodeHexFromNativeTokenSecurity,
  referenceFungibleSecuredConfigFromNativeTokenSecurity,
  referenceFungibleSecuredDeployBytecodeHex,
  type ReferenceFungibleSecuredConfigBytes,
  type SecuredWizardBuildContext,
} from './referenceFungibleSecuredDeployBytecode.js';
export {
  BOING_TESTNET_CHAIN_ID_DECIMAL,
  BOING_TESTNET_CHAIN_ID_HEX,
  normalizeBoingChainIdHex,
  isBoingTestnetChainId,
} from './chainIds.js';
export {
  BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX,
  preflightContractDeployMetaQa,
  describeContractDeployMetaQaResponse,
  preflightContractDeployMetaWithUi,
  buildAndPreflightReferenceFungibleDeploy,
  buildAndPreflightReferenceNftCollectionDeployMeta,
  buildAndPreflightNativeConstantProductPoolDeploy,
  buildAndPreflightBoingIntegrationDeploy,
  type BoingDeployIntegrationKind,
  defaultPurposeCategoryForBoingDeployKind,
  buildBoingIntegrationDeployMetaTx,
  preflightBoingIntegrationDeploy,
  type DeployMetaQaUiOutcome,
  type PreflightContractDeployMetaWithUiResult,
  type BuildAndPreflightReferenceFungibleResult,
  type BuildAndPreflightReferenceNftCollectionResult,
  type BoingIntegrationDeployInput,
  type PreflightBoingIntegrationDeployResult,
  type BuildAndPreflightNativeConstantProductPoolResult,
} from './dappDeploy.js';
export {
  mergeNativeDexIntegrationDefaults,
  fetchNativeDexIntegrationDefaults,
  fetchNativeDexFactoryRegisterLogs,
  buildNativeDexIntegrationOverridesFromProcessEnv,
  type NativeDexIntegrationDefaults,
  type NativeDexIntegrationOverrides,
  type NativeDexDefaultSource,
} from './dexIntegration.js';
export {
  nativeDexPairKey,
  buildNativeDexRegisterLogPoolIndex,
  pickNativeDexPoolFromRegisterLogs,
  fetchNativeDexDirectorySnapshot,
  resolveNativeDexPoolForTokens,
  suggestNativeDexRegisterLogCatchUpRange,
  type NativeDexDirectorySnapshot,
  type FetchNativeDexDirectorySnapshotOptions,
  type ResolveNativeDexPoolForTokensResult,
  type ResolveNativeDexPoolForTokensOptions,
} from './nativeDexDirectory.js';
export {
  BOING_NATIVE_DEX_NOT_EVM_TAGLINE,
  BOING_NATIVE_DEX_NOT_EVM_BULLETS,
  BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS,
  formatBoingNativeDexNotEvmDisclaimer,
  describeNativeDexDefaultGaps,
  assertBoingNativeDexToolkitRpc,
  formatNativeDexToolkitPreflightForUi,
  buildNativeCpPoolSwapExpressTx,
} from './nativeDexSeamless.js';
export {
  quoteCpPoolSwap,
  rankDirectCpPools,
  findBestCpRoutes,
  findBestCpRoute,
  quoteCpEvenSplitAcrossDirectPools,
  hydrateCpPoolVenuesFromRpc,
  fetchCpRoutingFromDirectoryLogs,
  type CpPoolVenue,
  type CpQuoteResult,
  type RouteHop,
  type CpSwapRoute,
  type PoolTokenRow,
  type FetchCpRoutingFromDirectoryLogsOptions,
} from './nativeDexRouting.js';
export {
  type NativeDexIndexerHistoryPoint,
  type NativeDexIndexerPersistedDoc,
  type NativeDexIndexerHistoryStore,
  type NativeDexIndexerTokenMeta,
  type NativeDexIndexerStatsOptions,
  type NativeDexIndexerPoolRow,
  type NativeDexIndexerStatsPayload,
  parseNativeDexIndexerPersistedDoc,
  appendVenuesToHistoryDoc,
  buildDexOverridesFromPlainEnv,
  buildNativeDexIndexerStatsForClient,
} from './nativeDexIndexerStats.js';
export {
  NATIVE_DEX_DIRECTORY_API_ID,
  NATIVE_DEX_DIRECTORY_SCHEMA_VERSION,
  NativeDexDirectoryHttpError,
  normalizeNativeDexDirectoryWorkerBaseUrl,
  parseNativeDexDirectoryMetaResponse,
  parseNativeDexDirectoryPoolsPageResponse,
  parseNativeDexDirectoryPoolEventsPageResponse,
  parseNativeDexDirectoryUserEventsPageResponse,
  fetchNativeDexDirectoryMeta,
  fetchNativeDexDirectoryPoolsPage,
  fetchNativeDexDirectoryPoolEventsPage,
  fetchNativeDexDirectoryUserEventsPage,
  collectAllNativeDexDirectoryPools,
  type NativeDexDirectoryMetaResponse,
  type NativeDexDirectoryPoolsPageResponse,
  type NativeDexDirectoryPoolEventsPageResponse,
  type NativeDexDirectoryUserEventsPageResponse,
  type FetchNativeDexDirectoryPoolsPageQuery,
  type FetchNativeDexDirectoryPoolEventsPageQuery,
  type FetchNativeDexDirectoryUserEventsPageQuery,
  type CollectNativeDexDirectoryPoolsOptions,
} from './nativeDexDirectoryApi.js';
export {
  getInjectedEip1193Provider,
  providerSupportsBoingNativeRpc,
  boingSendTransaction,
  requestAccounts,
  readChainIdHex,
  connectInjectedBoingWallet,
  mapInjectedProviderErrorToUiMessage,
  BOING_WALLET_RPC_METHODS_NATIVE_DAPP,
  explainEthSendTransactionInsufficientForBoingNativeCall,
  type Eip1193Requester,
  type BoingInjectedWalletConnectResult,
} from './walletProvider.js';

/**
 * Create a Boing RPC client.
 * @param config - Node URL string (e.g. "http://localhost:8545") or config object (baseUrl, fetch?, timeoutMs?, maxRetries?, …).
 */
export function createClient(config: string | import('./client.js').BoingClientConfig): BoingClient {
  return new BoingClient(config);
}
