---
title: Express, Finance, Observer
description: How the three product surfaces map onto public testnet RPC and each other.
---

## Boing Express

- **Repo:** [boing.express](https://github.com/Boing-Network/boing.express)
- **Role:** Non-custodial wallet + `window.boing` injection for dApps
- **Default RPC:** `https://testnet-rpc.boing.network/`
- **Notes:** PutBlob publish sessions; DEX list RPCs; Connected sites management

## Boing Finance

- **Repo:** [boing.finance](https://github.com/Boing-Network/boing.finance)
- **Role:** App hub — swaps, pools, tokens, NFTs / media on L1 **6913**
- **Wiring:** Canonical pool / factory addresses from network info + SDK; React env `REACT_APP_BOING_NATIVE_AMM_POOL` for the default CP pool hex
- **Notes:** Prefer L1 blobs for NFT media when enabled; approval / swap strings must match wallet intent UI

## Boing Observer

- **Repo:** [boing.observer](https://github.com/Boing-Network/boing.observer)
- **Role:** Explorer and transparency (blocks, accounts, QA, tokens)
- **Default RPC:** `NEXT_PUBLIC_TESTNET_RPC` → `https://testnet-rpc.boing.network/`
- **Notes:** Slim block fetches (`include_code: false`) for large PutBlob/deploy payloads; blob download ACL via wallet proof / operator tokens; same-origin RPC proxy when CORS blocks the browser

## Shared rules

| Rule | Detail |
|------|--------|
| Testnet only for production apps today | Mainnet 6914 not live |
| One RPC snapshot | Align DEX UI to `boing_getNetworkInfo` + factory-scoped list methods |
| PDFs & long specs | Stay on [boing.network/pdfs](https://boing.network/pdfs/) and GitHub `docs/` — this site summarizes |
