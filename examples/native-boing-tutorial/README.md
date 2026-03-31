# Native Boing tutorial (SDK — no ethers)

End-to-end examples for **Boing L1** using only **`boing-sdk`**: transfer, reference-token `contract_call`, and a minimal `contract_deploy` with QA. For **browser + Boing Express**, use the same RPC helpers and `window.boing.request({ method: 'boing_sendTransaction', params: [...] })` per [BOING-EXPRESS-WALLET.md](../../docs/BOING-EXPRESS-WALLET.md).

## Prereqs

1. A running Boing node RPC (e.g. `http://127.0.0.1:8545`).
2. Funded account: 32-byte Ed25519 **secret** as hex (`0x` + 64 hex chars). Testnet: `boing_faucetRequest` via SDK or site faucet.
3. Build the SDK:

```bash
cd ../../boing-sdk && npm install && npm run build
```

4. Install this package:

```bash
cd examples/native-boing-tutorial && npm install
```

## Scripts

### 1. Transfer (`npm run transfer`)

Env:

| Variable | Required | Description |
|----------|----------|-------------|
| `BOING_RPC_URL` | no | Default `http://127.0.0.1:8545` |
| `BOING_SECRET_HEX` | **yes** | `0x` + 64 hex (32-byte signing seed) |
| `BOING_TO_HEX` | **yes** | Recipient 32-byte account id |
| `BOING_AMOUNT` | no | Default `1` (integer string → `bigint`) |

Uses **`submitTransferWithSimulationRetry`** (simulate → merge access list if needed → submit).

### 2. Reference token call (`npm run contract-call`)

Requires a **deployed** reference-token contract.

| Variable | Required |
|----------|----------|
| `BOING_SECRET_HEX` | yes |
| `BOING_CONTRACT_HEX` | yes (32-byte contract account) |
| `BOING_RPC_URL` | no |

Sends reference **`transfer(to, amount)`** calldata ([BOING-REFERENCE-TOKEN.md](../../docs/BOING-REFERENCE-TOKEN.md)). Set `BOING_TRANSFER_TO_HEX` and `BOING_TRANSFER_AMOUNT` (default `1`).

### 3. Minimal deploy (`npm run deploy-minimal`)

Deploys tiny bytecode (`RETURN 1` style) with purpose category **`tooling`** (override with `BOING_PURPOSE`).

| Variable | Required |
|----------|----------|
| `BOING_SECRET_HEX` | yes |
| `BOING_DEPLOY_BYTECODE_HEX` | no — defaults to minimal runtime |

Runs **`boing_qaCheck`** first; on mempool **`QA reject`** or **pool** responses, prints `explainBoingRpcError`.

## Browser + Express

Node cannot hold the user’s extension key. In a page:

1. `boing_requestAccounts` → `senderHex`
2. Build a **tx object** expected by Express (`boing_sendTransaction`) for `transfer` / `contract_call` / purpose deploy — see wallet `dappTxRequest` types.
3. On errors, map JSON-RPC `code` using [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](../../docs/BOING-RPC-ERROR-CODES-FOR-DAPPS.md).

## Docs

- [BOING-DAPP-INTEGRATION.md](../../docs/BOING-DAPP-INTEGRATION.md)
- [BOING-SIGNED-TRANSACTION-ENCODING.md](../../docs/BOING-SIGNED-TRANSACTION-ENCODING.md)
- [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](../../docs/BOING-VM-CAPABILITY-PARITY-ROADMAP.md) (E1)
