# Incentivized Testnet — Readiness, Promotion & Mainnet Migration

> **Purpose:** Prepare for, promote, and migrate from an incentivized testnet where validators, developers, and users can earn rewards.  
> **References:** [TESTNET.md](TESTNET.md), [READINESS.md](READINESS.md), [RUNBOOK.md](RUNBOOK.md), [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md), [BUILD-ROADMAP.md](BUILD-ROADMAP.md).

---

## Part 1 — Readiness & Launch

### Current Status

VibeMiner shows "no nodes" until bootnodes and public RPC are live. Complete the [Launch-Blocking Checklist](READINESS.md#3-launch-blocking-checklist-critical-path) in [READINESS.md](READINESS.md) before announcing the incentivized testnet.

### Duration: 1 Week vs 1 Month

| Duration | Recommendation |
|----------|----------------|
| **1 week** | Soft launch or rehearsal only |
| **2–4 weeks** | **Recommended** for first incentivized testnet |
| **1 month+** | Consider for Phase 2 after a 2–4 week Phase 1 |

### Technical Readiness Checklist

- [ ] Bootnodes running; multiaddrs in TESTNET.md and website
- [ ] Public RPC live; faucet enabled and tested
- [ ] Genesis and chain ID documented; multi-node sync verified
- [ ] Website testnet/faucet pages show correct RPC and bootnodes

### Incentive Program Design

| Role | Primary incentive | Optional mainnet link |
|------|-------------------|------------------------|
| Validators | Testnet BOING (blocks) + leaderboard/uptime | Small allocation or NFT for top N (capped) |
| Developers | Testnet BOING, bug bounties, dApp recognition | Grant eligibility |
| Users | Faucet + quests/feedback | Optional NFT (capped) |

Document caps and eligibility in a single "Incentivized Testnet Rules" page.

### Pre-Launch Checklist

- [ ] Incentive program documented with criteria and caps
- [ ] **Testnet Portal** live: registration (developer / user / node operator), dashboards; see [TESTNET-PORTAL.md](TESTNET-PORTAL.md). Community quests live under **Users** at `/testnet/users`; see [COMMUNITY-QUESTS.md](COMMUNITY-QUESTS.md).
- [ ] Bug bounty scope (if any) defined; contact for submissions
- [ ] Feedback channel live (Discord, GitHub Discussions)
- [ ] Announcement draft with start/end dates and links

### Launch Day

1. Publish bootnode list and public RPC URL
2. Announce start with duration and links
3. Monitor RPC and bootnodes
4. Pin quick start in Discord/Telegram

### Phase 1 Parameters (fill before launch)

| Parameter | Placeholder |
|-----------|-------------|
| Start date | e.g. 2025-03-15 |
| End date | 2–4 weeks recommended |
| Public RPC | https://testnet-rpc.boing.network/ |
| Validator rewards | TBD — define criteria and caps |
| Developer rewards | TBD — per track |
| Bug bounty | GitHub Security Advisories or security@ |

---

## Part 2 — Promotion

### Messaging

- **One-liner:** "Earn testnet BOING and qualify for mainnet BOING. Run a validator, build a dApp, or complete quests — rewards from our Community & Grants pool."

### Channels and Timeline

| When | Action |
|------|--------|
| 2–3 weeks before | Teaser: dates and rules coming soon |
| 1 week before | Publish full rules; pin in Discord/Telegram |
| Launch day | Announce live; link testnet, faucet, rules |
| Mid-phase | Recap stats; optional leaderboard teaser |
| 1 week before end | Reminder to qualify |

### Where to Promote

- Twitter/X, Discord, Telegram, blog, website, VibeMiner/partners. Link to the **Testnet Portal** (`/testnet`) for registration and role-specific dashboards.

---

## Part 3 — Mainnet Migration

### Before Testnet Ends

- Fix end date; state everywhere
- Clarify "mainnet reward" meaning
- Collect validator addresses, bug reports, quest submissions

### After Testnet Ends

- Thank-you post; publish results/leaderboards
- No new promises; stick to published rules

### Mainnet Launch

- "Same chain you tested is now mainnet"
- Publish "Testnet rewards claim" page with eligibility and claim process
- Distribute mainnet BOING from Community & Grants pool per rules

### Tying to Initial Supply

Reserve a defined **Testnet Rewards** slice (e.g. X% of initial supply) from **Community & Grants (15%)** for validators, developers, and users. Publish the cap so all payouts come from this pool.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
