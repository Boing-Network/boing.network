# JSON-RPC error codes for dApps (Boing node)

Canonical definitions live in [RPC-API-SPEC.md](RPC-API-SPEC.md). This page is a **dApp-focused** cheat sheet and alignment target for **Boing Express** (and any injected wallet): returned `code` / `message` / `data` should match these contracts so clients like `boing-sdk` can branch reliably.

---

## Standard JSON-RPC

| Code | Meaning |
|------|---------|
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |

---

## Boing application errors

| Code | When | `data` (when applicable) |
|------|------|-------------------------|
| -32000 | Generic server error (serialization, faucet empty, etc.) | varies |
| -32016 | Faucet rate limit | — |
| -32050 | **QA: deployment rejected** | `{ rule_id: string, message: string }` |
| -32051 | **QA: Unsure → governance pool** | `{ tx_hash: string }` |
| -32052 | QA pool: no pending item for `tx_hash` | — |
| -32053 | QA pool: voter not an administrator | — |
| -32054 | QA pool disabled by governance | — |
| -32055 | QA pool full (global cap) | optional `{ reason: "pool_full" }` |
| -32056 | QA pool: per-deployer cap | optional `{ reason: "deployer_cap" }` |
| -32057 | Operator RPC: missing/invalid `X-Boing-Operator` | — |

---

## TypeScript handling

`boing-sdk` throws **`BoingRpcError`** with the JSON-RPC `code` and optional structured `data`. Use:

- `e.isQaRejected`, `e.qaData` for **-32050**
- `e.isQaPendingPool`, `e.pendingPoolTxHash` for **-32051**
- `e.isQaPoolDisabled`, `e.isQaPoolFull`, `e.isQaPoolDeployerCap` for **-32054..-32056**
- **`explainBoingRpcError(e)`** for user-facing log/UI strings

---

## Boing Express alignment (track W2)

When the extension surfaces errors (user reject, locked wallet, RPC forward failures), prefer:

1. **User reject / lock:** use a **distinct** message from QA/mempool errors (do not reuse `-32050` text).
2. **Forwarded node errors:** preserve the node’s numeric **`code`** and **`data`** object when proxying `boing_submitTransaction` / `boing_simulateTransaction` so `BoingRpcError` parsing stays valid.

Implementation lives primarily in the **boing.express** repo; this document is the **contract** the wallet should follow.

---

## References

- [RPC-API-SPEC.md](RPC-API-SPEC.md)
- [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)
- [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md)
