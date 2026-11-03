# BOING — Initial Tokenomics (draft)

**Status:** planning draft for mainnet design and a future **incentivized testnet**.  
**Not an offer.** Percentages marked **Proposed** are working numbers for discussion. Items marked **Undecided** need Nico + partner lock-in.  
**No TGE / mainnet launch date** is asserted here.

**Audience:** founders, partners, operators writing rules pages. Everyday readers: [BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md](BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md).

**Cross-links:** [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md) § Tokenomics · [BOING-NETWORK-WHITEPAPER.md](BOING-NETWORK-WHITEPAPER.md) · [TESTNET.md](TESTNET.md) Part 3 · crate [`boing-tokenomics`](../crates/boing-tokenomics/src/lib.rs).

---

## 1. Hard facts vs planning

| Topic | Status | Notes |
|-------|--------|--------|
| Public testnet chain id **6913** | **Fact** | Live; values are test-only |
| Mainnet chain id **6914** | **Planned** | Not live |
| Genesis / initial supply = **1,000,000,000** BOING (`INITIAL_SUPPLY`) | **Locked (BN-155)** | 1B at genesis only — schedule base for emissions; **not** a minting ceiling |
| Supply ceiling after genesis | **Locked (BN-155): uncapped** | Block emissions continue past 1B under the declining schedule + floor; no hard-cap clamp ([design plan](BOING-BLOCKCHAIN-DESIGN-PLAN.md#chosen-model-uncapped-supply-with-floor-triggered-waves)) |
| Historical hard-cap clamp (`MAX_SUPPLY` / audit #61 item 40) | **Superseded** | Clamp removed from producer/validation; `MAX_SUPPLY` remains an alias of `INITIAL_SUPPLY` only |
| Incentivized testnet rewards program | **Not running** | Public testnet is open for soak; rewards are **not** live and do **not** create a mainnet claim |
| TGE / mainnet calendar date | **Undecided** | Do not invent one |

### Testnet ≠ mainnet claim

- Completing quests, running a node, reporting bugs, or registering in the Testnet Portal **does not** entitle anyone to mainnet BOING unless a **published incentivized-testnet rules page** says so and a rewards program is explicitly **live**.
- Testnet BOING has **no** cash value.
- Any future mainnet recognition for testnet work would be **capped**, paid from a reserved Community & Grants slice, and announced on official channels — never implied by faucet drips or leaderboard practice.

---

## 2. Initial supply allocation (mainnet genesis)

High-level buckets already published on the site and in docs (staking / treasury / community). This draft keeps that shape and adds founder/ops and community **sub-pots**.

| Bucket | Share of initial supply | Status | Purpose |
|--------|------------------------:|--------|---------|
| **Staking / validator rewards reserve** | **50%** | **Proposed** (was 55% on marketing charts) | Bootstrap security; topped up by ongoing block emissions (uncapped after genesis) |
| **Protocol treasury** | **20%** | **Proposed** (was 30%) | Audits, infra, contingency, governance-approved spend |
| **Community & Grants** | **15%** | **Proposed** (unchanged headline) | Quests/airdrop recognition, bug bounty, ecosystem grants, dApp incentives |
| **Team & ops** | **15%** | **Proposed** | Founders + early ops; see equal-split note below |

**Undecided:** exact rounding if charts stay at 55 / 30 / 15 vs the 50 / 20 / 15 / 15 split above; whether Team is a separate genesis bucket or a **vested carve-out of Treasury**.

### Team & ops (partner call)

| Item | Status |
|------|--------|
| Equal BOING allocation between Nico and partner at mainnet | **Proposed** (principle from partner call) |
| % of total supply for Team & ops | **Undecided** (15% above is a discussion starter) |
| Cliff / vesting schedule | **Undecided** — must be written before TGE messaging |
| Role titles (CSO / community / security) | **Undecided** |

### Community & Grants — proposed sub-pots (of the 15%)

Percentages below are **of total initial supply**, not of the 15% slice alone. Sum = 15%.

| Sub-pot | Share of initial supply | Status | Notes |
|---------|------------------------:|--------|-------|
| **Incentivized testnet / quest / airdrop recognition** | **6%** | **Proposed** | Caps eligibility; only if/when a rewards program is live |
| **Bug bounty** | **3%** | **Proposed** | Security research + critical protocol/app bugs |
| **Ecosystem grants & successful dApps** | **6%** | **Proposed** | Includes per-dApp epoch caps from design plan |

**Undecided:** exact sub-pot sizes; whether unused quest pot rolls to treasury or stays reserved; NFT vs liquid BOING for recognition.

---

## 3. Emissions (block rewards)

Aligned with design plan + `boing-tokenomics` (decline toward a low floor; fee-dominant maturity).

| Epoch | Annual inflation (illustrative) | Status |
|-------|--------------------------------:|--------|
| Year 1 | ~8% | **Proposed** |
| Year 2 | ~6% | **Proposed** |
| Year 3–5 | ~4.5% → ~2.5% | **Proposed** |
| Year 6–10 | ~2% → ~1.5% | **Proposed** |
| Year 10+ | **1% floor** or **0%** if fees suffice | **Proposed** |

Formula sketch: `emission_year_n = emission_year_1 × (decay)^(n-1)` with `decay ≈ 0.85`.  
Emission to round **proposer** remains independent of the tx-fee split (`BLOCK_EMISSION_PROPOSER_BPS`). Minting **continues past 1B** (uncapped after genesis); `INITIAL_SUPPLY` is only the genesis amount and the % base for the emission schedule.

**Undecided:** year-1 absolute BOING/block at mainnet genesis; whether floor is 1% or fee-triggered 0%.

---

## 4. Fee split

| Model | Validators / proposer | Treasury | Burn | Status |
|-------|----------------------:|---------:|-----:|--------|
| **Design target (maturity)** | **80%** | **20%** | **0%** (optional 0–5%) | **Proposed** |
| **Current public testnet runtime** | Protocol gas **0%** (100% treasury); tips **100% proposer**; block emission to proposer | **100%** of protocol gas | **0%** | **Fact** |

See [OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md). Micro fee target (~0.00021 BOING/transfer at recommended BPS) is product preference, not a TGE claim.

**Undecided:** when (if ever) mainnet moves from treasury-default gas to the 80/20 design target; tip BPS bounds.

---

## 5. Incentivized testnet framing

| Item | Status |
|------|--------|
| Public testnet open for users / devs / operators | **Fact** (chain **6913**) |
| Rewards program live | **No** — not currently running |
| Path: soak → publish rules → short Phase 1 (2–4 weeks) → capped recognition from Community pot | **Proposed** process |
| Duration of first program | **Proposed** 2–4 weeks ([TESTNET.md](TESTNET.md) Part 3) |
| Start/end calendar dates | **Undecided** — fill only when announcing |
| Mainnet claim from testnet activity alone | **Rejected** — requires live rules + explicit eligibility |

### Role sketch (when a program is live)

| Role | Primary testnet activity | Optional mainnet link (capped) |
|------|--------------------------|--------------------------------|
| Users | Faucet, Express, Finance, Observer quests, feedback | From quest/airdrop sub-pot |
| Developers | Deploy, register dApp, bug reports | Grants / bounty eligibility |
| Operators | Run node, uptime, leaderboard practice | Small allocation or NFT for top N |

Full ops checklist: [TESTNET.md](TESTNET.md) Part 3. Portal: [boing.network/testnet](https://boing.network/testnet).

---

## 6. Honest network limits (today)

Do not contradict these in tokenomics marketing:

- Single hosted public validator path (`boing-testnet-1`); not multi-region HA
- Independent audits unfinished
- PutBlob consensus cap 256 MiB ≠ practical public-gateway body size
- Governance / some opcodes still height- or env-gated

---

## 7. Open decisions for Nico (+ partner)

1. Lock **55/30/15** marketing pie vs **50/20/15/15** (with Team) for genesis.
2. Team **%**, **equal split**, **cliff/vesting** — write before any TGE talk.
3. Community sub-pot sizes (quest **6%** / bounty **3%** / grants **6%**) — accept, adjust, or defer.
4. Fee path: keep treasury-default gas on mainnet day one, or schedule move to 80/20.
5. Whether first incentivized testnet is announced only after **≥2 validators** (partner-call soak priority) or can run on honest single-validator framing.
6. Bug-bounty intake (GitHub Security Advisories vs email) and severity table.
7. Rules page URL and claim language legal review before “rewards live” flip.

---

## 8. Document map

| Doc | Role |
|-----|------|
| This file | Allocation / emissions / fees / pots — Proposed vs Undecided |
| [BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md](BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md) | Plain-language companion (+ PDF) |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Long-form design + sustainability |
| [BOING-NETWORK-WHITEPAPER.md](BOING-NETWORK-WHITEPAPER.md) | Protocol + product; no invented launch date |
| [TESTNET.md](TESTNET.md) Part 3 | Incentivized program ops design |
| `crates/boing-tokenomics` | Implemented emission / fee / `INITIAL_SUPPLY` constants (uncapped after genesis) |

---

*Draft only — update when decisions lock. No rewards offer is made by this document.*
