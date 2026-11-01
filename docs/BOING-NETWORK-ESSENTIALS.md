# Boing Network — Essentials

A plain-language snapshot of what Boing Network is today, what you can use, and what is still early.

**Status:** public **testnet** is live. **Mainnet is not live yet.**

---

## What is Boing Network?

Boing Network is its own **Layer 1** blockchain — a new chain built from the ground up, not a copy of another network.

Today you can:

- Create a wallet and hold testnet BOING
- Swap tokens and explore NFTs / media on-chain
- Look up blocks, accounts, and activity on the explorer
- Get free testnet BOING from the faucet
- Join the community on Discord

Everything below runs on the **public testnet**. Values and assets there are for trying things out, not for real-world money.

---

## Products

| Product | What it is | Link |
|---------|------------|------|
| **Boing Express** | The wallet — create an account, send and receive, approve app actions | [boing.express](https://boing.express) |
| **Boing Finance** | The app hub — swaps, pools, tokens, and NFTs on Boing (and related tools) | [boing.finance](https://boing.finance) |
| **Boing Observer** | The explorer — see blocks, accounts, tokens, QA decisions, and network activity | [boing.observer](https://boing.observer) |
| **Faucet** | Free testnet BOING so you can try the chain without buying anything | [boing.network/faucet](https://boing.network/faucet) |
| **Website** | Home base — docs, testnet portal, status, and community links | [boing.network](https://boing.network) |

---

## How apps talk to the chain

Apps do not “live inside” the chain. They ask a public gateway (the **RPC**) for balances, recent activity, and to submit the actions you approve in your wallet.

**Public testnet gateway:** [https://testnet-rpc.boing.network/](https://testnet-rpc.boing.network/)

In plain terms, that gateway lets apps:

- Check your balance and recent activity
- Look up blocks, tokens, and pools
- Send a transaction you already signed in Express
- Request faucet funds (via the faucet page)
- Work with on-chain media (with wallet help for larger files)

Deep method lists and integrator details: [BOING-NETWORK-TECHNICAL-ESSENTIALS.md](BOING-NETWORK-TECHNICAL-ESSENTIALS.md) (technical companion) and [RPC-API-SPEC.md](RPC-API-SPEC.md) (full method catalog).

---

## What you can do today

- **Swap** — Trade tokens on Boing’s native DEX through Finance (wallet connected).
- **NFTs & media** — Create and view NFTs; larger media can live on the chain itself (PutBlob), with wallet-assisted upload paths.
- **Quality Gate (QA)** — Before a new token or contract lands on the ledger, Boing runs transparent checks. Clear cases are allowed or rejected automatically; odd edge cases can go to a community review pool. Goal: cut down malicious or deceptive junk while still allowing playful meme / community projects.
- **Fees & staking (high level)** — Everyday actions use a small network fee. Staking and validator rewards are part of how the chain is designed to stay secure over time; on today’s public testnet the setup is still early (see limits below).
- **Community** — Chat, feedback, and updates on [Discord](https://discord.gg/K3w9nbUw4N).

Deeper reading: [SIX-PILLARS.md](SIX-PILLARS.md) (why Boing prioritizes what it does) · [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) (QA in more detail) · [TESTNET.md](TESTNET.md) (how to join and use testnet).

---

## Honest limits (today)

Be clear with yourself and others:

- This is a **public testnet**, not mainnet.
- The hosted chain is still **early** — for now, a **single hosted validator** runs the public network path people use day to day.
- Independent security audits are **not** finished.
- Features continue to ship and change; some Finance surfaces are still deepening (more pools, richer discovery, more staking UI).
- Testnet BOING has **no** real-world cash value.

When mainnet and a more decentralized validator set ship, this page will be updated.

---

## Quick links

| I want to… | Go here |
|------------|---------|
| Make a wallet | [boing.express](https://boing.express) |
| Get testnet BOING | [Faucet](https://boing.network/faucet) |
| Swap / NFTs | [boing.finance](https://boing.finance) |
| Look something up | [boing.observer](https://boing.observer) |
| Read the pillars | [SIX-PILLARS](SIX-PILLARS.md) · [PDF](../website/public/pdfs/SIX-PILLARS.pdf) |
| See what shipped when | [FEATURES.md](FEATURES.md) · [PDF](../website/public/pdfs/FEATURES.pdf) |
| Join Discord | [discord.gg/K3w9nbUw4N](https://discord.gg/K3w9nbUw4N) |
| Build or integrate | [Technical Essentials](BOING-NETWORK-TECHNICAL-ESSENTIALS.md) · [PDF](../website/public/pdfs/BOING-NETWORK-TECHNICAL-ESSENTIALS.pdf) · [docs index](README.md) · [dApp integration](BOING-DAPP-INTEGRATION.md) · [RPC spec](RPC-API-SPEC.md) |
| Run a node / ops | [TESTNET.md](TESTNET.md) · [RUNBOOK.md](RUNBOOK.md) |

---

*Boing Network — Quality, enforced by the protocol.*
