---
title: Get started
description: First path on Boing public testnet — wallet, faucet, explorer, and a minimal RPC check.
---

## 1. Install the wallet

1. Install **[Boing Express](https://boing.express)** (Chrome extension).
2. Create or import an account. Addresses are 32-byte AccountIds (`0x` + 64 hex).
3. Confirm the extension points at the public testnet RPC (`https://testnet-rpc.boing.network/` by default).

Live advisory versions from `boing_getNetworkInfo.client_hints`: minimum wallet **0.1.18**, recommended wallet **0.1.25**, recommended SDK **0.5.41** (`@boing-network/boing-sdk`). These hints are **never enforced** by the node.

## 2. Get testnet BOING

Use the faucet: [boing.network/faucet](https://boing.network/faucet).

## 3. Explore the chain

Open [boing.observer](https://boing.observer) to inspect blocks, accounts, tokens, and QA outcomes.

## 4. Try Finance

[boing.finance](https://boing.finance) is the app hub for swaps, pools, tokens, and NFTs on chain **6913**. Connect with Express (`window.boing`).

## 5. Smoke-check RPC

```bash
curl -sS https://testnet-rpc.boing.network/ \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_getNetworkInfo","params":[]}'
```

Confirm `chain_id` is `6913`, then read `head_height`, activation fields, and `end_user` / `developer` hints before wiring an app.

## Next

- [Network & activations](/protocol/network/)
- [JSON-RPC](/developers/rpc/)
- [SDK](/developers/sdk/)
- [Join testnet (roles)](/testnet/join/)
