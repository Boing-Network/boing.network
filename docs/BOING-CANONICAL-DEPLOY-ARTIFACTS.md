# Canonical native deploy artifacts (fungible + NFT)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This document is the **integration anchor** for dApps (e.g. **boing.finance**) that want **EVM-style “form only” deploy** on Boing L1: **pinned Boing VM bytecode** + **`contract_deploy_meta`** (or `contract_deploy_purpose`) + Boing Express signing.

It pairs with [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md) (calldata layout), [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md), [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md), and [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md).

---

## Why this exists

- **EVM apps** ship fixed bytecode (e.g. `ERC20_BYTECODE`) and only ask for name, symbol, supply.
- **Native Boing** historically asked integrators to **paste VM bytecode** because a **single ops-approved fungible program** was not yet published as a stable artifact.
- **Goal:** Same **product shape**: one (or a few) **versioned** binaries + form fields; bytecode is an **implementation detail** unless the user opens “Advanced”.

Signing **always** uses **Boing Express** (or another native signer). MetaMask does **not** sign Boing VM transactions.

---

## Fungible token template

| Field | Status |
|--------|--------|
| **Reference calldata** | **Defined** — `transfer` / `mint_first` ([BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)) |
| **SDK encoders** | **Shipped** — `encodeReferenceTransferCalldata`, `encodeReferenceMintFirstCalldata` |
| **Canonical deploy bytecode (balances token)** | **Shipped** — `reference_fungible_template_bytecode()` (`boing-execution` / `reference_token.rs`), purpose **`token`** |
| **Secured fungible (`0xFD` init + runtime)** | **Shipped** — `reference_fungible_secured_pinned_default_deploy_bytecode()` (`boing-execution` / `reference_fungible_secured.rs`), purpose **`token`**; init sets admin + optional enforcement flags ([BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md) § Secured template) |

### Versioning (fungible)

| Version | Artifact id | BLAKE3 (init/runtime payload) | Notes |
|---------|-------------|----------------------------------|--------|
| **1** | `boing.reference_fungible.v0` | Run `cargo run -p boing-execution --example dump_reference_token_artifacts` and hash the **second** `0x` line | Lazy **admin** (first caller); one-time **`mint_first`** (admin only); **`transfer`** with balance + overflow checks. Purpose **`token`**. |

**SDK:** `resolveReferenceFungibleTemplateBytecodeHex` (embedded default + env overrides), `REFERENCE_FUNGIBLE_TEMPLATE_VERSION` = **`1`**. Regenerate TS hex: `node boing-sdk/scripts/embed-reference-fungible-template-hex.mjs`.

### Versioning (secured fungible)

| Version | Artifact id | BLAKE3 (deploy payload) | Notes |
|---------|-------------|-------------------------|--------|
| **1** | `boing.reference_fungible_secured.v0` | Run `cargo run -p boing-execution --example dump_reference_token_artifacts` and hash the **fourth** `0x` line | **`0xFD`** init bootstraps storage (admin = deployer, flags, caps, anti-bot window, cooldown, pause, etc.) and **`RETURN`s** the runtime. Same **96-byte** `transfer` / `mint_first` calldata as the minimal template; extra **admin** selectors **`0x03`–`0x07`** (deny, pause, renounce, transfer-unlock). Default pinned config has enforcement flags **off** (same rough UX as minimal fungible). **`buildReferenceFungibleSecuredDeployMetaTx`** with **`nativeTokenSecurity`** builds matching init bytecode on-chain; pass **`chainContext.chainHeight`** when **`timelock`** is enabled. |

**SDK:** `resolveReferenceFungibleSecuredTemplateBytecodeHex`, `buildReferenceFungibleSecuredDeployMetaTx` (optional `nativeTokenSecurity`, `chainContext`, `mintFirstTotalSupplyWei`), `referenceFungibleSecuredConfigFromNativeTokenSecurity`, `DEFAULT_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX`, `DEFAULT_REFERENCE_FUNGIBLE_SECURED_RUNTIME_BYTECODE_HEX` (fifth stdout line; init builder), `REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION` = **`1`**. Regenerate TS hex: `node boing-sdk/scripts/embed-reference-fungible-secured-template-hex.mjs`.

**Env (optional override):** `BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX`, `VITE_BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX`, `REACT_APP_BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX`.

**Execution note:** Deploy init runs with a **higher** interpreter gas budget than ordinary calls (`GAS_PER_CONTRACT_DEPLOY_INIT` in `boing-execution` `vm.rs`) so large **`MSTORE`/`RETURN`** of the runtime does not **`OutOfGas`**.

When bytecode changes, ops should:

