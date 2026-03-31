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

### Chain tip, committed height, and finality

Boing uses a **HotStuff-style BFT** consensus layer ([`boing-consensus`](../crates/boing-consensus)): a block is appended to this node’s [`ChainState`](../crates/boing-node/src/chain.rs) only after it is **committed** through that path. There is **no separate JSON-RPC “unsafe head”** ahead of commit in the current node implementation.

| Term | Meaning in this codebase |
|------|-------------------------|
| **`boing_chainHeight` / `head_height` in `boing_getSyncState`** | Height of the latest **committed** block stored on this node (the chain tip). |
| **Finalized (protocol sense)** | Under standard BFT assumptions (honest quorum, etc.), a committed block is not reverted by consensus. |
| **`finalized_height` in `boing_getSyncState`** | Today **equal to** `head_height`, because the node only exposes committed blocks. Reserved so a future release can report lag (e.g. optimistic execution vs commit) without breaking clients. |
| **Syncing / lagging peers** | A node that is still catching up may report a **lower** height than the rest of the network until it imports commits. Clients should not treat “height stopped increasing” as finality across the whole network without comparing multiple sources. |

Wallets and observers that need a single number for “how deep is my tx” can use **`boing_chainHeight`** or **`boing_getSyncState`**; until multiple heights diverge in a future version, they are equivalent for “committed tip.”

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

**QA pool (Unsure):** If mempool QA returns Unsure **and** governance allows the pool to accept work (`qa_pool_config`: non-zero `max_pending_items` and either `administrators` or `dev_open_voting`), the node enqueues the deployment and responds with **`-32051`** and `data: { "tx_hash": "0x..." }`. **Governance-listed administrators** vote via `boing_qaPoolVote`. Hard caps (`max_pending_items`, `max_pending_per_deployer`) prevent pool congestion; when full, **`-32055`** / **`-32056`** apply instead of enqueueing.

**Operator RPC (optional):** When the node process has environment variable **`BOING_OPERATOR_RPC_TOKEN`** set to a non-empty string, **`boing_qaPoolVote`** and **`boing_operatorApplyQaPolicy`** require HTTP header **`X-Boing-Operator: <same token>`**. If the variable is unset, behavior matches earlier releases (no header check). Use this on any RPC endpoint reachable from untrusted networks so pool votes cannot be triggered by spoofing an admin hex alone.

**Does the pool need RPC to “run”?** No. The node **owns** the pool: when QA returns Unsure and governance allows it, enqueueing happens inside normal transaction/mempool handling (`boing_submitTransaction` may return **`-32051`**). No operator client is required for items to enter the queue or for the node to age them out per config. JSON-RPC is how **operators** *inspect and change* the pool—**`boing_qaPoolList`**, **`boing_qaPoolConfig`**, **`boing_qaPoolVote`**, **`boing_operatorApplyQaPolicy`**. For routine governance work, the **Boing Network desktop hub** (QA operator view) calls those methods over HTTP, so a terminal or **`boing` CLI** is optional (CLI remains useful for scripts and file-based `boing qa apply`).

---

### boing_qaPoolList

List pending items in the community QA pool (same `tx_hash` keys as `-32051`).

| Field | Type | Description |
|-------|------|-------------|
| Params | `[]` | None |
| Result | `{ items: [...] }` | Each item: `tx_hash`, `bytecode_hash`, `deployer` (hex), `allow_votes`, `reject_votes`, `age_secs`. |

---

### boing_qaPoolVote

Cast a vote on a pending pool item. When quorum and allow/reject thresholds are met, the item is resolved; on **Allow**, the stored signed transaction is inserted into the mempool.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[tx_hash_hex, voter_hex, vote]` | `vote` is `allow`, `reject`, or `abstain` (case-insensitive). Only accounts listed in governance `qa_pool_config.administrators` may vote, unless `dev_open_voting` is true with an empty admin list (local dev). |
| Result | `{ outcome: "pending" \| "reject" \| "allow", mempool?: boolean, duplicate?: boolean, error?: string }` | On `allow` with `mempool: true`, the tx is in the mempool. |

**Errors:** `-32052` no pending item for `tx_hash`; `-32053` voter is not a governance QA administrator; **`-32057`** operator authentication required (see **Operator RPC** above).

---

### boing_operatorApplyQaPolicy

Replace the node’s in-memory QA registry and pool governance config (same effect as loading `qa_registry.json` / `qa_pool_config.json` at startup, plus persistence to the node data directory). Intended for operators; requires **`X-Boing-Operator`** when **`BOING_OPERATOR_RPC_TOKEN`** is set.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[qa_registry_json, qa_pool_config_json]` | Two **strings**, each the full JSON document (not a path). |
| Result | `{ ok: true }` | Policy applied. |

