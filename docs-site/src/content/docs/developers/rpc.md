---
title: JSON-RPC
description: The public Boing JSON-RPC endpoint, how to discover methods, and the main method groups.
---

## Endpoint

| | |
|--|--|
| URL | `https://testnet-rpc.boing.network/` |
| Transport | HTTPS JSON-RPC `POST /` |
| Gateway | Cloudflare Worker (`workers/public-rpc-gateway`) → Fly `boing-testnet-1` |
| Batching | Supported (`jsonrpc_batch_max` reported on network info; commonly **32**) |
| WebSocket | `/ws` with `newHeads` subscribe handshake (see `developer.websocket`) |

Browser apps that call the RPC directly depend on the gateway's CORS settings. If a browser blocks your requests, route them through a same-origin proxy on your own site.

## Discover what the node implements

Ask the node what it supports instead of hardcoding a method list:

| Method | Returns |
|--------|---------|
| `boing_rpcSupportedMethods` | Sorted string array of implemented `boing_*` names |
| `boing_getRpcMethodCatalog` | Embedded JSON Schema catalog for codegen/docs |
| `boing_getRpcOpenApi` | OpenAPI document |
| HTTP | `GET /openapi.json`, `GET /.well-known/boing-rpc` |

The node implements over 100 `boing_*` methods. [RPC-API-SPEC.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RPC-API-SPEC.md) documents every one of them with exact parameters and results; a test in the node fails if a method is missing from it.

## Method groups (orientation)

| Group | Examples |
|-------|----------|
| Health / tip | `boing_health`, `boing_getSyncState`, `boing_chainHeight`, `boing_getNetworkInfo` |
| Accounts / txs | `boing_getAccount`, `boing_getBalance`, `boing_submitTransaction`, `boing_getTransactionReceipt`, `boing_simulateTransaction` |
| Blocks / logs | `boing_getBlockByHeight`, `boing_getBlockByHash`, `boing_getLogs` |
| Contracts | `boing_getContractCode`, `boing_getContractStorage`, `boing_simulateContractCall` |
| DEX directory | `boing_listDexPools`, `boing_listDexTokens`, `boing_getDexToken` |
| Tokens | `boing_listTokens`, `boing_getTokenMetadata` |
| Media blobs | `boing_getMediaBlobParams`, `boing_getBlob`, `boing_getBlobMeta`, DAS/sample helpers |
| QA / governance | `boing_qaCheck`, `boing_qaPool*`, `boing_governance*` |
| Names | `boing_resolveName`, `boing_getNameInfo`, … |
| Operator (auth) | `boing_operatorApplyFeePolicy`, `boing_operatorApplyQaPolicy`, … |

`boing_getNetworkInfo.rpc.not_available` lists capabilities the surface intentionally omits (e.g. `staking_apy`).

## Minimal client sketch

```js
const res = await fetch('https://testnet-rpc.boing.network/', {
  method: 'POST',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    id: 1,
    method: 'boing_getBalance',
    params: ['0x' + '11'.repeat(32)],
  }),
});
const { result, error } = await res.json();
```

Prefer **`BoingClient`** from `@boing-network/boing-sdk` for retries, typing, and error mapping.

## Full specification

- Spec: [RPC-API-SPEC.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RPC-API-SPEC.md)
- PDF: [RPC-API-SPEC.pdf](https://boing.network/pdfs/RPC-API-SPEC.pdf)
- Tx encoding: [BOING-SIGNED-TRANSACTION-ENCODING.md](https://github.com/Boing-Network/boing.network/blob/main/docs/BOING-SIGNED-TRANSACTION-ENCODING.md)
