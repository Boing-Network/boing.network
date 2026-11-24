---
title: Glossary
description: Plain definitions of the terms used across Boing Network, Boing Express, Boing Observer, and Boing Finance.
---

The words you will see across Boing Network and its apps.

| Term | Meaning |
|------|---------|
| **Boing Network (Boing L1)** | An independent Layer 1 blockchain with its own VM, Ed25519 signatures, and native BOING. It is not an EVM chain. |
| **BOING** | The native coin. It pays fees. Since block 1050 on testnet it has 18 decimals (1 BOING = 10^18 base units). Testnet BOING comes from the faucet and has no monetary value. |
| **Address (AccountId)** | Your 32-byte account identifier, shown as `0x` + 64 hex characters. It is your Ed25519 public key. Some APIs call it the account id. |
| **Private key** | The 32-byte secret that signs for your address. Boing Express shows it under **Reveal private key**. Never share it. |
| **Boing Express** | The official wallet: a web app at [boing.express](https://boing.express) and a Chrome extension that connects dApps through `window.boing`. |
| **Boing Observer** | The block explorer at [boing.observer](https://boing.observer): blocks, transactions, accounts, tokens, and Quality Gate outcomes. |
| **Boing Finance** | The app hub at [boing.finance](https://boing.finance): native swaps and pools, token and NFT deploys. |
| **Testnet / Mainnet** | The public testnet is chain id **6913** (`0x1b01`). Mainnet will be chain id **6914** (`0x1b02`) and is not live. |
| **Faucet** | Free testnet BOING at [boing.network/faucet](https://boing.network/faucet). |
| **Nonce** | A per-account transaction counter. Wallets, the SDK, and the CLI fill it in for you. |
| **Quality Gate (QA)** | The rules every contract deploy is checked against before it lands on chain. Clear cases are allowed or rejected automatically; unclear ones go to a public review pool. See [Quality Gate](/protocol/quality-gate/). |
| **Reference token** | The standard fungible token template used by Boing Finance, the `boing token` CLI, and `@boing-network/boing-sdk`. |
| **Priority tip** | An optional extra fee that asks for faster inclusion when the network is busy. See [Fees](/protocol/fees/). |
| **Media blob** | Content-addressed data, such as NFT media, stored on chain. See [Media blobs](/protocol/media-blobs/). |
| **Bootnode** | A stable peer that a new node dials first to find the network. See [Bootnodes](/nodes/bootnodes/). |
| **Validator** | A node that proposes and votes on blocks. On testnet, anyone can register one with a 100 BOING bond. See [Become a validator](/nodes/validator/). |
| **Epoch** | A 100-block period. Validator set changes take effect at epoch boundaries. |
| **Activation height** | The block height from which a new protocol rule applies. Nodes running a build without the rule fork off at that height. |
