# dApp integration — Boing native path

Short checklist for web apps that want **Boing L1** behavior without assuming a foreign L1 wallet stack (20-byte addresses + secp256k1 signing).

---

## 1. Discover the provider

- Prefer `window.boing` or EIP-6963 providers whose name/rdns contains Boing ([BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)).
- `eth_requestAccounts` / `boing_requestAccounts` return **32-byte** account ids (`0x` + 64 hex chars). **`boing-sdk`:** **`isBoingNativeAccountIdHex(addr)`** returns true only for valid 32-byte ids — use in multi-wallet UIs to pick the **native Boing** deploy path vs **20-byte** EVM addresses.

---

## 2. Chain alignment

- Read `boing_chainId` (fallback `eth_chainId`). Boing testnet uses `0x1b01` (6913 decimal) in docs.
- For **HTTP JSON-RPC** (servers, scripts, headless dApps), call **`boing_getNetworkInfo`** — **`boing-sdk`** **`getNetworkInfo()`** — for optional numeric **`chain_id`** / **`chain_name`** (when the node sets **`BOING_CHAIN_ID`** / **`BOING_CHAIN_NAME`**), target block time, tip height/hash, and **`client_version`**. The payload includes **`rpc.not_available`** listing **chain-wide total stake** and **staking APY** as **not** provided on public RPC; use protocol metrics or indexers if you need them ([RPC-API-SPEC.md](RPC-API-SPEC.md) § **boing_getNetworkInfo**).
- Call `boing_switchChain` when the user must be on a specific network.

---

## 3. Read state

- Use JSON-RPC (`boing_getAccount`, `boing_getBalance`, blocks, proofs) via **`boing-sdk`** `createClient(url)`.
- Do **not** assume generic “browser provider + `getSigner()`” helpers built for 20-byte accounts work for native Boing `AccountId`s.

---

## 4. Simulate before send

- Build a **unsigned** transaction shape the wallet understands, or hex-encode a `SignedTransaction` with a placeholder signature only if your stack requires it — the supported path is:
  - **`boing_simulateTransaction`** with a valid signed tx hex from the wallet or SDK, **or**
  - Wallet **`boing_sendTransaction`** which signs and simulates internally ([BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)).
- Use simulation’s `suggested_access_list` and merge with `mergeAccessListWithSimulation` / `accessListFromSimulation` in `boing-sdk` when building txs yourself.
- For **reference-style `ContractCall` calldata** (selector low byte + 32-byte words — Boing-defined, not Solidity ABI), `boing-sdk` exposes **`encodeBoingCallTyped`**, **`encodeBoingCallFromAbiArgs`**, and **`BoingReferenceCallDescriptors`** / **`encodeBoingCallFromDescriptor`** in `callAbi.ts` (same on-wire bytes as `referenceToken.ts`, `referenceNft.ts`, and `nativeAmm.ts` helpers).

### Native constant-product swap (Boing VM)

For **chain 6913** swaps against the **in-ledger** constant-product pool (not a foreign-chain router/factory deployment):

- **Accounts are 32-byte** Boing `AccountId`s (`0x` + 64 hex). Do not treat them as 20-byte contract addresses or feed them into tooling that assumes that shape.
- **Pool id** is **not** fixed in the RPC layer — configure the deployed pool address in your app (see [RPC-API-SPEC.md](RPC-API-SPEC.md) § Native constant-product AMM). **Public testnet** canonical pool: **`0xffaa1290614441902ba813bf3bd8bf057624e0bd4f16160a9d32cd65d3f4d0c2`** ([TESTNET.md](TESTNET.md) §5.3). Optional: **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** from **`boing-sdk`**. Pool rotations: [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md).
- Build **`contract_call`** with **`calldata`** per [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) and an explicit **`access_list`** (`read` / `write`: signer + pool for the current MVP bytecode).
- **Pre-flight:** either rely on **`boing_sendTransaction`** (wallet may simulate internally), or follow **boing.finance** / **`boing-sdk`** pattern: **`boing_signTransaction`** → **`boing_simulateTransaction`**; if **`access_list_covers_suggestion`** is `false`, merge **`suggested_access_list`** and **sign again** (users may see a second approval in the extension).
- **Reserves:** read pool storage via **`boing_getContractStorage`** using keys from the native AMM spec (`boing-sdk`: **`fetchNativeConstantProductReserves`**, **`NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX`**, **`NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX`**), or your indexer once the pool emits logs.
- **Regression reference:** `cargo test -p boing-node --test native_amm_rpc_happy_path` (deploy → add liquidity → swap over JSON-RPC).
- **Browser smoke:** [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md) (Boing Express + boing.finance).

---

## 5. Submit transactions

| Context | Recommended API |
|---------|-----------------|
| Browser + Boing Express | `provider.request({ method: 'boing_sendTransaction', params: [txObject] })` |
| Node / backend / tests | `boing-sdk`: `submitTransferWithSimulationRetry` / `submitContractCallWithSimulationRetry` / `submitDeployWithPurposeFlow`, or manual `signTransactionInput` → `submitTransaction(hex)` |
| Hardware / external signer | Build bincode per [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md), sign Ed25519 over `signableTransactionHash` |

**Tutorial package (SDK + node only):** [examples/native-boing-tutorial](../examples/native-boing-tutorial/).

