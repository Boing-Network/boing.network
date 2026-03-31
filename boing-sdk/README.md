# Boing SDK

TypeScript/JavaScript client for [Boing Network](https://github.com/chiku524/boing.network): typed RPC client, hex utilities, and structured errors (including QA rejection feedback).

## Install

```bash
npm install
npm run build
```

Or from a parent repo (when published): `npm install boing-sdk`.

## Quick start

```ts
import { createClient, BoingRpcError } from 'boing-sdk';

const client = createClient('http://localhost:8545');

// Read chain and account state
const height = await client.chainHeight();
const account = await client.getAccount('0x' + '00'.repeat(32)); // 32-byte hex
console.log(account.balance, account.nonce, account.stake);

// Pre-flight QA check before deploying a contract
const qa = await client.qaCheck('0x600160005260206000f3'); // hex bytecode
if (qa.result === 'reject') {
  console.error('QA rejected:', qa.rule_id, qa.message);
} else if (qa.result === 'allow') {
  // Submit signed tx (hex from Rust CLI or future signer)
  await client.submitTransaction(hexSignedTx);
}

// Handle structured QA errors on submit
try {
  await client.submitTransaction(hexSignedTx);
} catch (e) {
  if (e instanceof BoingRpcError && e.isQaRejected) {
    const { rule_id, message } = e.qaData ?? {};
    console.error('Deployment rejected:', rule_id, message);
  }
  throw e;
}
```

## API

- **createClient(config)** — `config` can be a URL string or `{ baseUrl, fetch?, timeoutMs? }`. Default timeout 30s; set `timeoutMs: 0` to disable.
- **BoingClient** — typed methods for all RPCs (32-byte account/hash params are validated locally before sending):
  - `chainHeight()`, `getBalance(hexAccountId)`, `getAccount(hexAccountId)`
  - `getBlockByHeight(height, includeReceipts?)`, `getBlockByHash(hexHash, includeReceipts?)`
  - `getTransactionReceipt(hexTxId)`, `getLogs(filter)` — bounded log query ([RPC-API-SPEC.md](../docs/RPC-API-SPEC.md))
  - Receipt/log helpers: `normalizeTopicWord`, `iterBlockReceiptLogs`, `filterReceiptLogsByTopic0`, … (`receiptLogs.ts`; see [INDEXER-RECEIPT-AND-LOG-INGESTION.md](../docs/INDEXER-RECEIPT-AND-LOG-INGESTION.md))
  - `getAccountProof(hexAccountId)`, `verifyAccountProof(hexProof, hexStateRoot)`
  - `simulateTransaction(hexSignedTx)`, `submitTransaction(hexSignedTx)`
  - High-level flows: `submitTransferWithSimulationRetry`, `submitContractCallWithSimulationRetry`, `submitDeployWithPurposeFlow` (see **Simulate → access list → submit** below)
  - `registerDappMetrics(hexContract, hexOwner)`, `submitIntent(hexSignedIntent)`
  - `qaCheck(hexBytecode, purposeCategory?, descriptionHash?, assetName?, assetSymbol?)` — pre-flight QA without submitting (same param order as node `boing_qaCheck`)
  - `qaPoolList()`, `qaPoolConfig()`, `qaPoolVote(txHashHex, voterHex, vote)` — governance QA pool for Unsure deploys
  - `faucetRequest(hexAccountId)` — testnet only
- **BoingRpcError** — `code`, `message`, `data`, `method`; `isQaRejected`, `isQaPendingPool`, `pendingPoolTxHash`, `isQaPoolDisabled`, `isQaPoolFull`, `isQaPoolDeployerCap`, `qaData`; `toString()` for logging.
- **Hex helpers** — `ensureHex`, `bytesToHex`, `hexToBytes`, `accountIdToHex`, `hexToAccountId`, `validateHex32` (normalize + require 32 bytes).

All 32-byte IDs (account, hash) are hex strings with or without `0x` prefix. Invalid hex or wrong length throws before the request.

## Submitting transactions

The node expects **hex-encoded bincode-serialized `SignedTransaction`**. Encoding matches Rust `boing-primitives` (bincode 1.3); see [BOING-SIGNED-TRANSACTION-ENCODING.md](../docs/BOING-SIGNED-TRANSACTION-ENCODING.md).

**Options:**

1. **TypeScript (Node or bundler)** — build + sign with a 32-byte Ed25519 secret key:

```ts
import {
  createClient,
  fetchNextNonce,
  buildTransferTransaction,
  signTransactionInput,
} from 'boing-sdk';

const client = createClient('http://localhost:8545');
const senderHex = '0x' + '<64-hex of public key>';
const secret32 = new Uint8Array(32); // your signing seed / secret key bytes
const nonce = await fetchNextNonce(client, senderHex);
const tx = buildTransferTransaction({
  nonce,
  senderHex,
  toHex: '0x' + '<64-hex recipient>',
  amount: 1n,
});
const signedHex = await signTransactionInput(tx, secret32);
await client.simulateTransaction(signedHex);
await client.submitTransaction(signedHex);
```

2. **Injected wallet** — `boing_sendTransaction` / `boing_signTransaction` in **Boing Express** ([BOING-EXPRESS-WALLET.md](../docs/BOING-EXPRESS-WALLET.md)).

3. **Rust CLI** — `cargo run -p boing-cli -- …` for local dev.

4. **Custom signer** — `signTransactionInputWithSigner(tx, async (hash) => …)` (must return 64-byte Ed25519 signature over `hash`).

**Protocol QA:** Contract deploys are checked in the mempool (`boing_qa`). Use `qaCheck` before submit and purpose-bearing deploy types from the wallet. See [QUALITY-ASSURANCE-NETWORK.md](../docs/QUALITY-ASSURANCE-NETWORK.md).

**dApp checklist:** [BOING-DAPP-INTEGRATION.md](../docs/BOING-DAPP-INTEGRATION.md).

See [RPC-API-SPEC.md](../docs/RPC-API-SPEC.md), [BUILD-ROADMAP.md](../docs/BUILD-ROADMAP.md), and [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](../docs/BOING-VM-CAPABILITY-PARITY-ROADMAP.md).

## Simulate → access list → submit (P4)

Use **`submitTransferWithSimulationRetry`**, **`submitContractCallWithSimulationRetry`**, or **`submitDeployWithPurposeFlow`** so the node can widen `access_list` when `boing_simulateTransaction` returns `access_list_covers_suggestion: false` (merges `suggested_access_list` via `mergeAccessListWithSimulation`).

```ts
import {
  createClient,
  submitTransferWithSimulationRetry,
  explainBoingRpcError,
  senderHexFromSecretKey,
  hexToBytes,
} from 'boing-sdk';

const client = createClient(process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545');
const secret = hexToBytes(process.env.BOING_SECRET_HEX!); // 0x + 64 hex
const senderHex = await senderHexFromSecretKey(secret);

try {
  const { tx_hash, lastSimulation, attempts } = await submitTransferWithSimulationRetry({
    client,
    secretKey32: secret,
    senderHex,
    toHex: process.env.BOING_TO_HEX!,
    amount: 10n,
  });
  console.log({ tx_hash, gas: lastSimulation.gas_used, attempts });
} catch (e) {
  console.error(explainBoingRpcError(e));
}
```

Deploy path runs **`boing_qaCheck`** first; **`reject`** throws `BoingRpcError` with `isQaRejected`. On **`submitTransaction`**, catch **`BoingRpcError`** for mempool QA (**-32050**), pool (**-32051**), and pool caps (**-32054..-32056**). Code reference: [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](../docs/BOING-RPC-ERROR-CODES-FOR-DAPPS.md).

**Canonical scripts:** [examples/native-boing-tutorial](../examples/native-boing-tutorial/) (`transfer`, `contract-call`, `deploy-minimal`).

## API additions (transaction flows)

- `senderHexFromSecretKey(secret32)` — derive `AccountId` hex from signing seed.
- `submitTransferWithSimulationRetry`, `submitContractCallWithSimulationRetry`, `submitDeployWithPurposeFlow` — see `submitFlow.ts`.
- `SimulationFailedError` — simulation failed without an access-list retry path.
- `explainBoingRpcError(e)` — human-readable string for `BoingRpcError` and QA pool codes.

## Tests

```bash
cd boing-sdk && npm install && npm test
```

Golden vectors are tied to `cargo run -p boing-primitives --example dump_bincode`.

## Planned

- CLI auto-completion, contract templates. See [DEVELOPMENT-AND-ENHANCEMENTS.md](../docs/DEVELOPMENT-AND-ENHANCEMENTS.md).