**Errors:** **`-32057`** missing or wrong operator header when the token is configured; **`-32602`** invalid JSON or schema.

**CLI:** `boing qa apply --registry <path> --pool <path> [--operator-token …]` (also reads `BOING_OPERATOR_RPC_TOKEN`).

---

### boing_qaPoolConfig

Read effective QA pool governance parameters and current queue depth (no params).

| Field | Type | Description |
|-------|------|-------------|
| Params | `[]` | None |
| Result | object | `max_pending_items`, `max_pending_per_deployer`, `review_window_secs`, quorum/threshold fractions, `default_on_expiry`, `dev_open_voting`, `administrator_count`, `accepts_new_pending`, `pending_count`. |

---

### boing_getQaRegistry

Return the **effective protocol QA rule registry** the node uses for deployment checks (read-only, no authentication). Same JSON shape as on-disk **`qa_registry.json`** and as the first argument to **`boing_operatorApplyQaPolicy`**.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[]` | None |
| Result | object | `max_bytecode_size`, `blocklist` (array of 32-byte arrays), `scam_patterns`, `always_review_categories` (array of strings), `content_blocklist`. |

**Reference:** Canonical baseline JSON for comparison lives in the monorepo at **`docs/config/qa_registry.canonical.json`** — see **`docs/config/CANONICAL-QA-REGISTRY.md`**. Live nodes may differ after governance updates.

---

### boing_chainHeight

Return the height of the latest **committed** block on this node (see [Chain tip, committed height, and finality](#chain-tip-committed-height-and-finality)).

| Field | Type | Description |
|-------|------|-------------|
| Params | `[]` | None |

**Result:** `u64`

---

### boing_getSyncState

Structured view of the node’s committed chain tip for clients that want explicit **head** vs **finalized** fields (Solana-/Ethereum-style clarity). Today both heights are identical.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[]` | None |

**Result:**

```json
{
  "head_height": 42,
  "finalized_height": 42,
  "latest_block_hash": "0x..."
}
```

- **`head_height`:** Same value as `boing_chainHeight`.
- **`finalized_height`:** Same as `head_height` in the current implementation; may diverge in a future release if the node exposes pre-commit or optimistic data.
- **`latest_block_hash`:** BLAKE3 hash of the tip block’s header (`Block::hash()`), 32-byte hex with `0x` prefix.

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
| Params | `[height]` or `[height, include_receipts]` | Block height (u64); optional boolean — when `true`, adds `receipts` array (same order as `transactions`, `null` if no receipt cached). |

**Result:** Block object or `null` if not found. `header` includes `receipts_root` (hex-encoded 32-byte Merkle root over execution receipts; see protocol spec). Optional `hash` field (hex) is added for convenience. With `include_receipts: true`, each entry matches **`boing_getTransactionReceipt`** shape or `null`.

---

### boing_getTransactionReceipt

Lookup execution outcome for an included transaction (same id as `Transaction::id()` / mempool `tx_hash`).

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_tx_id]` | 32-byte transaction id (hex) |

**Result:** Receipt object or `null` if unknown (e.g. not yet included, or node predates receipt persistence).

Receipt: `{ tx_id, block_height, tx_index, success, gas_used, return_data, logs, error }` — `return_data` is hex; `logs` is an array of `{ topics: string[], data: string }` (each topic is `0x` + 32-byte hex, `data` is hex); `error` is set when `success` is false.

---

### boing_getLogs

Query execution logs across a **bounded** range of committed blocks. Intended for indexers and wallets; nodes enforce limits to keep scans cheap.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[filter]` | Single object (camelCase keys below). |
| `fromBlock` | number or string | Start height (inclusive). JSON number or decimal / `0x` hex string. |
| `toBlock` | number or string | End height (inclusive). Must be `>= fromBlock`. |
| `address` | string (optional) | 32-byte `AccountId` hex. When set, only logs attributed to that contract are returned (`ContractCall` target, or deploy-derived address for deploy txs that carry logs). |
| `topics` | array (optional) | Up to 4 entries; each is `null` (wildcard) or a 32-byte topic hex string. Same index semantics as Ethereum `eth_getLogs` (positional match). |

**Limits (reference implementation):** inclusive block span at most **128**; at most **2048** log entries per call. Exceeding span returns JSON-RPC error `-32602`; exceeding the result cap returns `-32603`.

**Result:** JSON array of objects:

`{ block_height, tx_index, tx_id, log_index, address, topics, data }`

- `address` is hex or `null` when the node cannot attribute an emitting contract.
- Order is block height ascending, then transaction order in the block, then `log_index`.