Contract deploys from dApps must use **purpose-bearing** deploy types (`contract_deploy_purpose` / `contract_deploy_meta`); bare `contract_deploy` is rejected from injection so QA declarations are explicit.

**`contract_deploy_meta` convenience:** Boing Express treats **`purpose_category` as optional** when **`asset_name` and/or `asset_symbol`** are set — it defaults to **`token`**. You still need an explicit `purpose_category` for metadata-free deploys (use `contract_deploy_purpose`) or when the category must differ (e.g. tooling with a display name). Other wallets may require the field; set it explicitly for maximum compatibility.

### Form parity with EVM deploy UIs

To avoid asking end users for **raw bytecode**, pin **ops-approved** native programs (fungible + NFT) and only expose **name / symbol / supply** (and similar) in the main flow. Use **`boing-sdk`** **`buildContractDeployMetaTx`** and **`resolveReferenceFungibleTemplateBytecodeHex`** (embedded default; optional env override `BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX`, `VITE_…`, `REACT_APP_…`). Full checklist: [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md).

### One wizard, two backends (EVM vs native Boing)

Multi-chain launchers (same steps: **basics → network → review → deploy**) should **branch only at submit**:

| Wizard step | EVM (MetaMask-style) | Native Boing (Boing Express) |
|-------------|----------------------|------------------------------|
| **Network** | `eth_chainId` / CAIP id | `boing_chainId` or `eth_chainId`; **`isBoingTestnetChainId(...)`** in **`boing-sdk`** (`chainIds.ts`) matches **6913** / **`0x1b01`** ([TESTNET.md](TESTNET.md)). Prefer **`boing_getNetworkInfo`**.`chain_id` when you already use JSON-RPC. |
| **Token form** | Name, symbol, supply → factory/constructor | **Same visible fields** (no supply in pinned template’s first tx — admin **`mint_first`** after deploy per [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)). |
| **Build deploy payload** | ABI-encoded constructor + factory | **One call:** **`buildReferenceFungibleDeployMetaTx({ assetName, assetSymbol })`** — bundles **`resolveReferenceFungibleTemplateBytecodeHex`** + **`buildContractDeployMetaTx`**. NFT collections: **`buildReferenceNftCollectionDeployMetaTx`** (requires env or **`bytecodeHexOverride`**). |
| **Review / QA** | Simulation / gas | **`preflightContractDeployMetaWithUi(client, tx)`** or **`buildAndPreflightReferenceFungibleDeploy(client, { assetName, assetSymbol })`** — runs **`boing_qaCheck`** and returns **`{ qa, ui }`** with **`ui.headline`**, **`ui.detail`**, **`ui.readyToSign`**, **`ui.tone`** for banners and primary-button state. Lower-level: **`preflightContractDeployMetaQa`** + **`describeContractDeployMetaQaResponse(qa)`**. Handle **`unsure`** like pool acknowledgement ([QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)). |
| **Submit** | `eth_sendTransaction` | **`boing_sendTransaction`** with the **`contract_deploy_meta`** object (no bincode in the browser). |

**Minimal browser sequence (fungible):**

```ts
import {
  buildAndPreflightReferenceFungibleDeploy,
  createClient,
} from 'boing-sdk';

const client = createClient(rpcUrl);
const { tx, ui } = await buildAndPreflightReferenceFungibleDeploy(client, {
  assetName: values.name,
  assetSymbol: values.symbol,
});

if (!ui.readyToSign) {
  // reject: show ui.headline + ui.detail (includes rule / message when present)
  return;
}
if (ui.tone === 'warning') {
  // unsure: explain community QA pool (ui.detail) before sign
}

await provider.request({ method: 'boing_sendTransaction', params: [tx] });
```

**Equivalent (manual build + preflight):** `buildReferenceFungibleDeployMetaTx` → `preflightContractDeployMetaWithUi(client, tx)` or `preflightContractDeployMetaQa` + `describeContractDeployMetaQaResponse`.

Keep **paste bytecode** / **description_hash** under **Advanced** only — same mental model as EVM “custom bytecode” toggles.

---

## 6. Auth / “login”

- Use **`boing_signMessage`** (or `personal_sign` alias) with the portal/dApp message format; backend verifies Ed25519 + BLAKE3(message) per portal docs.

---

## 7. Errors and UX

- **Locked wallet:** extension should return a clear error; dApp should prompt “Unlock Boing Express”.
- **User reject:** treat like any wallet rejection; do not show “wrong network” for a signing refusal.
- **QA / mempool / pool:** use numeric JSON-RPC `code` and structured `data` as in [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md). In TypeScript, `explainBoingRpcError(e)` from `boing-sdk` gives short user-facing text.

---

## 8. Optional: EIP-6963 capability hints (track W3)

[EIP-6963](https://eips.ethereum.org/EIPS/eip-6963) `info` objects today include `uuid`, `name`, `rdns`, `icon`. Wallets **may** extend discovery with a **namespaced** vendor object (see the concrete `boing` example in [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md#eip-6963-optional-multi-wallet-discovery)). Until that is widely adopted, probe with `boing_chainHeight` or read wallet docs. Prefer **not** calling signing RPCs with dummy payloads to detect support.

---

## References

- [boing-sdk README](../boing-sdk/README.md) — RPC client, `callAbi.ts`, calldata helpers, submit flows  
- [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) — Phase 1 tracks P / W / E  
- [RPC-API-SPEC.md](RPC-API-SPEC.md)  
- [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)  
- [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md)  
