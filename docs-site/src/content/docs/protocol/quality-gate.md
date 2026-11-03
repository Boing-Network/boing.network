---
title: Quality Gate
description: Protocol-level deploy screening — automated gates plus on-chain Unsure pool semantics after QA canonical height.
---

## What it is

The **Boing Quality Gate** screens contract deployments against transparent rules before acceptance on the ledger. Automated checks handle clear Allow / Reject cases; unclear cases can go to a community **Unsure** review path.

Never market this as eliminating all malicious activity. Approved phrasing lives in monorepo `BRAND.md`.

## Live knobs

From height **`qa_canonical_activation_height`** (**1700** on public testnet 6913 as reported by the node):

- Block execution uses the **committed on-chain QA policy** (node-local config mainly gates mempool admission).
- **Unsure** deploys follow the on-chain public pool configuration.
- Related RPCs: `boing_getQaRegistry`, `boing_qaCheck`, `boing_qaPoolConfig`, `boing_qaPoolList`, `boing_qaPoolVote`, plus operator apply helpers when authenticated.

Probe the node for `source: "canonical"` vs `"local"` on pool config after that height.

## Builder workflow

1. Prefer reference fungible / NFT templates and canonical deploy artifacts from the SDK.
2. Run `boing_qaCheck` / local QA tooling before paying fees on a doomed deploy.
3. Read Allow / Reject / Unsure outcomes on [Boing Observer](https://boing.observer).

## Deeper reading (not mirrored here)

- Plain-language: [QUALITY-ASSURANCE-NETWORK.md](https://github.com/Boing-Network/boing.network/blob/main/docs/QUALITY-ASSURANCE-NETWORK.md) · [PDF](https://boing.network/pdfs/QUALITY-ASSURANCE-NETWORK.pdf)
- Gate catalog: [QA-GATE-RULES.md](https://github.com/Boing-Network/boing.network/blob/main/docs/QA-GATE-RULES.md) · [PDF](https://boing.network/pdfs/QA-GATE-RULES.pdf)
