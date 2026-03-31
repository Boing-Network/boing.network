# E2 — Partner apps: native Boing deploy / call (no ethers)

**Roadmap:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) track **E2**.

This is the **canonical pattern** for apps (e.g. **boing.finance**) that already support **EVM** flows but need a **native Boing** path when users connect **Boing Express** with a **32-byte AccountId**.

---

## Why a separate path exists

- **ethers.js `BrowserProvider.getSigner()`** expects **20-byte Ethereum addresses** and EVM signing.
- Boing Express exposes **Ed25519** accounts and **`boing_sendTransaction`** / **`boing_signTransaction`** for **Boing VM** payloads ([BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)).

---

## Recommended integration shape

### Browser (Boing Express injected)

1. Detect **native account**: `0x` + **64** hex chars ([`isBoingNativeAccountIdHex`](BOING-DAPP-INTEGRATION.md) / `boing-sdk` pattern).
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

- **Not** ERC-20 bytecode: implement or reuse a **Boing VM** contract that follows the **reference token** calldata layout for interoperability.
- Pre-flight **`boing_qaCheck`** with category **`token`** when appropriate.
- UI copy should distinguish **“ERC-20 (EVM)”** vs **“Native Boing token (VM)”** to avoid user confusion.

---

## boing.finance note

The **Deploy Token** page’s **EVM** path remains for MetaMask / Sepolia / other EVM chains. When the user is on **native Boing**, show a **CTA** that links here and/or **Boing Native VM** in-product, until a full in-app **`boing_sendTransaction`** flow ships.

---

## References

- [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md)
- [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md)
- [boing-sdk README](../boing-sdk/README.md)
