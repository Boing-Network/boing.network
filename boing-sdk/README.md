# Boing SDK

TypeScript/JavaScript client for [Boing Network](https://github.com/Boing-Network/boing.network): typed RPC client, hex utilities, and structured errors (including QA rejection feedback).

## Install

```bash
npm install
npm run build
```

Or from a parent repo (when published): `npm install boing-sdk`.

### Consuming via `file:` (e.g. [boing.finance](https://github.com/Boing-Network/boing.finance))

The compiled output lives in **`dist/`**, which is **committed** in `Boing-Network/boing.network` (repo root `.gitignore` ignores generic `dist/` but not `boing-sdk/dist/`). Clones and CI siblings therefore get a usable `dist/index.js` without a separate SDK build step.

When you change SDK **source** (`src/`), run `npm run build` in `boing-sdk` and commit the updated **`dist/`**. The **`boing-sdk`** workflow fails if `dist/` drifts from the TypeScript build output.

**boing.finance** checks out `boing.network` only so the `file:` path resolves; it does not build the SDK first. **boing.finance**’s `frontend/scripts/postinstall-boing-sdk.mjs` only runs `npm run build` in the linked package when `dist/index.js` is missing (e.g. local SDK development or a broken checkout) and **fails the install** if that build does not succeed.

## Tests

- **Unit / offline:** `npm test` — Vitest; does not require a running Boing node.
- **Optional RPC integration:** `tests/rpcIntegration.test.ts` runs only when **`BOING_INTEGRATION_RPC_URL`** is set (e.g. `http://127.0.0.1:8545`). If it is unset, Vitest **skips** that whole suite so default runs stay offline-friendly. Core cases: sync state (or **`getBlockByHeight`** fallback), blocks + receipts, **`getLogsChunked`**, **`fetchBlocksWithReceiptsForHeightRange`**, **`getTransactionReceipt`** (unknown tx → **`null`**). Endpoints that return **-32601** for **`boing_getSyncState`** or **`boing_getLogs`** still pass where noted. With **`BOING_EXPECT_FULL_RPC=1`** (see **`.github/workflows/boing-sdk-rpc-integration.yml`**), an extra test asserts **`boing_clientVersion`**, **`boing_rpcSupportedMethods`**, and **`probeBoingRpcCapabilities`** (6/6 core methods, including **`boing_getNetworkInfo`** / **`boing_getBlockByHeight`** / **`boing_getTransactionReceipt`**).
- **Operator / tutorial smoke (separate from Vitest):** in **`examples/native-boing-tutorial`**, **`npm run preflight-rpc`** (or **`check-testnet-rpc`** only) with **`BOING_RPC_URL`** — same scripts CI runs after starting a local node; see [PRE-VIBEMINER-NODE-COMMANDS.md](../docs/PRE-VIBEMINER-NODE-COMMANDS.md). From the **monorepo root**, the same **`npm run <script>`** names delegate to the tutorial package (see root **`package.json`**).
- **`npm run verify`** — same as `npm test`, then prints a short note explaining the skip when the env var is missing.

## Quick start

```ts
import { createClient, BoingRpcError } from 'boing-sdk';

const client = createClient('http://localhost:8545');

// Read chain and account state
const height = await client.chainHeight();
const account = await client.getAccount('0x' + '00'.repeat(32)); // 32-byte hex
console.log(account.balance, account.nonce, account.stake);

// Pre-flight QA check before deploying a contract
const qa = await client.qaCheck('0x600160005260206000f3'); // hex bytecode
if (qa.result === 'reject') {
  console.error('QA rejected:', qa.rule_id, qa.message);
} else if (qa.result === 'allow') {
  // Submit signed tx (hex from Rust CLI or future signer)
  await client.submitTransaction(hexSignedTx);
}

// Handle structured QA errors on submit
try {
  await client.submitTransaction(hexSignedTx);
} catch (e) {
  if (e instanceof BoingRpcError && e.isQaRejected) {
    const { rule_id, message } = e.qaData ?? {};
    console.error('Deployment rejected:', rule_id, message);
  }
  throw e;
}
```

## API

- **createClient(config)** — `config` can be a URL string or `{ baseUrl, fetch?, timeoutMs? }`. Default timeout 30s; set `timeoutMs: 0` to disable.
- **BoingClient** — typed methods for all RPCs (32-byte account/hash params are validated locally before sending):
  - **RPC probe** — **`probeBoingRpcCapabilities`** returns **`{ clientVersion, supportedMethods, methods }`** plus **`countAvailableBoingRpcMethods`**, **`explainBoingRpcProbeGaps`** (`rpcCapabilities.ts`): detect **-32601** missing methods; CLI: **`npm run build`** then **`npm run probe-rpc`** (or **`npm run probe-rpc`** from repo root); **`diagnosis`** in JSON explains stale node vs current repo; node discovery: **`boing_clientVersion`**, **`boing_rpcSupportedMethods`**
  - `chainHeight()`, `getBalance(hexAccountId)`, `getAccount(hexAccountId)`
  - `getBlockByHeight(height, includeReceipts?)`, `getBlockByHash(hexHash, includeReceipts?)`
  - `getTransactionReceipt(hexTxId)`, `getLogs(filter)` — bounded log query ([RPC-API-SPEC.md](../docs/RPC-API-SPEC.md))
  - Receipt/log helpers: `normalizeTopicWord`, `iterBlockReceiptLogs`, `filterReceiptLogsByTopic0`, … (`receiptLogs.ts`; see [INDEXER-RECEIPT-AND-LOG-INGESTION.md](../docs/INDEXER-RECEIPT-AND-LOG-INGESTION.md))
  - `getAccountProof(hexAccountId)`, `verifyAccountProof(hexProof, hexStateRoot)`
  - `simulateTransaction(hexSignedTx)`, `submitTransaction(hexSignedTx)`
  - High-level flows: `submitTransferWithSimulationRetry`, `submitContractCallWithSimulationRetry`, `submitDeployWithPurposeFlow` (see **Simulate → access list → submit** below)
  - **Pinned native deploy (Boing Express tx objects)** — `buildContractDeployMetaTx`, **`buildReferenceFungibleDeployMetaTx`** / **`buildReferenceNftCollectionDeployMetaTx`** (wizard-friendly one-call builders), `resolveReferenceFungibleTemplateBytecodeHex`, `resolveReferenceNftCollectionTemplateBytecodeHex`, `ensure0xHex`, `REFERENCE_FUNGIBLE_TEMPLATE_VERSION`, `REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION` (`canonicalDeployArtifacts.ts`; [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](../docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md)); multi-network wizards: **`isBoingTestnetChainId`**, **`normalizeBoingChainIdHex`**, **`BOING_TESTNET_CHAIN_ID_*`** (`chainIds.ts`); review-step QA (`dappDeploy.ts`; [BOING-DAPP-INTEGRATION.md](../docs/BOING-DAPP-INTEGRATION.md) § **One wizard, two backends**): **`buildAndPreflightReferenceFungibleDeploy`**, **`buildAndPreflightReferenceNftCollectionDeployMeta`**, **`preflightContractDeployMetaWithUi`**, **`describeContractDeployMetaQaResponse`**, **`preflightContractDeployMetaQa`**, **`BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX`**
  - **Generic calldata words** — `calldataSelectorLastByte`, `calldataU128BeWord`, `calldataAccountIdWord`, `calldataFixedWord32`, `concatCalldata` (same layout as reference token/NFT encoders; use for custom contracts)
  - **Typed call builder** — `BoingCalldataWord`, `assertBoingCalldataWord`, `boingWordU128` / `boingWordAccount` / `boingWordFixed` / `boingWordSelector` (32-byte argument words), **`encodeBoingCall(selectorLowByte, args)`** (reference-style first word + argument words)
  - **Minimal call ABI (Boing-native)** — `encodeBoingCallTyped(selector, ['u128','account',…], values)`, `encodeBoingCallFromAbiArgs`, **`BoingReferenceCallDescriptors`** (reference token / NFT / native AMM layouts) + **`encodeBoingCallFromDescriptor`** — not Solidity ABI / keccak4; matches `docs/BOING-REFERENCE-TOKEN.md`, `BOING-REFERENCE-NFT.md`, `NATIVE-AMM-CALLDATA.md`
  - **Native CP pool (MVP)** — `buildNativeConstantProductPoolAccessList`, `buildNativeConstantProductContractCallTx`, `mergeNativePoolAccessListWithSimulation`; **`fetchNativeConstantProductReserves`**, **`fetchNativeConstantProductPoolSnapshot`** (batched reserves + total LP ± signer LP), **`fetchNativeConstantProductTotalLpSupply`**, **`fetchNativeConstantProductSwapFeeBps`** (v3/v4), **`fetchNativeAmmSignerLpBalance`**, **`decodeBoingStorageWordU128`**, **`decodeNativeAmmAddLiquidityReturnLpMinted`**, **`decodeNativeAmmLogDataU128Triple`**, reserve / total-LP / token / **swap-fee-bps** key helpers (`nativeAmmPool.ts`); **`tryParseNativeAmmLog2`**, **`filterMapNativeAmmRpcLogs`** (`nativeAmmLogs.ts`); **`constantProductAmountOut`** / **`constantProductAmountOutWithFeeBps`** / **`constantProductAmountOutNoFee`** / **`encodeNativeAmmSetTokensCalldata`** / **`encodeNativeAmmSetSwapFeeBpsCalldata`** / **`NATIVE_CP_SWAP_FEE_BPS`** / **`NATIVE_AMM_TOPIC_*_HEX`** / **`nativeAmmLogTopic0Utf8`** (`nativeAmm.ts`; [NATIVE-AMM-CALLDATA.md](../docs/NATIVE-AMM-CALLDATA.md)); **LP vault** — `encodeNativeAmmLpVaultConfigureCalldata`, `buildNativeAmmLpVaultDepositAddAccessList`, `buildNativeAmmLpVaultDepositAddContractCallTx`, merge helpers (`nativeAmmLpVault.ts`; [NATIVE-AMM-LP-VAULT.md](../docs/NATIVE-AMM-LP-VAULT.md)); **LP share token** — encoders + `buildLpShareTokenAccessList`, `buildLpShareTokenContractCallTx`, `mergeLpShareTokenAccessListWithSimulation` (`nativeLpShareToken.ts`; [NATIVE-LP-SHARE-TOKEN.md](../docs/NATIVE-LP-SHARE-TOKEN.md)); **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** (`canonicalTestnet.ts`) — convenience mirror of [RPC-API-SPEC.md](../docs/RPC-API-SPEC.md) § Native constant-product AMM (bump when the on-chain canonical pool changes)
  - **Native DEX (pair directory + routers)** — [BOING-NATIVE-DEX-CAPABILITY.md](../docs/BOING-NATIVE-DEX-CAPABILITY.md): factory calldata + access lists (`nativeDexFactory.ts`), **`fetchNativeDexFactoryPairsCount`**, **`findNativeDexFactoryPoolByTokens`**, **`Log3`** parsers (`nativeDexFactoryPool.ts`, `nativeDexFactoryLogs.ts`; [NATIVE-DEX-FACTORY.md](../docs/NATIVE-DEX-FACTORY.md)); ledger-router forward calldata + tx builders **v1–v3** (`nativeDexLedgerRouter.ts`; [NATIVE-DEX-LEDGER-ROUTER.md](../docs/NATIVE-DEX-LEDGER-ROUTER.md)); **swap2** + **multihop** encoders (`nativeDexSwap2Router.ts`; [NATIVE-DEX-SWAP2-ROUTER.md](../docs/NATIVE-DEX-SWAP2-ROUTER.md), [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](../docs/NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md)); CREATE2 predictors + salts (`create2.ts`); tutorial **`deploy-native-dex-directory`** ([examples/native-boing-tutorial](../examples/native-boing-tutorial/) README §7c2)
  - **Seamless defaults + wallet glue** — **`fetchNativeDexIntegrationDefaults`** / **`mergeNativeDexIntegrationDefaults`** (`dexIntegration.ts`): merge **`boing_getNetworkInfo`**.**`end_user`** (**`BOING_CANONICAL_NATIVE_CP_POOL`** / **`BOING_CANONICAL_NATIVE_DEX_FACTORY`**) with overrides and **6913** embedded pool fallback; **`fetchNativeDexFactoryRegisterLogs`** for chunked **`register_pair`** scans; **`getInjectedEip1193Provider`**, **`boingSendTransaction`**, **`requestAccounts`**, **`readChainIdHex`**, **`providerSupportsBoingNativeRpc`** (`walletProvider.ts`; [BOING-DAPP-INTEGRATION.md](../docs/BOING-DAPP-INTEGRATION.md))
  - **Directory snapshot (Boing-only)** — **`fetchNativeDexDirectorySnapshot`** (defaults + **`pairs_count`** + optional log backfill), **`pickNativeDexPoolFromRegisterLogs`**, **`resolveNativeDexPoolForTokens`** (`logs` | `simulate` | `auto`), **`nativeDexPairKey`**, **`suggestNativeDexRegisterLogCatchUpRange`** (`nativeDexDirectory.ts`; no external chain RPC)
  - **DEX onboarding + preflight** — **`formatBoingNativeDexNotEvmDisclaimer`**, **`describeNativeDexDefaultGaps`**, **`assertBoingNativeDexToolkitRpc`**, **`formatNativeDexToolkitPreflightForUi`**, **`buildNativeCpPoolSwapExpressTx`** (`nativeDexSeamless.ts`); wallet copy **`BOING_WALLET_RPC_METHODS_NATIVE_DAPP`**, **`explainEthSendTransactionInsufficientForBoingNativeCall`**, **`connectInjectedBoingWallet`**, **`mapInjectedProviderErrorToUiMessage`** (`walletProvider.ts`)
  - **CP routing + aggregation (off-chain)** — **`quoteCpPoolSwap`**, **`rankDirectCpPools`**, **`findBestCpRoutes`**, **`quoteCpEvenSplitAcrossDirectPools`**, **`hydrateCpPoolVenuesFromRpc`**, **`fetchCpRoutingFromDirectoryLogs`** (`nativeDexRouting.ts`; combine with multihop calldata encoders to execute)
  - **CREATE2 address prediction** — **`predictCreate2ContractAddress`**, **`predictNativeCpPoolCreate2Address`** … **`predictNativeCpPoolV5Create2Address`**, **`nativeCpPoolCreate2SaltV1Hex`** … **`nativeCpPoolCreate2SaltV5Hex`**, **`NATIVE_CP_POOL_CREATE2_SALT_V1`** … **`V5`** (`create2.ts`; matches `boing_primitives::create2_contract_address` / `native_amm.rs`; see [NATIVE-AMM-CALLDATA.md](../docs/NATIVE-AMM-CALLDATA.md) § CREATE2)
  - **Indexer-scale reads** — **`getIndexerChainTips`** / **`clampIndexerHeightRange`** / **`planIndexerChainTipsWithFallback`** / **`planIndexerCatchUp`** (`indexerSync.ts`; fallback when **`boing_getSyncState`** is **-32601**); **`fetchBlocksWithReceiptsForHeightRange`** (replay path: full blocks + receipts; demos: [fetch-blocks-range.mjs](../examples/native-boing-tutorial/scripts/fetch-blocks-range.mjs), [indexer-ingest-tick.mjs](../examples/native-boing-tutorial/scripts/indexer-ingest-tick.mjs)); **`summarizeIndexerFetchGaps`**, **`mergeInclusiveHeightRanges`**, **`unionInclusiveHeightRanges`**, **`subtractInclusiveRangeFromRanges`**, **`blockHeightGapRowsForInsert`**, **`nextContiguousIndexedHeightAfterOmittedFetch`** (`indexerBatch.ts`, `indexerGaps.ts`; pruned gaps, D1 row shapes, archive backfill subtraction); `getLogsChunked` / `planLogBlockChunks` (≤128-block spans per [RPC-API-SPEC.md](../docs/RPC-API-SPEC.md)); optional **`maxConcurrent`** on chunked logs and height-range fetches; **`mapWithConcurrencyLimit`**; **`flattenReceiptsFromBundles`**; `fetchReceiptsForBlockHeight`; **`fetchReceiptsForHeightRange`** (optional `onMissingBlock: 'omit'`) — ingestion pseudo-flow: [INDEXER-RECEIPT-AND-LOG-INGESTION.md](../docs/INDEXER-RECEIPT-AND-LOG-INGESTION.md); example DDL: [tools/observer-indexer-schema.sql](../tools/observer-indexer-schema.sql); JSON + SQLite ingest: [examples/observer-ingest-reference](../examples/observer-ingest-reference/) (`ingest-tick`, **`ingest-sqlite-tick`** + **`BOING_SQLITE_PATH`**); D1 cron sample: [examples/observer-d1-worker](../examples/observer-d1-worker/); backlog: [NEXT-STEPS-FUTURE-WORK.md](../docs/NEXT-STEPS-FUTURE-WORK.md)
  - **Fixed contract submitter** — `createNativeContractSubmitter({ client, secretKey32, senderHex, contractHex, accessList? })` → `.submitCalldata(bytes)`
  - `registerDappMetrics(hexContract, hexOwner)`, `submitIntent(hexSignedIntent)`
  - `qaCheck(hexBytecode, purposeCategory?, descriptionHash?, assetName?, assetSymbol?)` — pre-flight QA without submitting (same param order as node `boing_qaCheck`)
  - `qaPoolList()`, `qaPoolConfig()`, `qaPoolVote(txHashHex, voterHex, vote)` — governance QA pool for Unsure deploys
  - `faucetRequest(hexAccountId)` — testnet only
- **BoingRpcError** — `code`, `message`, `data`, `method`; `isQaRejected`, `isQaPendingPool`, `pendingPoolTxHash`, `isQaPoolDisabled`, `isQaPoolFull`, `isQaPoolDeployerCap`, `qaData`; `toString()` for logging.
- **Hex helpers** — `ensureHex`, `bytesToHex`, `hexToBytes`, `accountIdToHex`, `hexToAccountId`, `validateHex32`, **`isBoingNativeAccountIdHex`** (32-byte account id check for multi-wallet wizards).

All 32-byte IDs (account, hash) are hex strings with or without `0x` prefix. Invalid hex or wrong length throws before the request.

## Submitting transactions

The node expects **hex-encoded bincode-serialized `SignedTransaction`**. Encoding matches Rust `boing-primitives` (bincode 1.3); see [BOING-SIGNED-TRANSACTION-ENCODING.md](../docs/BOING-SIGNED-TRANSACTION-ENCODING.md).

**Options:**

1. **TypeScript (Node or bundler)** — build + sign with a 32-byte Ed25519 secret key:

```ts
import {
  createClient,
  fetchNextNonce,
  buildTransferTransaction,
  signTransactionInput,
} from 'boing-sdk';

const client = createClient('http://localhost:8545');
const senderHex = '0x' + '<64-hex of public key>';
const secret32 = new Uint8Array(32); // your signing seed / secret key bytes
const nonce = await fetchNextNonce(client, senderHex);
const tx = buildTransferTransaction({
  nonce,
  senderHex,
  toHex: '0x' + '<64-hex recipient>',
  amount: 1n,
});
const signedHex = await signTransactionInput(tx, secret32);
await client.simulateTransaction(signedHex);
await client.submitTransaction(signedHex);
```

2. **Injected wallet** — `boing_sendTransaction` / `boing_signTransaction` in **Boing Express** ([BOING-EXPRESS-WALLET.md](../docs/BOING-EXPRESS-WALLET.md)).

3. **Rust CLI** — `cargo run -p boing-cli -- …` for local dev.

4. **Custom signer** — `signTransactionInputWithSigner(tx, async (hash) => …)` (must return 64-byte Ed25519 signature over `hash`).

**Protocol QA:** Contract deploys are checked in the mempool (`boing_qa`). Use `qaCheck` before submit and purpose-bearing deploy types from the wallet. See [QUALITY-ASSURANCE-NETWORK.md](../docs/QUALITY-ASSURANCE-NETWORK.md).

**dApp checklist:** [BOING-DAPP-INTEGRATION.md](../docs/BOING-DAPP-INTEGRATION.md).

See [RPC-API-SPEC.md](../docs/RPC-API-SPEC.md), [BUILD-ROADMAP.md](../docs/BUILD-ROADMAP.md), and [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](../docs/BOING-VM-CAPABILITY-PARITY-ROADMAP.md).

## Simulate → access list → submit (P4)

Use **`submitTransferWithSimulationRetry`**, **`submitContractCallWithSimulationRetry`**, or **`submitDeployWithPurposeFlow`** so the node can widen `access_list` when `boing_simulateTransaction` returns `access_list_covers_suggestion: false` (merges `suggested_access_list` via `mergeAccessListWithSimulation`).

```ts
import {
  createClient,
  submitTransferWithSimulationRetry,
  explainBoingRpcError,
  senderHexFromSecretKey,
  hexToBytes,
} from 'boing-sdk';

const client = createClient(process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545');
const secret = hexToBytes(process.env.BOING_SECRET_HEX!); // 0x + 64 hex
const senderHex = await senderHexFromSecretKey(secret);

try {
  const { tx_hash, lastSimulation, attempts } = await submitTransferWithSimulationRetry({
    client,
    secretKey32: secret,
    senderHex,
    toHex: process.env.BOING_TO_HEX!,
    amount: 10n,
  });
  console.log({ tx_hash, gas: lastSimulation.gas_used, attempts });
} catch (e) {
  console.error(explainBoingRpcError(e));
}
```

Deploy path runs **`boing_qaCheck`** first; **`reject`** throws `BoingRpcError` with `isQaRejected`. On **`submitTransaction`**, catch **`BoingRpcError`** for mempool QA (**-32050**), pool (**-32051**), and pool caps (**-32054..-32056**). Code reference: [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](../docs/BOING-RPC-ERROR-CODES-FOR-DAPPS.md).

**Canonical scripts:** [examples/native-boing-tutorial](../examples/native-boing-tutorial/) (`transfer`, `contract-call`, `deploy-minimal`).

## API additions (transaction flows)

- `senderHexFromSecretKey(secret32)` — derive `AccountId` hex from signing seed.
- `submitTransferWithSimulationRetry`, `submitContractCallWithSimulationRetry`, `submitDeployWithPurposeFlow` — see `submitFlow.ts`.
- `SimulationFailedError` — simulation failed without an access-list retry path.
- `explainBoingRpcError(e)` — human-readable string for `BoingRpcError` and QA pool codes.

## Tests

```bash
cd boing-sdk && npm install && npm test
```

Golden vectors are tied to `cargo run -p boing-primitives --example dump_bincode`.

## Planned

- CLI auto-completion, contract templates. See [DEVELOPMENT-AND-ENHANCEMENTS.md](../docs/DEVELOPMENT-AND-ENHANCEMENTS.md).
