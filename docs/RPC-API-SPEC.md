# Boing Network — JSON-RPC API Specification

> **Version:** 0.1  
> **Transport:** HTTP POST  
> **Encoding:** JSON-RPC 2.0  
> **References:** [RUNBOOK.md](RUNBOOK.md)

---

## Overview

Boing nodes expose a JSON-RPC HTTP interface for submitting transactions, querying chain state, and simulation. Rate limiting applies per `RateLimitConfig` (see [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md)).

### Base URL

```
http://<host>:<rpc_port>/
```

Default RPC port: `8545`.

### Request Format

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "boing_chainHeight",
  "params": []
}
```

### Response Format

**Success:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": 42
}
```

**Error:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params: expected [hex_signed_tx]"
  }
}
```

---

## Methods

### boing_submitTransaction

Submit a signed transaction to the mempool.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_signed_tx]` | Hex-encoded bincode-serialized SignedTransaction |

**Example:**
```json
{"jsonrpc":"2.0","id":1,"method":"boing_submitTransaction","params":["0x..."]}
```

---

### boing_chainHeight

Return the current chain height (tip block number).

| Field | Type | Description |
|-------|------|-------------|
| Params | `[]` | None |

**Result:** `u64`

---

### boing_getBalance

Get the spendable balance for an account. **Recommended for wallets** (e.g. boing.express) to display balance without deriving from state.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_account_id]` | 32-byte AccountId (hex) |
| Result | `{ balance: string }` | Native BOING balance as **whole units** (u128 decimal string). Wallets and explorers should **not** assume Ethereum-style `10^18` scaling unless a future token standard adds it. |

**Example:** `{"jsonrpc":"2.0","id":1,"method":"boing_getBalance","params":["0x..."]}` → `{"jsonrpc":"2.0","id":1,"result":{"balance":"1000000"}}`

---

### boing_getAccount

Get full account state (balance, nonce, stake). **Recommended for wallets** to build transactions (nonce) and show balance/stake.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_account_id]` | 32-byte AccountId (hex) |
| Result | `{ balance: string, nonce: number, stake: string }` | balance and stake are u128 as decimal strings; nonce is u64. If account does not exist, returns balance "0", nonce 0, stake "0". |

**Example:** `{"jsonrpc":"2.0","id":1,"method":"boing_getAccount","params":["0x..."]}` → `{"jsonrpc":"2.0","id":1,"result":{"balance":"1000000","nonce":5,"stake":"0"}}`

---

### boing_getBlockByHeight

Get a block by height.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[height]` | Block height (u64) |

**Result:** Block object or `null` if not found.

---

### boing_getBlockByHash

Get a block by hash.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_block_hash]` | 32-byte block hash (hex) |

**Result:** Block object or `null` if not found.

---

### boing_getAccountProof

Get a Merkle proof for an account.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_account_id]` | 32-byte AccountId (hex) |

**Result:** `{ proof: string, root: string, value_hash: string }`

---

### boing_verifyAccountProof

Verify an account Merkle proof.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_proof, hex_state_root]` | Proof and expected root (hex) |

**Result:** `{ valid: boolean }`

---

### boing_simulateTransaction

Simulate a transaction without applying it.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_signed_tx]` | Hex-encoded SignedTransaction |

**Result:** `{ gas_used: number, success: boolean, error?: string }`

---

### boing_registerDappMetrics

Register a dApp for incentive tracking.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_contract, hex_owner]` | Contract and owner AccountIds (hex) |

**Result:** `{ registered: true, contract: string, owner: string }`

---

### boing_submitIntent

Submit a signed intent for solver fulfillment.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_signed_intent]` | Hex-encoded SignedIntent |

**Result:** `{ intent_id: string }`

---

### boing_qaCheck (optional — when QA is enabled)

Pre-flight check for a deployment without submitting. Allows clients to see whether bytecode (and optional purpose declaration) would be **Allow**, **Reject**, or **Unsure** (pool) before calling `boing_submitTransaction`.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_bytecode]` or `[hex_bytecode, purpose_category, description_hash?]` | Contract bytecode (hex); optionally purpose category and description hash for purpose checks. |
| Result | `{ result, rule_id?, message? }` | `result`: `"allow"` (would be accepted), `"reject"` (would be rejected; rule_id and message when applicable), or `"unsure"` (would go to community QA pool). |

**Errors:** When QA is not enabled, returns `-32601` (method not found) or a dedicated code. When QA rejects: use structured error code below.

---

### boing_faucetRequest (testnet only)

Request testnet BOING for an account. Only available when the node is started with `--faucet-enable`. **Do not enable on mainnet.**

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_account_id]` | 32-byte account ID (hex). Recipient of the faucet transfer. |

**Result:** `{ ok: true, amount: number, to: string, message: string }`

**Rate limit:** 1 request per 60 seconds per account ID. Returns `-32016` with message "Faucet cooldown" if called too soon.

**Errors:** `-32601` Faucet not enabled; `-32000` Faucet account not initialized or balance too low.

---

## Error Codes

| Code | Meaning |
|------|---------|
| -32600 | Invalid Request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32000 | Server error |
| -32016 | Rate limit exceeded |
| -32050 | **QA: Deployment rejected** — Transaction rejected by protocol QA (e.g. bytecode or purpose rule). Response SHOULD include `data: { rule_id: string, message: string }` for structured feedback. See [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md). |
| -32051 | **QA: Pending pool** — Deployment referred to community QA pool (result: Unsure). Response MAY include `data: { pending_id: string, deadline: number }`. |

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
