# BOING tokenomics & incentivized testnet

A plain-language guide to how BOING is meant to work, and how the **public testnet** relates to a possible **incentivized** phase later.

**Status:** public **testnet** (chain **6913**) is live. **Mainnet is not live.** An **incentivized** rewards program is **not** running today.  
**This page does not set a launch date** and does not promise you mainnet tokens for trying the testnet.

For the detailed draft (Proposed vs Undecided tables): [BOING-INITIAL-TOKENOMICS.md](BOING-INITIAL-TOKENOMICS.md).  
For the long design write-up: [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md).

---

## What BOING is for

BOING is the native token of Boing Network. On mainnet (when it ships) it would be used for:

- Paying small network fees
- Securing the chain (staking / validators)
- Funding treasury work (audits, tooling, ecosystem)
- Community programs (grants, quests, bug bounties) under transparent rules

On **today’s testnet**, BOING is only for trying things out. It has **no** real-world cash value.

---

## Testnet is not a mainnet claim

Please read this twice:

1. Using the faucet, completing quests, swapping on Finance, browsing Observer, or running a node **does not** mean you are owed mainnet BOING.
2. If we later run an **incentivized testnet**, rewards would only apply under a **published rules page**, with **caps**, and only while that program is marked **live**. Completing practice quests before that day does not create a claim unless those rules say so.

We want people on the testnet to learn the stack and help find bugs — not to treat every faucet drip as an airdrop ticket.

---

## Initial allocation (draft)

At mainnet launch, a **fixed initial supply** would be split into buckets. Exact percents are still being finalized. The working picture on the website has been roughly:

| Bucket | About | What it’s for |
|--------|------:|---------------|
| Staking / validator rewards | ~half or a bit more | Pay for security early on |
| Treasury | roughly a quarter to a third | Audits, ops, long-term runway |
| Community & Grants | about 15% | Quests / recognition, bug bounties, grants, successful dApps |

A **team & ops** slice (with equal founder shares and vesting) is under discussion and **not locked**. See [BOING-INITIAL-TOKENOMICS.md](BOING-INITIAL-TOKENOMICS.md) for Proposed vs Undecided labels.

**Hard cap in code today:** total native BOING held cannot grow past **1 billion** BOING. Older “uncapped waves” wording in design docs is historical context; shipping code enforces the cap.

---

## Fees and emissions (high level)

- **Fees:** Everyday actions should stay cheap (Solana-like small base fees; optional tips when the network is busy). Design docs aim for most fee revenue to support validators and treasury over time. On the **current** public testnet, protocol gas mostly goes to the **treasury**; block rewards still go to the block proposer.
- **Emissions:** New BOING from blocks is meant to **decline over years** toward a low floor (or stop if fees are enough), without sudden reward cliffs. Minting stops at the hard cap.

None of this is a promise of APY, price, or listing day.

---

## Community pots (quests, airdrop recognition, bug bounty)

Inside **Community & Grants**, the draft reserves room for:

- **Quest / incentivized-testnet recognition** — only if/when a rewards program is live and rules are public
- **Bug bounty** — for real security findings (program details TBD)
- **Ecosystem grants & dApps** — builders who ship useful apps, with per-app caps so one project cannot drain the pool

Until those programs are announced as live, treat portal quests as **practice and feedback**, not payouts.

---

## Path toward an incentivized testnet

We want to reach mainnet carefully. The preferred path is:

1. **Honest public testnet** — chain **6913**, wallet / DEX / explorer working, clear limits (today that includes a **single hosted validator** path for day-to-day public traffic).
2. **Soak & feedback** — users, developers, and operators exercise Express, Finance, and Observer; report bugs; improve docs.
3. **Publish rules** — criteria, caps, start/end, and “what is not a claim.”
4. **Short incentivized phase** (design sketch: about **2–4 weeks**) — optional recognition from the Community pot.
5. **Mainnet when ready** — no invented calendar date here.

Announcements will land on official channels (website, Discord). Until then, the portal banner is correct: **incentivized testnet is not currently running.**

---

## How to join today (no rewards required)

| I want to… | Go here |
|------------|---------|
| Make a wallet | [Boing Express](https://boing.express) |
| Get testnet BOING | [Faucet](https://boing.network/faucet) |
| Swap / NFTs | [Boing Finance](https://boing.finance) |
| Explore the chain | [Boing Observer](https://boing.observer) |
| Open the Testnet Hub | [boing.network/testnet](https://boing.network/testnet) |
| Join as a role (user / developer / operator) | [Register](https://boing.network/testnet/register) |
| Run a node | [Join Testnet](https://boing.network/testnet/join) · [Validator guide](https://boing.network/testnet/operators/validator) |
| Read the detailed draft | [BOING-INITIAL-TOKENOMICS.md](BOING-INITIAL-TOKENOMICS.md) |
| Read the whitepaper | [BOING-NETWORK-WHITEPAPER.md](BOING-NETWORK-WHITEPAPER.md) |

**Public RPC:** [https://testnet-rpc.boing.network/](https://testnet-rpc.boing.network/) · chain id **6913**.

---

## Honest limits (today)

- Public testnet only — **not** mainnet
- Day-to-day hosted path is still **early** (single hosted validator for most public users)
- Independent security audits are **not** finished
- Large media uploads have practical size limits even when the consensus cap is higher
- Testnet BOING is worthless as money

---

## Quick vocabulary

| Phrase | Meaning |
|--------|---------|
| **Public testnet** | The live practice chain (6913) |
| **Incentivized testnet** | A future, time-boxed program with published reward rules — **not live now** |
| **Mainnet claim** | A right to mainnet BOING — **only** if rules say so while a program is live |
| **Proposed** | Working number for discussion |
| **Undecided** | Needs an explicit founder/partner decision |

---

*Boing Network — Quality, enforced by the protocol. Draft language; not a rewards offer.*