1. Run `cargo test -p boing-execution` (including QA allow/unsure for purpose `token`).
2. Regenerate `boing-sdk/src/defaultReferenceFungibleTemplateBytecodeHex.ts` via the script above (or paste stdout line 2).
3. Bump `REFERENCE_FUNGIBLE_TEMPLATE_VERSION` if the pinned binary changes.

For **secured** template changes, repeat with `cargo test -p boing-execution` (including `reference_fungible_secured` tests), regenerate `defaultReferenceFungibleSecuredTemplateBytecodeHex.ts`, and bump `REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION`.

### dApp wiring (today)

1. **`boing-sdk`:** `buildContractDeployMetaTx`, `resolveReferenceFungibleTemplateBytecodeHex`, **`resolveReferenceFungibleSecuredTemplateBytecodeHex`** — see package exports.
2. **Wizard shortcut (recommended):** **`buildReferenceFungibleDeployMetaTx({ assetName, assetSymbol })`** — single call for **pinned minimal bytecode + `contract_deploy_meta`**. Use **`buildReferenceFungibleSecuredDeployMetaTx`** when you want the **secured** pinned default (same meta-tx shape; different bytecode). Collections: **`buildReferenceNftCollectionDeployMetaTx`** (needs NFT template env or **`bytecodeHexOverride`**).
3. **Chain picker:** **`isBoingTestnetChainId(chainId)`** / **`normalizeBoingChainIdHex`** — branch the wizard when the user selects Boing testnet (**6913**).
4. **QA on the review step:** **`preflightContractDeployMetaQa(client, tx)`** wraps **`boing_qaCheck`** with the correct optional params (uses a **placeholder `description_hash`** when the wizard has not committed one yet — [RPC-API-SPEC.md](RPC-API-SPEC.md) § **boing_qaCheck**).
5. **Build-time env (optional override):**  
   `BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX` — `0x` + hex when you need a **non-default** ops-approved **minimal** binary.  
   `BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX` (and `VITE_` / `REACT_APP_` variants) for the **secured** template.
6. **Browser:** inject the same via your bundler (`define` / `import.meta.env` pattern) and pass the string into `resolveReferenceFungibleTemplateBytecodeHex({ explicitHex: … })`, **`resolveReferenceFungibleSecuredTemplateBytecodeHex`**, or **`bytecodeHexOverride`** on the shortcut builders.
7. **Preflight:** `boing_qaCheck` / wallet flow — category **`token`** often yields **`unsure`** (community pool); handle like existing **boing.finance** “acknowledge pool” UX.

---

## NFT collection template

| Field | Status |
|--------|--------|
| **Reference calldata** | **Defined** — `owner_of`, `transfer_nft`, `set_metadata_hash`, **`mint_batch` (`0x06`, layout `96+64n`)** ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)) |
| **SDK encoders** | **Shipped** — `encodeReferenceOwnerOfCalldata`, `encodeReferenceTransferNftCalldata`, `encodeReferenceSetMetadataHashCalldata`, **`encodeReferenceMintBatchCalldata`** / **`encodeReferenceMintBatchCalldataHex`**, `MAX_REFERENCE_NFT_MINT_BATCH` (= **500** on template v3) |
| **Canonical collection bytecode** | **Shipped** — `reference_nft_collection_template_bytecode()` (`boing-execution` / `reference_nft.rs`); dump **third** `0x` line |
| **Marketplace / royalties (F2 doc)** | **Roadmap** — on-chain binding royalties still app-layer ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) § Marketplace) |

### Versioning (NFT)

| Version | Artifact id | BLAKE3 (deploy payload) | Notes |
|---------|-------------|-------------------------|--------|
| **1** | `boing.reference_nft_collection.v0` | Historical: dump **third** `0x` line from the v1 binary | Lazy admin; `owner_of` / `transfer_nft` / `set_metadata_hash` only. **Cannot** be upgraded in place. |
| **2** | `boing.reference_nft_collection.v0` | Pinned: [`artifacts/reference-nft-collection-template-v2.hex`](artifacts/reference-nft-collection-template-v2.hex) (SHA-256 [`artifacts/reference-nft-collection-template-v2.sha256`](artifacts/reference-nft-collection-template-v2.sha256)) | **`mint_batch` (`0x06`)**, max **n = 50**. Existing v2 collections keep this cap. |
| **3** | `boing.reference_nft_collection.v0` | Pinned: [`artifacts/reference-nft-collection-template-v3.hex`](artifacts/reference-nft-collection-template-v3.hex) (SHA-256 [`artifacts/reference-nft-collection-template-v3.sha256`](artifacts/reference-nft-collection-template-v3.sha256)); regenerate via dump **third** `0x` line | Same storage XOR keys + **`mint_batch`**, max **n = 500**. Needs node **`GAS_PER_CONTRACT_CALL = 40_000_000`**. Redeploy for new collections. Purpose **`nft`** / **`NFT`**. |

