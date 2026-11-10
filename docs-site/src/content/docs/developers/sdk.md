---
title: SDK
description: Install and use @boing-network/boing-sdk against public testnet.
---

## Package

| | |
|--|--|
| npm | **`@boing-network/boing-sdk`** |
| Recommended (live `client_hints`) | **`^0.5.41`** |
| Source | [`boing-sdk/`](https://github.com/Boing-Network/boing.network/tree/main/boing-sdk) |

Install the **scoped** package. Do not depend on an unscoped `boing-sdk` name for external apps.

```bash
npm install @boing-network/boing-sdk@^0.5.41
```

After changing SDK source in the monorepo, run `npm run build` inside `boing-sdk/` so `dist/` matches `src/` (tutorial packages consume `file:../../boing-sdk`).

## Core client

```ts
import {
  createClient,
  BoingRpcError,
  explainBoingRpcError,
  BOING_TESTNET_CHAIN_ID_DECIMAL,
} from '@boing-network/boing-sdk';

// Prefer an options object on public RPC so Cloudflare 429s can retry.
// Default maxRetries is 0 (no retries) — fine for a local node, not for testnet-rpc.
const client = createClient({
  baseUrl: 'https://testnet-rpc.boing.network/',
  maxRetries: 3,
});

const info = await client.getNetworkInfo(); // wraps boing_getNetworkInfo
console.log(info.chain_id, BOING_TESTNET_CHAIN_ID_DECIMAL);

try {
  await client.chainHeight();
} catch (e) {
  if (e instanceof BoingRpcError) {
    console.error(explainBoingRpcError(e)); // HTTP 429 / -32016 / gateway limits
  }
  throw e;
}
```

Config is a URL string or `{ baseUrl, fetch?, timeoutMs?, maxRetries? }` — not `rpcUrl`. A bare URL string keeps `maxRetries: 0`.

**Public RPC note:** `https://testnet-rpc.boing.network/` is fronted by Cloudflare. Expect occasional **HTTP 429** (and faucet **-32016**). Set **`maxRetries: 2–3`** so the client backs off (honors `Retry-After` when present). Use `explainBoingRpcError` / `isRetriableBoingRpcError` for UX.

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
