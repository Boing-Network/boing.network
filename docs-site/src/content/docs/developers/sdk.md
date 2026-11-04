---
title: SDK
description: Install and use @boing-network/boing-sdk against public testnet.
---

## Package

| | |
|--|--|
| npm | **`@boing-network/boing-sdk`** |
| Recommended (live `client_hints`) | **`^0.5.39`** |
| Source | [`boing-sdk/`](https://github.com/Boing-Network/boing.network/tree/main/boing-sdk) |

Install the **scoped** package. Do not depend on an unscoped `boing-sdk` name for external apps.

```bash
npm install @boing-network/boing-sdk@^0.5.39
```

After changing SDK source in the monorepo, run `npm run build` inside `boing-sdk/` so `dist/` matches `src/` (tutorial packages consume `file:../../boing-sdk`).

## Core client

```ts
import { createClient, BOING_TESTNET_CHAIN_ID_DECIMAL } from '@boing-network/boing-sdk';

// URL string, or `{ baseUrl, fetch?, timeoutMs?, maxRetries? }` (not `rpcUrl`).
const client = createClient('https://testnet-rpc.boing.network/');

const info = await client.getNetworkInfo(); // wraps boing_getNetworkInfo
console.log(info.chain_id, BOING_TESTNET_CHAIN_ID_DECIMAL);
```

Use `BOING_TESTNET_CHAIN_ID_DECIMAL` (**6913**) / `BOING_TESTNET_CHAIN_ID_HEX` (`0x1b01`) for local branching, but prefer the live `chain_id` when connected.

## What else ships in the package

Exports (non-exhaustive) cover:

- Tx helpers, fees, hex/AccountId utilities, access lists
- Native pool / router / custody / LP vault call builders
- Canonical testnet DEX address constants + merge helpers driven by network info
- Reference fungible / NFT / linked-NFT / builder-attestation bytecode helpers
- Error explanation helpers for RPC rejection reasons

See `boing-sdk/src/index.ts` for the authoritative export surface.

## dApp integration

End-to-end connect / simulate / submit patterns: [BOING-DAPP-INTEGRATION.md](https://github.com/Boing-Network/boing.network/blob/main/docs/BOING-DAPP-INTEGRATION.md). Wallet provider API: Express docs for `window.boing`.