**SDK:** `resolveReferenceNftCollectionTemplateBytecodeHex`, env `BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX` (and `VITE_` / `REACT_APP_` variants), `REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION` = **`3`**. Embed: `node boing-sdk/scripts/embed-reference-nft-collection-template-hex.mjs`. Partner apps that keep a separate env name (e.g. FreshMint `BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_V3_BYTECODE_HEX`) should use the same pinned hex.

**v1 collections on chain never gain `0x06`.** v2 stays at n≤50. Do not replace bytecode. Empty drops: redeploy **v3**. Already-minted sets: one-tx-per-token or a new collection + remint unminted drafts. For 10k tokens: **20 × 500** txs (not one 10k consensus call).

**Batch mint (marketplaces / FreshMint-style apps):** encode with **`encodeReferenceMintBatchCalldata(Hex)`** — one consensus `ContractCall`, not N txs and not JSON-RPC HTTP batching. Access list = **AccountIds** (sender + collection). Poll **`boing_getTransactionReceipt(tx_id)`** (`Transaction::id()`); mempool `"ok"` is only an ack. **Hosted Fly testnet** needs a node redeploy with **`GAS_PER_CONTRACT_CALL = 40_000_000`** before n≈500 is usable on `testnet-rpc.boing.network` (see [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) ops banner). Pin hex: `cargo run -p boing-execution --example dump_reference_token_artifacts` (line **3**) or `node boing-sdk/scripts/embed-reference-nft-collection-template-hex.mjs`.

**Upgrades (product):**

- **Phase B — F2 marketplace:** optional selectors or companion contracts for **binding** royalties ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) § Marketplace).

---

## Smoke contract (not a user token)

`boing_execution::smoke_contract_bytecode()` is a **tiny** program for VM / pool tests. It does **not** implement reference **balances** or a fungible token. **Do not** ship it as the default “Deploy token” template in consumer UIs.

Hex dump for debugging only:

```bash
cargo run -p boing-execution --example dump_reference_token_artifacts
```

Stdout: line **1** = smoke (not a token), line **2** = **minimal fungible** template, line **3** = **NFT collection** template, line **4** = **secured fungible** deploy (`0xFD` + init).

---

## Handoff: boing.finance (and similar apps)

1. **Token deploy UX:** **boing.finance** routes **Boing testnet + Boing Express** through the same **Launch Wizard** as EVM (**Token Basics → Network & Plan → Security & Info → Review & Deploy**). The final **Deploy** button submits **`contract_deploy_meta`** via Express; **Advanced** (bytecode override, `description_hash`, explicit QA) lives on the review step only.
2. **Default path:** If `resolveReferenceFungibleTemplateBytecodeHex()` (or the **secured** resolver) returns a string, users see **no bytecode** on the main path — same rhythm as EVM **form → approve in wallet**. Prefer **`buildReferenceFungibleDeployMetaTx`** or **`buildReferenceFungibleSecuredDeployMetaTx`** in code so the wizard does not manually pair **`resolve` + `buildContractDeployMetaTx`**.
3. **Advanced:** Keep **paste bytecode** + `description_hash` + QA for power users.
4. **Copy:** Native deploy uses **Boing Express**, not MetaMask; avoid implying an ERC-20 factory on L1.
5. **NFT page:** The **Create NFT** wizard (collection → images → metadata → review) is shared; **on-chain native collection** deploy uses **Native VM** in the app with **`purpose_category: 'nft'`** when collection bytecode is configured (`resolveReferenceNftCollectionTemplateBytecodeHex` + `buildContractDeployMetaTx` / **`buildReferenceNftCollectionDeployMetaTx`**). Template **version `"3"`** (artifact id **`boing.reference_nft_collection.v0`**) includes **`mint_batch`** with **n ≤ 500**. After deploy, first-touch admin, then Express approval(s) for **`encodeReferenceMintBatchCalldata`** (preview: “Mint N NFTs to {to}”; result includes **`tx_id`**; chunk 10k as **20×500**). Export metadata JSON from the wizard for any toolchain.

---

## References

- [E2-PARTNER-APP-NATIVE-BOING.md](E2-PARTNER-APP-NATIVE-BOING.md)  
- [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) — `contract_deploy_meta`  
- `boing-sdk` — `canonicalDeployArtifacts.ts`  
- `examples/native-boing-tutorial/` — scripted deploy patterns
