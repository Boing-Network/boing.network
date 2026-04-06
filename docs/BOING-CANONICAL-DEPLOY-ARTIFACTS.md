# Canonical native deploy artifacts (fungible + NFT)

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

### Versioning (fungible)

| Version | Artifact id | BLAKE3 (init/runtime payload) | Notes |
|---------|-------------|----------------------------------|--------|
| **1** | `boing.reference_fungible.v0` | Run `cargo run -p boing-execution --example dump_reference_token_artifacts` and hash the **second** `0x` line | Lazy **admin** (first caller); one-time **`mint_first`** (admin only); **`transfer`** with balance + overflow checks. Purpose **`token`**. |

**SDK:** `resolveReferenceFungibleTemplateBytecodeHex` (embedded default + env overrides), `REFERENCE_FUNGIBLE_TEMPLATE_VERSION` = **`1`**. Regenerate TS hex: `node boing-sdk/scripts/embed-reference-fungible-template-hex.mjs`.

When bytecode changes, ops should:

1. Run `cargo test -p boing-execution` (including QA allow/unsure for purpose `token`).
2. Regenerate `boing-sdk/src/defaultReferenceFungibleTemplateBytecodeHex.ts` via the script above (or paste stdout line 2).
3. Bump `REFERENCE_FUNGIBLE_TEMPLATE_VERSION` if the pinned binary changes.

### dApp wiring (today)

1. **`boing-sdk`:** `buildContractDeployMetaTx`, `resolveReferenceFungibleTemplateBytecodeHex` — see package exports.
2. **Wizard shortcut (recommended):** **`buildReferenceFungibleDeployMetaTx({ assetName, assetSymbol })`** — single call for **pinned bytecode + `contract_deploy_meta`** (same shape **boing.finance** should use inside a multi-chain **Deploy token** wizard). Collections: **`buildReferenceNftCollectionDeployMetaTx`** (needs NFT template env or **`bytecodeHexOverride`**).
3. **Chain picker:** **`isBoingTestnetChainId(chainId)`** / **`normalizeBoingChainIdHex`** — branch the wizard when the user selects Boing testnet (**6913**).
4. **QA on the review step:** **`preflightContractDeployMetaQa(client, tx)`** wraps **`boing_qaCheck`** with the correct optional params (uses a **placeholder `description_hash`** when the wizard has not committed one yet — [RPC-API-SPEC.md](RPC-API-SPEC.md) § **boing_qaCheck**).
5. **Build-time env (optional override):**  
   `BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX` — `0x` + hex when you need a **non-default** ops-approved binary.
6. **Browser:** inject the same via your bundler (`define` / `import.meta.env` pattern) and pass the string into `resolveReferenceFungibleTemplateBytecodeHex({ explicitHex: … })` or **`bytecodeHexOverride`** on the shortcut builders.
7. **Preflight:** `boing_qaCheck` / wallet flow — category **`token`** often yields **`unsure`** (community pool); handle like existing **boing.finance** “acknowledge pool” UX.

---

## NFT collection template

| Field | Status |
|--------|--------|
| **Reference calldata** | **Defined** — `owner_of`, `transfer_nft`, `set_metadata_hash` ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)) |
| **SDK encoders** | **Shipped** — `encodeReferenceOwnerOfCalldata`, etc. |
| **Canonical collection bytecode** | **Shipped** — `reference_nft_collection_template_bytecode()` (`boing-execution` / `reference_nft.rs`) |
| **Marketplace / royalties (F2 doc)** | **Roadmap** — on-chain binding royalties still app-layer ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) § Marketplace) |

### Versioning (NFT)

| Version | Artifact id | BLAKE3 (deploy payload) | Notes |
|---------|-------------|-------------------------|--------|
| **1** | `boing.reference_nft_collection.v0` | Run `cargo run -p boing-execution --example dump_reference_token_artifacts` and hash the **second** `0x` line | Lazy **admin** (first caller); `owner_of` / `transfer_nft` / `set_metadata_hash`; mint = admin-only when `owner_of` is zero. Purpose **`nft`** / **`NFT`**. |

**SDK:** `resolveReferenceNftCollectionTemplateBytecodeHex`, env `BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX` (and `VITE_` / `REACT_APP_` variants), `REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION` = **`1`**.

**Upgrades (product):**

- **Phase B — F2 marketplace:** optional selectors or companion contracts for **binding** royalties ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) § Marketplace).

---

## Smoke contract (not a user token)

`boing_execution::smoke_contract_bytecode()` is a **tiny** program for VM / pool tests. It does **not** implement reference **balances** or a fungible token. **Do not** ship it as the default “Deploy token” template in consumer UIs.

Hex dump for debugging only:

```bash
cargo run -p boing-execution --example dump_reference_token_artifacts
```

Stdout: line **1** = smoke (not a token), line **2** = **fungible** template, line **3** = **NFT collection** template.

---

## Handoff: boing.finance (and similar apps)

1. **Token deploy UX:** **boing.finance** routes **Boing testnet + Boing Express** through the same **Launch Wizard** as EVM (**Token Basics → Network & Plan → Security & Info → Review & Deploy**). The final **Deploy** button submits **`contract_deploy_meta`** via Express; **Advanced** (bytecode override, `description_hash`, explicit QA) lives on the review step only.
2. **Default path:** If `resolveReferenceFungibleTemplateBytecodeHex()` returns a string, users see **no bytecode** on the main path — same rhythm as EVM **form → approve in wallet**. Prefer **`buildReferenceFungibleDeployMetaTx({ assetName, assetSymbol })`** in code so the wizard does not manually pair **`resolve` + `buildContractDeployMetaTx`**.
3. **Advanced:** Keep **paste bytecode** + `description_hash` + QA for power users.
4. **Copy:** Native deploy uses **Boing Express**, not MetaMask; avoid implying an ERC-20 factory on L1.
5. **NFT page:** The **Create NFT** wizard (collection → images → metadata → review) is shared; **on-chain native collection** deploy uses **Native VM** in the app with **`purpose_category: 'nft'`** when collection bytecode is configured (`resolveReferenceNftCollectionTemplateBytecodeHex` + `buildContractDeployMetaTx`). Export metadata JSON from the wizard for any toolchain.

---

## References

- [E2-PARTNER-APP-NATIVE-BOING.md](E2-PARTNER-APP-NATIVE-BOING.md)  
- [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) — `contract_deploy_meta`  
- `boing-sdk` — `canonicalDeployArtifacts.ts`  
- `examples/native-boing-tutorial/` — scripted deploy patterns
