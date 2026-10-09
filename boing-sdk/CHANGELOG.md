# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as applied before **1.0.0** (minor releases may include breaking TypeScript surface changes).

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
