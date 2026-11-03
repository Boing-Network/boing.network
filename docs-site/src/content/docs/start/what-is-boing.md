---
title: What is Boing
description: Independent Layer 1 with protocol-level Quality Gate — public testnet today; mainnet not live.
---

Boing Network is an **independent Layer 1**: its own accounts, VM, consensus, and JSON-RPC surface — not an EVM L2 or a fork of another chain.

**Positioning (brand):** quality is enforced by the protocol. Deployments are screened by the **Boing Quality Gate** before they land on the ledger. That does **not** mean “scam-proof”; it means transparent automated checks plus a community path for edge cases.

## Status you should assume

| Network | Chain id | Status |
|---------|----------|--------|
| Public testnet | **6913** (`0x1b01`) | **Live** |
| Mainnet | **6914** (`0x1b02`) | **Planned — not live** |

Testnet assets are for experimentation. Do not treat them as mainnet claims or transferable real-world value.

## Product map

| Product | Job | URL |
|---------|-----|-----|
| **Boing Express** | Non-custodial wallet; injects `window.boing` for dApps | [boing.express](https://boing.express) |
| **Boing Finance** | Swaps, pools, tokens, NFTs on Boing L1 (plus related tools) | [boing.finance](https://boing.finance) |
| **Boing Observer** | Explorer, QA visibility, chain reads | [boing.observer](https://boing.observer) |
| **Website** | Portal, faucet, resources, **PDF library** | [boing.network](https://boing.network) |

## What “native” means for builders

- Addresses are **32-byte AccountIds** (Ed25519 public keys), shown as `0x` + 64 hex.
- Transactions are **bincode-encoded** and signed with Boing domains (`BOING_TX_V2`, optional priority tip) — not Ethereum RLP / secp256k1.
- Prefer **`@boing-network/boing-sdk`** and live **`boing_getNetworkInfo`** over hardcoding heights or contract addresses.

Deep wire formats stay in specialist specs (linked from [Specialist specs](/resources/specialist-specs/)). Printable overviews live in the [PDF library](/resources/pdfs/).
