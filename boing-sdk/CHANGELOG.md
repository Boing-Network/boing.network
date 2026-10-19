# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as applied before **1.0.0** (minor releases may include breaking TypeScript surface changes).

## [Unreleased]

- **Linked NFT↔token registry v2** (creator-only claims, `docs/BOING-LINKED-NFT-TOKEN.md`): `CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX` now points to the v2 registry `0xf704485a…55dd` (v1 kept as `CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX`). New: `claim_self` / `get_asset_deployer` selectors and encoders, `buildLinkedNftTokenClaimSelfInitSnippetHex` (asset init code claims itself for the deploy tx sender), claimer / deployer storage keys, `LINKED_NFT_TOKEN_TOPIC_CLAIM_V2_HEX`, `LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX`, `DEFAULT_LINKED_NFT_TOKEN_REGISTRY_V2_RUNTIME_BYTECODE_HEX`, `buildLinkedNftTokenRegistryV2DeployMetaTx`; `buildLinkedNftTokenRegisterFlowTxs({ includeClaims: false })` and `buildLinkedNftTokenProjectPack({ claimAtDeploy: true })` (`LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER_CLAIM_AT_DEPLOY`). On v2 a `claim_asset` by anyone other than the recorded deployer fails.
- **Native value on contract calls** (testnet height 1150, `docs/NATIVE-VALUE-AND-WBOING.md`): `contractCallWithValue` payload (bincode variant 20) and batch step (`BatchCall` 2), `buildContractCallWithValueTransaction`, value steps in `buildBatchTransaction`, `BOING_TESTNET_NATIVE_VALUE_FROM`, `nativeValueActiveAt`.
- **wBOING** (`nativeValue.ts`): `WRAPPED_BOING_BYTECODE_HEX`, `encodeWboing*Calldata`, `buildWrapBoingTransaction`, `buildUnwrapBoingTransaction`, topics and supply key.
- **Pool v10** (u128 amounts for 18-decimal tokens): `NATIVE_CP_POOL_BYTECODE_HEX[10]`, `boingPoolU128Amounts`, quotes above u64. `planBoingNativeSwap` builds one atomic batch (wrap + approve + swap, or approve + swap + withdraw_all); `boingTxToExpressRequest` converts it for Boing Express.
- **Native BOING 18 decimals** (`nativeAmount.ts`): `BOING_DECIMALS`, `BASE_UNITS_PER_BOING`, `GAS_PRICE_BASE_UNITS`, bigint `formatBoing` / `parseBoing`, `formatUnits` / `parseUnits`, height-aware `nativeDecimalsAt` (testnet switches at 1050), `formatNativeAmount`, `boingToLedgerUnits`, `feeForGasBaseUnits`. Types: `native_currency` denomination fields, `*_boing` / `decimals` on `AccountBalance` / `AccountState`, `network_fee` on `SimulateResult`. See `docs/NATIVE-DENOMINATION.md`.
- **Breaking defaults:** `deployBoingToken` / `boingTokenBytecodeHex` now default to reference fungible **v2**, and the pool helpers default to pool **v8**. Pass `version: 1` / `version: 7` for the old behaviour.
- Reference fungible v2: `approveBoingToken`, `getBoingTokenAllowance`, `buildBoingTokenApproveTransaction`, `boingTokenAllowanceStorageKeyHex`, `encodeReference{Approve,TransferFrom,Allowance}Calldata`, `DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_V2_BYTECODE_HEX`.
- Pool v8 (token-pulling custody): `boingPoolPullsTokens`, `encodeBoingPoolV8*Calldata`, `BoingPoolV8Options` (`tokenAHex`, `tokenBHex`, `noApprove`, `deadlineHeight`); add/swap send exact-amount approvals first and return them in `approvals`. Byte-identical to `boing pool` / `boing token` (`tests/fixtures/pool-cli-vectors.json`).
- Native AMM pool helpers mirroring the new `boing pool` CLI: `createBoingPool`, `addBoingPoolLiquidity`, `removeBoingPoolLiquidity`, `swapBoingPool` (slippage `minOut`), `quoteBoingPoolSwap`, `getBoingPoolState`, `getBoingPoolLpBalance`, unsigned builders, pure quote/LP math and `NATIVE_CP_POOL_BYTECODE_HEX` (v1..v7); byte-identical to the CLI (`tests/fixtures/pool-cli-vectors.json`). See `docs/NATIVE-AMM-CLI.md`.
- Token helpers mirroring `boing token`: `deployBoingToken`, `mintBoingToken`, `transferBoingToken`, `getBoingTokenBalance`, `boingTokenBytecodeHex` and unsigned builders; byte-identical to the CLI (cross-checked against CLI `--dry-run` vectors).

## [0.5.3] - 2026-10-13

### Added