**Example:** `{"jsonrpc":"2.0","id":1,"method":"boing_getLogs","params":[{"fromBlock":1,"toBlock":10,"topics":[null,"0x000…"]}]}`

---

### boing_getBlockByHash

Get a block by hash.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_block_hash]` or `[hex_block_hash, include_receipts]` | Optional boolean — same as `boing_getBlockByHeight`. |

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

### boing_getContractStorage

Read a single 32-byte Boing VM storage word for a contract (same semantics as `SLOAD`: missing slot → zero word). Useful for indexers and wallets that know the storage key layout (e.g. reference NFT / token conventions).

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_contract_id, hex_storage_key]` | Two 32-byte values (hex) |

**Result:** `{ value: string }` — `value` is `0x` + 64 hex chars (32 bytes).

---

### Native constant-product AMM (chain 6913 — integration note)

The JSON-RPC surface does **not** embed a canonical pool address. Wallets and dApps configure the **32-byte pool `AccountId`** (e.g. boing.finance `nativeConstantProductPool` / `REACT_APP_BOING_NATIVE_AMM_POOL`).

| Need | RPC / approach |
|------|----------------|
| **Reserve A / B** | Two **`boing_getContractStorage`** calls: `hex_contract_id` = pool account; storage keys are the **32-byte big-endian layout** for the pool program (see [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) and `boing_execution::reserve_a_key` / `reserve_b_key`). Each `value` word holds the u128 reserve in the **low 16 bytes** (high 16 zero), matching reference-token word style. |
| **Swap / liquidity** | Signed **`contract_call`** transactions (calldata per [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md)); preflight **`boing_simulateTransaction`** / **`boing_qaCheck`** on deploy bytecode as usual. |
| **Automated regression** | Repository test: `cargo test -p boing-node --test native_amm_rpc_happy_path` (deploy → add liquidity → swap; asserts reserves via `boing_getContractStorage`). |

When operators publish a **long-lived public testnet pool id**, it should be duplicated in [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) and partner env/config (checklist **A6.4**).

---

### boing_simulateTransaction

Simulate a transaction without applying it.

| Field | Type | Description |
|-------|------|-------------|
| Params | `[hex_signed_tx]` | Hex-encoded SignedTransaction |

**Result:** `{ gas_used: number, success: boolean, return_data: string, logs?: array, error?: string, suggested_access_list: { read: string[], write: string[] }, access_list_covers_suggestion: boolean }` — on success, `return_data` is hex (contract return buffer) and `logs` matches receipt log shape; on failure, `return_data` is `"0x"` and `logs` is `[]`. **`suggested_access_list`** is a **heuristic** minimum account set for parallel scheduling from the tx payload (see `TECHNICAL-SPECIFICATION.md` §4.2); **`access_list_covers_suggestion`** is `true` when the signed tx’s declared access list includes every account in that suggestion (read or write). Hints are included on both success and failure so clients can fix access lists before submit.

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
| Params | `[hex_bytecode]` or `[hex_bytecode, purpose_category, description_hash?, asset_name?, asset_symbol?]` | Bytecode only runs size/opcode/blocklist rules. With `purpose_category`, the same **full mempool QA** path applies as in `boing_submitTransaction` for deploy payloads (including optional 32-byte `description_hash`, then optional `asset_name` / `asset_symbol` for content policy). To pass only name/symbol without a real description commitment, use a placeholder 32-byte hex (e.g. all zeros). |
| Result | `{ result, rule_id?, message?, doc_url? }` | `result`: `"allow"`, `"reject"` (rule_id/message when applicable), or `"unsure"` (community QA pool). Mirrors mempool `boing_qa` for contract deploy. |

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
| -32051 | **QA: Pending pool** — Deployment referred to governance QA pool (result: Unsure). Response includes `data: { tx_hash: string }` (hex). |
| -32052 | **QA pool** — No pending item for the given `tx_hash`. |
| -32053 | **QA pool** — Voter is not a governance QA administrator. |
| -32054 | **QA pool disabled** — Governance has not enabled the pool (e.g. no `administrators` and `dev_open_voting` false, or `max_pending_items` is 0). |
| -32055 | **QA pool full** — Global `max_pending_items` reached; optional `data.reason: "pool_full"`. |
| -32056 | **QA pool deployer cap** — Sender exceeded `max_pending_per_deployer`; optional `data.reason: "deployer_cap"`. |
| -32057 | **Operator RPC auth** — `boing_qaPoolVote` or `boing_operatorApplyQaPolicy` called without valid `X-Boing-Operator` while `BOING_OPERATOR_RPC_TOKEN` is set on the node. |

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
