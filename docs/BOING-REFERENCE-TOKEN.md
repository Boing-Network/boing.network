# Boing reference fungible layout (off-chain standard)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This document defines a **recommended** calldata layout for fungible-token-style contracts on the **Boing VM**. It is **not** a consensus-enforced transaction type: deployers ship ordinary `ContractDeploy` bytecode that implements (or ignores) this ABI. All deploys still pass **protocol QA** (`boing-qa`).

## Principles

- **Boing VM only.** Opcodes and semantics are Boing-defined (`TECHNICAL-SPECIFICATION.md` §7, [BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md)).
- **Storage layout** is contract-defined. A common pattern is **balance mapping**: `SLOAD` / `SSTORE` with key = holder `AccountId` (32 bytes) and value = balance as a 32-byte big-endian word (practical amounts often use the low 16 bytes as `u128`).
- **Access control** uses **`CALLER`** (`0x33`) to push the **immediate caller** `AccountId` and compare with stored roles (e.g. minter). At **top-level** that is the transaction signer; if a **pool or router** contract uses nested **`Call` (`0xf1`)** to invoke your token, **`Caller` inside the token** is that **contract’s** id (like `msg.sender` being the pool on other chains), not the end-user.

## Calldata (reference)

Total **96 bytes** per call:

| Offset | Length | Content |
|--------|--------|---------|
| 0 | 32 | Selector word: 31 zero bytes + one-byte selector in the **last** byte (index 31). |
| 32 | 32 | `to` — recipient `AccountId` |
| 64 | 32 | `amount` — unsigned amount; reference encoding uses big-endian `u128` in the **low 16 bytes** (high 16 bytes zero). |

### Selectors (reference)

| Selector (low byte) | Name | Intended meaning |
|---------------------|------|------------------|
| `0x01` | `transfer` | Move `amount` from `CALLER` balance to `to` (contract must enforce balances). |
| `0x02` | `mint_first` | **Example** hook for “first minter” or treasury bootstrap; concrete rules belong in contract bytecode + docs. |

### Rust / SDK helpers

- Rust: `boing_execution::encode_transfer_calldata`, `encode_mint_first_calldata`, constants `SELECTOR_TRANSFER`, `SELECTOR_MINT_FIRST`.
- TypeScript: `boing-sdk` — `encodeReferenceTransferCalldata`, `encodeReferenceMintFirstCalldata`.

## Canonical deploy template (pinned bytecode)

dApps that want **EVM-style “form only”** deploy should ship **versioned** Boing VM bytecode implementing this calldata layout, submit **`contract_deploy_meta`**, and hide hex behind an **Advanced** panel until ops publishes a default binary.

- **Status table, env keys, and handoff for frontends:** [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md)
- **SDK helpers:** `boing-sdk` — `buildContractDeployMetaTx`, `resolveReferenceFungibleTemplateBytecodeHex` (pinned default + env override)
- **Canonical template:** `boing_execution::reference_fungible_template_bytecode()` — balances + `transfer` / `mint_first`; QA purpose **`token`** ([EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) **C6**).

## Secured template (pinned `0xFD` deploy)

For dApps that want the **same 96-byte** `transfer` / `mint_first` layout but **optional on-chain enforcement** of policy toggles, use the **secured** line item in [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md):

- **Rust:** `reference_fungible_secured_pinned_default_deploy_bytecode()` or `reference_fungible_secured_deploy_bytecode(&ReferenceFungibleSecuredConfig)` (`boing_execution::reference_fungible_secured`). Init stores **admin** (= deployer), **flags** (denylist, max tx, max wallet, anti-bot window, cooldown, no-mint, transfer-unlock), numeric limits, and optional **initial pause**, then **`RETURN`s** the runtime.
- **Runtime checks (when flags are set):** denylist on **caller** and **recipient**, **pause**, per-tx and per-wallet caps, **anti-bot** window + max amount, **per-sender cooldown** (uses **`Timestamp`**), **minimum block height** for transfers (**`BlockHeight`**), **`no_mint`**.
- **Admin calldata** (same 96-byte word layout: selector low byte + `to` word + amount word):
  | Selector | Meaning |
  |----------|---------|
  | `0x03` | **Set deny** — `to` = subject; amount word **zero** clears deny, **non-zero** sets denied. |
  | `0x04` | **Set pause** — amount word **zero** unpauses, **non-zero** pauses. |
  | `0x05` | **Renounce admin** — clears admin slot (irreversible). |
  | `0x07` | **Set transfer-unlock height** — amount word carries **u64** BE in low **8** bytes (requires transfer-unlock flag). |
