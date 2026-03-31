# dApp integration — Boing native path

Short checklist for web apps that want **Boing L1** behavior without assuming EVM (`ethers` / MetaMask signing).

---

## 1. Discover the provider

- Prefer `window.boing` or EIP-6963 providers whose name/rdns contains Boing ([BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)).
- `eth_requestAccounts` / `boing_requestAccounts` return **32-byte** account ids (`0x` + 64 hex chars).

---

## 2. Chain alignment

- Read `boing_chainId` (fallback `eth_chainId`). Boing testnet uses `0x1b01` (6913 decimal) in docs.
- Call `boing_switchChain` when the user must be on a specific network.

---

## 3. Read state

- Use JSON-RPC (`boing_getAccount`, `boing_getBalance`, blocks, proofs) via **`boing-sdk`** `createClient(url)`.
- Do **not** assume `provider.getSigner()` from ethers works for native Boing accounts.

---

## 4. Simulate before send

- Build a **unsigned** transaction shape the wallet understands, or hex-encode a `SignedTransaction` with a placeholder signature only if your stack requires it — the supported path is:
  - **`boing_simulateTransaction`** with a valid signed tx hex from the wallet or SDK, **or**
  - Wallet **`boing_sendTransaction`** which signs and simulates internally ([BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)).
- Use simulation’s `suggested_access_list` and merge with `mergeAccessListWithSimulation` / `accessListFromSimulation` in `boing-sdk` when building txs yourself.

### Native constant-product swap (Boing VM)

For **chain 6913** swaps against the **in-ledger** constant-product pool (not EVM router contracts):

- **Accounts are 32-byte** Boing `AccountId`s (`0x` + 64 hex). Do not use `ethers.Contract` against these ids unless you have an explicit EVM compatibility layer.
- **Pool id** is **not** fixed in the RPC layer — configure the deployed pool address in your app (see [RPC-API-SPEC.md](RPC-API-SPEC.md) § Native constant-product AMM). When the network publishes a canonical testnet pool, copy that hex into env / config.
- Build **`contract_call`** with **`calldata`** per [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) and an explicit **`access_list`** (`read` / `write`: signer + pool for the current MVP bytecode).
- **Pre-flight:** either rely on **`boing_sendTransaction`** (wallet may simulate internally), or follow **boing.finance** / **`boing-sdk`** pattern: **`boing_signTransaction`** → **`boing_simulateTransaction`**; if **`access_list_covers_suggestion`** is `false`, merge **`suggested_access_list`** and **sign again** (users may see a second approval in the extension).
- **Reserves:** read pool storage via **`boing_getContractStorage`** using keys from the native AMM spec, or your indexer once the pool emits logs.
- **Regression reference:** `cargo test -p boing-node --test native_amm_rpc_happy_path` (deploy → add liquidity → swap over JSON-RPC).

---

## 5. Submit transactions

| Context | Recommended API |
|---------|-----------------|
| Browser + Boing Express | `provider.request({ method: 'boing_sendTransaction', params: [txObject] })` |
| Node / backend / tests | `boing-sdk`: `submitTransferWithSimulationRetry` / `submitContractCallWithSimulationRetry` / `submitDeployWithPurposeFlow`, or manual `signTransactionInput` → `submitTransaction(hex)` |
| Hardware / external signer | Build bincode per [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md), sign Ed25519 over `signableTransactionHash` |

**Tutorial package (no ethers):** [examples/native-boing-tutorial](../examples/native-boing-tutorial/).

Contract deploys from dApps must use **purpose-bearing** deploy types (`contract_deploy_purpose` / `contract_deploy_meta`); bare `contract_deploy` is rejected from injection so QA declarations are explicit.

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

- [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) — Phase 1 tracks P / W / E  
- [RPC-API-SPEC.md](RPC-API-SPEC.md)  
- [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)  
- [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md)  
