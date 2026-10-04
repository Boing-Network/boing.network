# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as applied before **1.0.0** (minor releases may include breaking TypeScript surface changes).

## [0.4.0] - 2026-10-04

### Added

- **Reference NFT `mint_batch` encoders** — `SELECTOR_MINT_BATCH` (`0x06`), `MAX_REFERENCE_NFT_MINT_BATCH` (50), `encodeReferenceMintBatchCalldata` / `encodeReferenceMintBatchCalldataHex` (layout `96 + 64n`; all-zero metadata hashes skip on-chain `SSTORE`). FreshMint and other clients can drop local encoders.
- **Reference NFT collection template v2** — `REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION` is `"2"`; default embed / resolvers ship the v2 deploy bytecode (artifact id still `boing.reference_nft_collection.v0`). Existing on-chain v1 collections stay immutable without `mint_batch`.

### Notes

- Large `mint_batch` calls need nodes with `GAS_PER_CONTRACT_CALL = 3_000_000` (measured n=50 owner+metadata ≈ 2.15M gas). Public Fly testnet redeploy may still be pending.

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
