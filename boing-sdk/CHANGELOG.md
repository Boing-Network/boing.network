# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as applied before **1.0.0** (minor releases may include breaking TypeScript surface changes).

## [Unreleased]

### Added

- **Optional priority tips** — `TransactionInput.priorityTip`, `withPriorityTip`, `signableTransactionHashWithTip` / `TX_SIGNING_DOMAIN_PRIORITY`. Zero tip keeps historical encode/sign; non-zero tips bind into the signature and wire encoding.
- **Micro-fee helpers** — `MICRO_FEE_EXECUTION_GAS_BPS` (100 → ~0.00021 BOING/transfer), `estimateExecutionGasFee`, `estimateMicroTransferFee`, `suggestPriorityTipLadder`, `estimateTotalFeeWithTip`, `formatBoingFromBaseUnits`, `formatTipLadderForUi`. Product target is a tiny base fee + optional tip (not free-by-default). See [OPTIONAL-PRIORITY-FEES.md](../docs/OPTIONAL-PRIORITY-FEES.md).
- **Tip split knobs** (node / policy JSON) — `tip_proposer_bps` / `tip_treasury_bps` / `tip_burn_bps` (default 100% proposer).

## [0.5.22] - 2026-10-22

### Added

- **L1 media blobs Phase 19** — `phase19DasSuccinctProofPlan`; `PHASE19_*` aliases; `phase19CommitteeRequiresAttested(pinnedLen)`; committee nets (`|pinned slash_appeal_resolvers| > 1`) **must** use `ResolveSlashAppealAttested` (**32**); unattested **31** rejected in that config; solo/unpinned/`|pinned|≤1` keep **31**; stake-weighted ⌈2W/3⌉ and registry V3 unchanged (no required clear); still **16 MiB** default; ReservedRealV1 verify still rejects; admission height unchanged; **true BLS aggregate still later**. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.21] - 2026-10-22

### Added

- **L1 media blobs Phase 18** — `phase18DasSuccinctProofPlan`; `PHASE18_*` aliases including `PHASE18_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX` (**32**); `encodeResolveSlashAppealAttested` carries domain-separated Ed25519 ballot (verify-each — not BLS); stake-weighted ⌈2W/3⌉ unchanged; registry V3 (no required clear); **31** retained unattested; still **16 MiB** default; ReservedRealV1 verify still rejects; admission height unchanged; BLS aggregate still later. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.20] - 2026-10-22

### Added

- **L1 media blobs Phase 17** — `phase17DasSuccinctProofPlan`; `PHASE17_*` aliases including `PHASE17_RESOLVE_THRESHOLD_NUM` / `DEN` (⌈2W/3⌉); **ResolveSlashAppeal (31)** is stake-weighted quorum over pinned resolvers (weight = bonded stake at cast); versioned `slash_appeals_registry.bin` (no required clear); still **16 MiB** default; ReservedRealV1 verify still rejects; admission height unchanged. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.19] - 2026-10-22

### Added

- **L1 media blobs Phase 16** — `phase16DasSuccinctProofPlan`; `PHASE16_*` aliases including `PHASE16_RESOLVE_THRESHOLD_NUM` / `DEN` (⌈2M/3⌉); **ResolveSlashAppeal (31)** is an N-of-M committee threshold vote (finalize restore only at quorum; M=1 still one-shot); payload index unchanged; still **16 MiB** default; ReservedRealV1 verify still rejects; admission height unchanged. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.18] - 2026-10-22

### Added