- **SDK:** `resolveReferenceFungibleSecuredTemplateBytecodeHex`, `buildReferenceFungibleSecuredDeployMetaTx` (with **`nativeTokenSecurity`** to encode wizard toggles into **`0xFD` init**; **`chainContext.chainHeight`** required when **`timelock`** is on; **`mintFirstTotalSupplyWei`** for **`maxWalletPercentage`**), `referenceFungibleSecuredConfigFromNativeTokenSecurity`, env `BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX` (+ `VITE_` / `REACT_APP_` variants). **Pinned default** (no `nativeTokenSecurity`) keeps enforcement flags **off**. Custom Rust configs remain available via `reference_fungible_secured_deploy_bytecode`.

## Reference fungible **v3** (`reference_fungible_template_v3_bytecode`)

Same selectors and balance layout as **v2**, but allowance slots use
`BLAKE3(pad32("BOING_REF_FUNGIBLE_ALLOWANCE_V3") ‖ owner ‖ spender)` via live
`Opcode::Blake3` (`0x20`). Requires `BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT`
(default never). CLI/SDK: `--version v3` / `BoingTokenVersion = 3`. Host twin:
`ref_fungible_allowance_key_v3_host` / `boingTokenAllowanceStorageKeyV3Hex`.

## Reference fungible **v2** (`reference_fungible_template_v2_bytecode`)

New, versioned template; the v1 bytecode above is unchanged. Same lazy admin, one-time `mint_first`, `transfer` and balance layout (`holder ^ REF_FUNGIBLE_BALANCE_XOR`), plus:

| Selector | Calldata | Behaviour |
|---|---|---|
| `0x08` `approve` | `sel ‖ spender ‖ amount` (96 B) | Sets `allowance[caller][spender] = amount` (overwrites; no increase/decrease race helper). Returns word `1`. |
| `0x09` `transfer_from` | `sel ‖ from ‖ to ‖ amount` (128 B) | Requires `amount > 0`, `from ≠ to`, allowance ≥ amount and balance ≥ amount; decrements both and credits `to`. Returns word `1`; on any failure it stops with **no** return data and no state change (callers such as pool v8 check for word `1`). |
| `0x0a` `allowance` | `sel ‖ owner ‖ spender` (96 B) | Returns the allowance word. |

Allowance slot: `(owner[0..16] ‖ spender[0..16]) ^ REF_FUNGIBLE_ALLOWANCE_XOR` (ASCII `BOING_REFFUNGIBLE_ALW2`, zero-padded), readable with `boing_getContractStorage`.

