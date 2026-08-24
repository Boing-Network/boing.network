# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as applied before **1.0.0** (minor releases may include breaking TypeScript surface changes).

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
