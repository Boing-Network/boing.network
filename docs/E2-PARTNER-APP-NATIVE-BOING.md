# E2 — Partner apps: native Boing deploy / call (no foreign chain client SDK)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


**Roadmap:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) track **E2**.

This is the **canonical pattern** for apps (e.g. **boing.finance**) that already support **20-byte-address / injected-provider** flows but need a **native Boing** path when users connect **Boing Express** with a **32-byte AccountId**.

---

## Why a separate path exists

- Typical **`BrowserProvider.getSigner()`** stacks expect **20-byte addresses** and secp256k1-oriented signing.
- Boing Express exposes **Ed25519** accounts and **`boing_sendTransaction`** / **`boing_signTransaction`** for **Boing VM** payloads ([BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)).

---

## Recommended integration shape

### Browser (Boing Express injected)

1. Detect **native account**: **`isBoingNativeAccountIdHex(addr)`** from **`boing-sdk`** (`hex.ts`) — `0x` + **64** hex chars ([BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) §1).
2. For **deploy**: build a **tx object** accepted by the wallet (`contract_deploy_purpose` / `contract_deploy_meta` with valid **`purpose_category`** per [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)).
3. `await provider.request({ method: 'boing_sendTransaction', params: [txObject] })`.
4. Map errors with [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md).

### Node / scripted

Use **`boing-sdk`**:

- **`submitDeployWithPurposeFlow`** — QA preflight + simulate + submit.
- **`submitContractCallWithSimulationRetry`** — reference token/NFT calldata from [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md) / [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md).

Tutorial repo: [examples/native-boing-tutorial](../examples/native-boing-tutorial/).

---

## Token deploy on native Boing

- **Not** foreign fungible-token bytecode: implement or reuse a **Boing VM** contract that follows the **reference token** calldata layout for interoperability.
- Pre-flight **`boing_qaCheck`** with category **`token`** when appropriate.
- UI copy should distinguish **“Token on another network”** vs **“Native Boing token (VM)”** to avoid user confusion.
- **Form parity:** pin bytecode + **`buildContractDeployMetaTx`** — [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md).
- **Same wizard as EVM:** use **`buildReferenceFungibleDeployMetaTx`** + **`isBoingTestnetChainId`** + optional **`preflightContractDeployMetaQa`** so the Boing branch does not manually glue **`resolve` + `build`** ([BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) § **One wizard, two backends**).

---

## NFT deploy on native Boing

- Use **reference NFT** calldata ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)) and purpose **`NFT`** / **`nft`** for collection contracts.
- **Pinned collection bytecode** template **version `"3"`**, artifact id **`boing.reference_nft_collection.v0`** — same pattern as fungibles (version bump, artifact id stays). Hex: dump **third** `0x` line from `cargo run -p boing-execution --example dump_reference_token_artifacts`, or `node boing-sdk/scripts/embed-reference-nft-collection-template-hex.mjs`. Existing v2 collections stay at n≤50.
- **Mint path:** admin lazy-mint via `transfer_nft` (single token) **or** template-v3 **`mint_batch` (`0x06`, layout `96+64n`, n≤**`MAX_REFERENCE_NFT_MINT_BATCH`**=500**) via **`encodeReferenceMintBatchCalldata(Hex)`**. Atomic check-then-write; failures are `VmError` (not `STOP`). Zero metadata hash skips `SSTORE`. Access lists are **AccountIds** (sender + collection), not storage keys. For 10k mints use **20×500** chunks — not one 10k consensus tx. Nodes need **`GAS_PER_CONTRACT_CALL = 40_000_000`**.
- **Express:** one approval; preview **“Mint N NFTs to {to}”**; `boing_sendTransaction` returns **`{ tx_hash, tx_id }`** — poll `boing_getTransactionReceipt(tx_id)`.
- **v1 collections cannot be upgraded** in place; redeploy v2. **Hosted Fly 6913** still needs node redeploy for 3M call gas before large batches work on public RPC.
- Roadmap for marketplace / royalties: **F2** in [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md).

---

## boing.finance note

The **Deploy Token** page’s **EVM** path stays for MetaMask-style wallets. For **Boing L1 + Boing Express**, prefer **one form** (name/symbol/…) with **internal bytecode** (`resolveReferenceFungibleTemplateBytecodeHex` + **`buildContractDeployMetaTx`**, or **`resolveReferenceFungibleSecuredTemplateBytecodeHex`** / **`buildReferenceFungibleSecuredDeployMetaTx`** for the secured pinned default), and keep **paste bytecode** under **Advanced**. See [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md) § Handoff.

---

## References

- [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md)
- [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md)
- [boing-sdk README](../boing-sdk/README.md)