**v1 self-transfer bug (fixed in v2; v1 retired for new deploys):** in the v1 template, `transfer` to yourself credits the new balance without debiting first, so a holder doubles their balance (mint 100, self-transfer 100 → 200). This is pinned by the test `v1_template_self_transfer_double_credits_known_bug`. The v1 bytecode is left untouched (already-deployed tokens can't change), so **treat supply of v1 tokens as untrusted**. **`boing token deploy` and `deployBoingToken` refuse v1** (use `--version v2` / default). Secured **v2** is the pinned SDK/CLI default for new secured deploys (same self-transfer no-op); secured v1 is retired the same way.

## CLI: `boing token` (Rust, no SDK required)

`boing-cli` ships a Boing-native token flow built only on `boing-primitives` + `boing-execution` (no EVM / Solana runtimes):

| Command | What it does |
|---------|--------------|
| `boing token deploy --symbol TKN [--name ...] [--initial-supply N] [--mint-to <id>] [--version v2]` | Signs `ContractDeployWithPurposeAndMetadata` (purpose **`token`**) with the reference template (**v2 only** for new deploys; v1 retired); prints the nonce-derived token address; optionally sends `mint_first` at nonce+1 (fixed supply). |
| `boing token mint --token <id> --amount N [--to <id>]` | One-time `mint_first` (admin = first caller). |
| `boing token transfer --token <id> --to <id> --amount N` | Reference `transfer`. |
| `boing token balance --token <id> --holder <id>` | Reads `boing_getContractStorage` at `holder ^ REF_FUNGIBLE_BALANCE_XOR` and prints the `u128` balance. |
| `boing token approve --token <id> --spender <id> --amount N` | v2 `approve` (overwrites). |
| `boing token allowance --token <id> --owner <id> --spender <id>` | Reads the v2 allowance slot. |
| `boing token bytecode [--version v2\|v1]` | Prints the template hex. |

Signer is a 32-byte Ed25519 seed from `--secret-hex` or `BOING_SECRET_HEX`. `--nonce` overrides the `boing_getAccount` lookup; `--dry-run` prints signed tx hex instead of calling `boing_submitTransaction`. Global `--rpc-url` selects the node.

## SDK: token helpers (`boing-sdk`, mirrors `boing token`)

`boing-sdk` exports the same flow for TypeScript (`src/boingToken.ts`). Signed transactions are **byte-identical** to the CLI for the same seed, nonce and arguments; `boing-sdk/tests/boingToken.test.ts` checks this against vectors captured from `boing token ... --dry-run` (`boing-sdk/tests/fixtures/token-cli-vectors.json`, public test seed `0x07…07`).

| CLI | SDK |
|-----|-----|
| `boing token deploy` | `deployBoingToken(client, secretKey32, { name, symbol, initialSupply, mintToHex, nonce?, dryRun? })` returns `{ deployerHex, tokenHex, deployTx, mintTx }` |
| `boing token mint` | `mintBoingToken(client, secretKey32, { tokenHex, toHex?, amount, nonce?, dryRun? })` |
| `boing token transfer` | `transferBoingToken(client, secretKey32, { tokenHex, toHex, amount, nonce?, dryRun? })` |
| `boing token balance` | `getBoingTokenBalance(client, tokenHex, holderHex)` returns `bigint` |
| `boing token approve` | `approveBoingToken(client, secretKey32, { tokenHex, spenderHex, amount, nonce?, dryRun? })` |
| `boing token allowance` | `getBoingTokenAllowance(client, tokenHex, ownerHex, spenderHex)` |
| `boing token bytecode` | `boingTokenBytecodeHex(version = 2)` |

`deployBoingToken` / `buildBoingTokenDeployTransaction` take `version` (default **2**, like the CLI). The original `token-cli-vectors.json` vectors are v1 (`version: 1`); v2 deploy/approve vectors and the allowance key come from the CLI's Rust test (`pool-cli-vectors.json`, `v8` block).

Lower level: `buildBoingTokenDeployTransaction`, `buildBoingTokenMintTransaction`, `buildBoingTokenTransferTransaction` (unsigned `TransactionInput` for `signTransactionInput` or a wallet signer), `boingTokenBalanceStorageKeyHex`, `decodeBoingTokenBalanceWord`, `validateBoingTokenMetadata`, `BOING_TOKEN_PURPOSE`. Each tx result is `{ txId, signedHex, submitted }`; with `dryRun` nothing is submitted, and `client` may be `null` if `nonce` is given.
## Smoke contract

`boing_execution::smoke_contract_bytecode()` is a minimal program used in tests: it stores the caller, emits `LOG0` over the first four calldata bytes, and returns the caller id. It is **not** a token.

## NFTs

See **[BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)** for the reference NFT calldata layout (`owner_of`, `transfer_nft`, `set_metadata_hash`, and template-v3 **`mint_batch` (`0x06`, `96+64n`, n≤500)`).

## QA

Token and NFT deploys should use purpose categories (`token`, `NFT`, …) as required by `QUALITY-ASSURANCE-NETWORK.md`. Bytecode must remain within the **Boing VM opcode whitelist** and well-formedness rules.

## Wizard security metadata (dApp parity)

Partner apps may commit ERC-20-style **security feature** toggles in **`description_hash`** using the canonical JSON schema **`boing.native_token_security.v1`** (Blake3-256 digest; see `boing-sdk` **`nativeTokenSecurity.ts`** / **`buildReferenceFungibleDeployMetaTx`** / **`buildReferenceFungibleSecuredDeployMetaTx`** field **`nativeTokenSecurity`**). **`description_hash`** is **metadata for wallets/indexers**; the **minimal** reference template does not read it. The **secured** template can enforce **on-chain** rules when the corresponding **init flags** are set (see § Secured template). VM **`BlockHeight` (`0x40`)** and **`Timestamp` (`0x41`)** support block/time checks (`TECHNICAL-SPECIFICATION.md` §7.2).

**Linked NFT collections (enforced):** on-chain registry (`linked_nft_token_registry` / `boing-sdk` **`linkedNftTokenRegistry.ts`**). **Public testnet:** `0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8` (`https://testnet-rpc.boing.network/`). Optional schema **`boing.linked_nft_token.v1`** in **`description_hash`** / off-chain keys is a **cache only**. See [BOING-LINKED-NFT-TOKEN.md](BOING-LINKED-NFT-TOKEN.md).

## Decimals, name and symbol on chain

The reference fungible templates (v1, v2, secured v1/v2) store **whole-unit balances** and have **no decimals, name, symbol or total-supply slot**. Their on-chain decimals are therefore genuinely **0**: a wallet or explorer showing whole numbers for these tokens is correct, not a display bug. The node reports this as `decimals: 0`, `decimalsSource: "template"`; only wrapped BOING (18) and operator-mapped tokens (`BOING_DEX_TOKEN_DECIMALS_JSON`) differ. Name and symbol exist only in the `ContractDeployWithPurposeAndMetadata` deploy transaction; total supply is the single `mint_first` amount. All of this is exposed by [`boing_getTokenMetadata` / `boing_listTokens`](RPC-API-SPEC.md).

A token that wants fractional amounts, or metadata readable from contract storage, needs a **new template** that stores a decimals word (and optionally name/symbol/supply) and a `decimals()` view selector. That is a new deployable contract, not a protocol change, but existing tokens cannot gain decimals retroactively (bytecode is immutable).