- **L1 media blobs Phase 15** — `phase15DasSuccinctProofPlan`; `PHASE15_*` aliases; consensus book seeded for equivocation/liveness; stake restore via existing Submit/Resolve (**30**/**31**); still **16 MiB** default; ReservedRealV1 verify still rejects; admission height unchanged. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.17] - 2026-10-22

### Added

- **L1 media blobs Phase 14** — `phase14DasSuccinctProofPlan`; `PHASE14_*` aliases; `encodeSubmitSlashAppeal` / `encodeResolveSlashAppeal` (payload indices **30** / **31**); consensus FailedServe unburn via block replay; ReservedRealV1 verify still rejects; admission height unchanged; live Put/Announce stays **16 MiB** unless gated. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.16] - 2026-10-22

### Added

- **L1 media blobs Phase 13** — `phase13DasSuccinctProofPlan`; `PHASE13_*` aliases; FailedServe approved appeal restores provider **balance**; settle-receipt TTL; slash list filter notes. Live Put/Announce stays **16 MiB** unless gated. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.15] - 2026-10-21

### Added

- **L1 media blobs Phase 12** — `phase12DasSuccinctProofPlan`; `PHASE12_*` aliases; settle-before-challenge + FailedServe `slashId` / SlashRegistry audit notes. Challenge requires prior Settle. Live Put/Announce stays **16 MiB** unless gated. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.14] - 2026-10-21

### Added

- **L1 media blobs Phase 11** — `PHASE11_SERVE_CHALLENGE_BOND` / `PHASE11_FAILED_SERVE_PENALTY`; `blobSampleServeChallengeBondEscrowAccount`; `phase11DasSuccinctProofPlan`. Challenge escrows bond+penalty; FailedServe refunds bond and burns penalty; ReservedRealV1 envelope checks (verify still rejects). Live Put/Announce stays **16 MiB** unless gated. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.13] - 2026-10-21

### Added

- **L1 media blobs Phase 10** — `challengeBlobSampleServePayload` (28), `respondBlobSampleServePayload` (29); `phase10DasSuccinctProofPlan`; `PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS`. Serve-challenge path + ReservedRealV1 opaque DAS scaffolding (verify rejects). Live Put/Announce stays **16 MiB** unless gated. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.12] - 2026-10-21

### Added

- **L1 media blobs Phase 9** — `cancelBlobSampleOfferPayload` (27); `blobSampleAskEscrowAccount` / `dasLightClientSeedFromLeaderVrf`; `PHASE9_ACTIVATED_MAX_MEDIA_BLOB_BYTES`. Client: `getLeaderVrfOutput`, `listSmtBlobSampleOffers`. SMT-resident offers + ask escrow on the node; succinct DAS remains StubBlake3V0. Live Put/Announce stays **16 MiB** unless gated cap activation is set. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.11] - 2026-10-21

### Added

- **L1 media blobs Phase 8** — `quoteBlobSampleSettle`; `buildDasSuccinctProofStub` / `buildDasSuccinctProofStubVrf` / `verifyDasSuccinctProofStub` (StubBlake3V0 — **not** a SNARK); `PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES`. Client: `quoteBlobSampleSettle`, `verifyBlobDasSuccinctProof`; LC batch may pass optional `vrfOutput`. Live Put/Announce stays **16 MiB** unless gated Phase 7 cap activation is set. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.10] - 2026-10-21

### Added

- **L1 media blobs Phase 7** — `registerBlobSampleOfferPayload` (25), `settleBlobSampleMatchPayload` (26); `mediaBlobMaxBytesAt` / `PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES`; `dasLightClientSeedFromVrf`, `dasSuccinctCommitmentStub`. Live Put/Announce stays **16 MiB** unless the node enables gated Phase 7 cap activation. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.9] - 2026-10-21

### Added

- **L1 media blobs Phase 6** — `PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES` (256 MiB design target, **not** live), `splitIntoLiveCapSegments` / `manifestFromSegmentBytes` / `segmentManifestId`, sample-offer match with expiry, `dasLightClientSeed` + indices. Client RPCs: `getBlobDasLightClientBatch`, `putBlobSampleOffer`, `listBlobSampleOffers`, `matchBlobSampleOffer` (node-local book). Live Put/Announce cap remains **16 MiB**. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.8] - 2026-10-20

### Added

- **L1 media blobs Phase 5** — `renewMediaBlobPinPayload` (23), `payBlobSamplePayload` (24); RS parity constant; pin horizon hint. Network: Reed–Solomon recovery, renew/sample fee hooks. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.7] - 2026-10-20

### Changed

- **L1 media blobs Phase 4** — live cap **16 MiB** (`MAX_MEDIA_BLOB_BYTES`); prefer BlobAnnounce above `MAX_MEDIA_BLOB_INLINE_PREFER_BYTES` (1 MiB).

### Added

- `chunkMediaBlob` / `recoverMediaBlobXor` / `mediaBlobChunksMerkleRoot`; client `getBlobDasSample`. Forever pin fee is on-chain via `NetworkFeePolicy.blob_forever_pin_fee_per_kib` (see network docs).

## [0.5.6] - 2026-10-20

### Added

- **L1 media blobs Phase 3 scaffold** — `PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES` (16 MiB, not live), `MEDIA_BLOB_DAS_CHUNK_BYTES`, `mediaBlobDasChunkCount`, `mediaBlobPinRentKibBlocks`; optional `BlockHeader.blobs_root`. Live cap remains **1 MiB**. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.5] - 2026-10-20

### Added

- **L1 media blobs Phase 2** — cap **1 MiB**; `blobAnnouncePayload` / `PayloadVariant.BlobAnnounce` (22); `tokenMetadataJsonWithBlobImage`; `extractBoingBlobUri` / `blobIdHexFromBoingUri`; `metadataHashWordToFetchUrls` prefers `boing://blob/0x…`. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md).

## [0.5.4] - 2026-10-20

### Added

- **L1 media blobs (MVP)** — `mediaBlob.ts` (`blobIdOf`, `putBlobPayload`, URI helpers, gas math), `PayloadVariant.PutBlob` (21) + `BlobRetentionVariant`, client `getMediaBlobParams` / `getBlobMeta` / `getBlob`. Docs: [BOING-L1-MEDIA-BLOBS.md](../docs/BOING-L1-MEDIA-BLOBS.md). Cap **256 KiB**; separate from interface SSTORE blobs.
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
