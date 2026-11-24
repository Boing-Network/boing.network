---
title: Wallet & dApps
description: Connect via window.boing, submit Boing-native transactions, and avoid EVM assumptions.
---

## Provider

**Boing Express** injects **`window.boing`**. It is a Boing-native provider (32-byte AccountIds, Ed25519). It does **not** execute EVM transactions; cross-chain EVM UX lives on Finance where applicable.

Typical dApp flow:

1. Detect `window.boing`.
2. Request accounts with `boing_requestAccounts` (full provider API: [WALLET_CONNECTION_AND_API.md](https://github.com/Boing-Network/boing.express/blob/main/docs/WALLET_CONNECTION_AND_API.md)).
3. Build payload with `@boing-network/boing-sdk`.
4. Simulate with `boing_simulateTransaction` / `boing_simulateContractCall` when useful.
5. Submit via the wallet (`boing_sendTransaction` path) so the user signs locally.

## Chain binding

Signing is chain-id aware (v2 domain). Wrong chain id or stale wallet builds produce opaque failures — honor `client_hints.min_wallet_version` (**0.1.18**+).

## CORS

If the browser talks to `https://testnet-rpc.boing.network/` directly, preflight must succeed. Many Boing apps use same-origin proxies (Observer `/api/rpc`, website Functions) to avoid CORS friction.

## Mainnet guardrails

Express keeps mainnet **off** until a distinct mainnet RPC URL is configured. Nodes on the planned mainnet chain **6914** refuse to start without operator RPC auth and never enable the faucet.