- **Builder attestation (on-chain registry)** — bytecode + selectors `0xE0`–`0xE6` (dual asset-claimer separate from companions; revoke = recorded builder **or** dual claimer); SDK **`builderAttestationRegistry.ts`** (deploy / claim / attest / revoke / query), CREATE2 salt `BOING_BUILDER_ATTEST_REG_V1`, **`CANONICAL_BOING_TESTNET_BUILDER_ATTESTATION_REGISTRY_HEX`** (`0x734077a9…1816`), **`buildBuilderAttestationAttestFlowTxs`**. Docs: [BOING-BUILDER-ATTESTATION.md](../docs/BOING-BUILDER-ATTESTATION.md). Shipped via [PR #49](https://github.com/Boing-Network/boing.network/pull/49).

## [0.5.2] - 2026-10-10

### Added

- **`buildLinkedNftTokenProjectPack`** — thin dApp helper composing **`buildLinkedNftTokenPairDeploys`** + **`buildLinkedNftTokenRegisterFlowTxs`** (joint NFT+token CREATE2 deploys, then claim×2 + `register_link`). Defaults registry to canonical public testnet. Docs: [BOING-LINKED-NFT-TOKEN.md](../docs/BOING-LINKED-NFT-TOKEN.md).

## [0.5.1] - 2026-10-09

### Added

- **Linked NFT ↔ fungible token (enforced on-chain)** — registry bytecode + selectors `0xE0`–`0xE6` (dual asset-claimer auth, many-to-many, mutable unlink); SDK **`linkedNftTokenRegistry.ts`** (deploy / claim / register / unlink / query), CREATE2 salt predictor, **`buildLinkedNftTokenRegisterFlowTxs`**. Optional schema **`boing.linked_nft_token.v1`** + off-chain keys remain as **cache only** (`linkedNftToken.ts`). Docs: [BOING-LINKED-NFT-TOKEN.md](../docs/BOING-LINKED-NFT-TOKEN.md).
- **`ContractDeployMetaTxObject.create2_salt`** / **`buildContractDeployMetaTx({ create2SaltHex })`** for Express CREATE2 deploys.

## [0.5.0] - 2026-10-05

### Changed

- **`MAX_REFERENCE_NFT_MINT_BATCH`** raised **50 → 500** (template **v3** bytecode).
- **`REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`** is **`"3"`**; default embed ships v3 hex (artifact id still `boing.reference_nft_collection.v0`). Existing on-chain v2 collections keep n≤50 forever.

### Notes

- Full n=500 owner+metadata ≈ **33.3M** gas → nodes need **`GAS_PER_CONTRACT_CALL = 40_000_000`**. Fee ≈ **1587 BOING**. For 10k tokens: **20 × 500** txs (10k in one call is not practical — see [BOING-REFERENCE-NFT.md](../docs/BOING-REFERENCE-NFT.md)).
- FreshMint / Vercel: set new v3 hex env (prefer `BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_V3_BYTECODE_HEX` or the shared resolver env) after Fly nodes redeploy with 40M gas.

## [0.4.0] - 2026-10-04

### Added

- **Reference NFT `mint_batch` encoders** — `SELECTOR_MINT_BATCH` (`0x06`), `MAX_REFERENCE_NFT_MINT_BATCH` (50), `encodeReferenceMintBatchCalldata` / `encodeReferenceMintBatchCalldataHex` (layout `96 + 64n`; all-zero metadata hashes skip on-chain `SSTORE`). FreshMint and other clients can drop local encoders.
- **Reference NFT collection template v2** — `REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION` is `"2"`; default embed / resolvers ship the v2 deploy bytecode (artifact id still `boing.reference_nft_collection.v0`). Existing on-chain v1 collections stay immutable without `mint_batch`.

### Notes

- Large `mint_batch` calls need nodes with `GAS_PER_CONTRACT_CALL = 3_000_000` (measured n=50 owner+metadata ≈ 2.15M gas). Superseded by **0.5.0** / template v3 (40M / n=500).

## [0.3.1] - 2026-04-12

### Added

- **Universal deploy index helpers** — parse **`boing_getBlockByHeight`** transaction JSON, derive **`Transaction::id`** (`transactionIdFromUnsignedRpcTransaction`), and extract predicted contract addresses (`extractUniversalContractDeploymentsFromBlock` / `FromBlockJson`). See **`docs/HANDOFF_Universal_Contract_Deploy_Indexer.md`** and **`workers/deploy-registry-indexer`**.
- **`QaPoolItemSummary`** optional **`purpose_category`**, **`asset_name`**, **`asset_symbol`** from `boing_qaPoolList` (asset review fields for Unsure deploys).

## [0.3.0] - 2026-04-12

### Added

- **DEX discovery JSON-RPC helpers** on `BoingClient`: `listDexPoolsPage`, `listDexTokensPage`, `getDexToken` (`boing_listDexPools`, `boing_listDexTokens`, `boing_getDexToken`), including optional `factory`, `light` / `enrich`, and `includeDiagnostics`.
- **Types** `DexPoolListRow`, `DexPoolListPage`, `DexTokenListRow`, `DexTokenListPage`, `DexDiscoveryPoolDiagnostics`, `DexDiscoveryTokenDiagnostics` (exported from package root).
- **`buildNativeDexIndexerStatsForClient`**: merges `createdAtHeight`, `tokenADecimals`, and `tokenBDecimals` from `boing_listDexPools` into `NativeDexIndexerPoolRow` when the node supports discovery RPC.

### Changed

- **`DexPoolListRow`** now includes required **`tokenADecimals`** and **`tokenBDecimals`** (aligned with node; default **18** when the node has no decimals map).

### Breaking (TypeScript)

- Code that **constructs** `DexPoolListRow` literals must supply **`tokenADecimals`** and **`tokenBDecimals`**. Consumers that only **parse** RPC JSON from a current node are unchanged.

### Packaging

- **`package.json` `files`**: the published tarball includes **`dist/`**, **`README.md`**, and **`CHANGELOG.md`** only (smaller install; `prepublishOnly` still runs **`npm run build`**).
