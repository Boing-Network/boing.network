# Accelerator Applications — Beacon & Outlier Ventures (OV)

> **Dated draft (reviewed 2026-10-27).** Pitch/application copy written before the 2026 rebrand; it is kept for reference and may lag current positioning. Boing is on a **public testnet. Mainnet is not live, and independent audits are not complete.** Approved wording lives in [../BRAND.md](../BRAND.md).

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


> **Purpose:** Draft answers and copy-paste content for **Beacon** and **Outlier Ventures (OV) Black** accelerator applications. Fill in fields marked **[YOU FILL]** before submitting.  
> **Pitch deck PDF:** Generate with `npm run build:pdfs` in `website/`; upload `website/public/pdfs/Executive-Summary-Pitch-Deck.pdf` when the form asks for a deck.

---

## Shared answers (copy-paste for either form)

### Product stage

**MVP / Prototype / Proof of Concept**

*Rationale: Boing has a working node binary, consensus (HotStuff), P2P (libp2p), CLI (`boing init`, `boing dev`, `boing deploy`), SDK (TypeScript), testnet infrastructure (bootnodes, faucet), and comprehensive documentation. The protocol is functional but not yet at mainnet.*

### Ecosystem

**My own network (Boing.Network)** — optional to add: bridged liquidity and routing (via boing.finance).

### Elevator pitch (75 words or less)

Boing Network is an independent L1 built from first principles with protocol-enforced quality assurance: Boing introduces protocol-level deployment checks designed to identify and restrict deployments that fail transparent quality criteria before they are accepted on-chain. Combined with native adaptive gas / micro fees, and transparent, published tokenomics, we are building infrastructure for a sustainable DeFi ecosystem. Public testnet live (mainnet not live); SDK and CLI ready.

### Problem (200 words max)

Most L1 blockchains allow any bytecode to be deployed. Users and protocols bear the risk of malicious or low-quality deployments, with no protocol-level protection. When anything can be deployed, chains become clogged with low-value or spam transactions—driving up fees and degrading UX for legitimate users. Liquidity is fragmented across dozens of chains; cross-chain coordination is trust-heavy and complex. Many networks lack verifiable transparency in tokenomics, treasury, and upgrades. Reward cliffs and unchecked inflation undermine long-term viability. These problems matter because they erode trust, waste resources, and slow mainstream adoption. Users lose funds to scams; developers face hostile environments; and validators operate in opaque ecosystems. A chain that enforces quality at the protocol layer—while remaining transparent and sustainable—directly addresses these pain points and creates a foundation for responsible DeFi growth.

### Solution (200 words max)

Boing introduces protocol-enforced quality assurance: deployments are checked against a configurable rule registry (bytecode size/opcode gates, optional blocklists and content policy, purpose declaration) at mempool admission and execution. Rejected deploys never land; `Unsure` results can enter a community QA pool rather than auto-accepting. Out-of-the-box nodes start with size/opcode gates until operators apply the canonical registry (including content policy). This reduces network congestion by keeping mempools and blocks focused on legitimate transactions. We combine QA with an adaptive / micro fee model (predictable caps, optional priority tips), and cross-chain DeFi coordination via boing.finance. Account abstraction (gasless UX, social recovery, session keys) remains on the design roadmap rather than a shipped protocol feature. Phased governance uses time-locked proposals—no surprise upgrades. Tokenomics are sustainable: a hard-capped supply of 1 billion BOING (decaying emissions with a long-term floor that stops minting once the cap is reached) and no reward cliffs. The core protocol (consensus, execution, state, P2P) is implemented in Rust; developer tools include `boing init`, `boing dev`, `boing deploy`, and a TypeScript SDK. The public testnet is live with bootnodes and faucet (mainnet is not live).

### Competition & advantage (200 words max)

Existing L1s generally permit any bytecode deployment. Quality assurance, when it exists, is usually application- or contract-layer rather than protocol-enforced. Boing’s approach is protocol QA: deployments that fail transparent quality criteria are restricted before they are accepted on-chain. This is not a dApp feature; it is infrastructure, designed to reduce malicious, deceptive and low-quality deployments through transparent, protocol-level checks. We add transparency by default (open specs, published tokenomics), a custom stack (Rust, HotStuff BFT, libp2p), and sustainable tokenomics (floor-triggered waves, no cliffs). We are independent of other chains—our own architecture, not a fork. Ecosystem integration via boing.finance provides cross-chain DeFi coordination. Developer tooling (CLI, SDK) and success-based dApp incentives are built in. We aim to serve users and protocols seeking an L1 with transparent, protocol-level quality checks.

### Target market & size (200 words max)

**Primary:** DeFi users, protocols, and validators seeking an L1 with protocol-level deployment checks, transparent governance and sustainable incentives. **Secondary:** Developers building on a quality-assured chain. **Market size:** Global DeFi TVL exceeds $50B across hundreds of chains. Millions of users and thousands of protocols face scam and congestion risk. We target early adopters: security-conscious protocols, validators tired of opaque ecosystems, and users burned by scams. Long-term, we aim for DeFi protocols that require compliance or quality standards. TAM includes the intersection of DeFi, quality-conscious institutions, and cross-chain liquidity—a growing segment as regulation and user expectations evolve.

### Go-to-market (200 words max)

**Phase 1—Testnet:** Public testnet with bootnodes, faucet, and RPC. Developer docs, CLI, SDK, and validator setup guides. An incentivized testnet is not currently running; we hope to run one in the future (planned as roughly 2–4 weeks, rewarding validators, developers, and users), and will announce it on our official channels. **Phase 2—Mainnet:** Security audits, formal verification, infrastructure hardening. Validator bootstrap via early incentives and hardware grants (capped). Developer grants and hackathons. boing.finance integration for cross-chain swap, bridge, and liquidity routing. **Phase 3—Ecosystem:** Success-based dApp incentives with per-dApp caps; community QA pool; governance-driven parameter evolution. Messaging: “Only quality assets on-chain. No other L1 enforces deployment standards at the protocol layer.” Channels: GitHub, Discord, docs, accelerator/VC outreach.

### How you generate value (200 words max)

BOING tokens are used for staking, governance, fees, and validator incentives. Default fee policy sends **100% of protocol gas fees to the treasury** (`PROTOCOL_TREASURY`); optional tips default **100% to the block proposer** ([OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md)). Usage drives fee revenue; validators and treasury earn from transactions. Floor-triggered waves restore balance when circulating supply reaches a floor—no reward cliffs. Transaction fees flow to validators and treasury. Treasury funds audits, grants, ecosystem growth, and infrastructure. dApp incentives (success-based, per-dApp capped) align developer success with network adoption. Cross-chain DeFi via boing.finance captures value from swap fees and liquidity. At maturity, fee revenue dominates over emissions. Emissions decline over time (Year 1 ~8%, Year 10+ targets 1% floor or 0% if fees sufficient).

### Validation steps (200 words max)

**Technical:** Core protocol implemented—consensus (HotStuff), execution (custom VM), state (Sparse Merkle), P2P (libp2p). Node binary runs in single- and multi-node modes. Developer tools (CLI, SDK) scaffold and deploy dApps. Protocol QA design includes opcode whitelist, blocklist, scam patterns; community pool for edge cases. Build and tests pass; fuzz harness for primitives. **Operational:** Testnet infrastructure designed—bootnodes, faucet, public RPC. Documentation (RUNBOOK, RPC-API-SPEC, TESTNET, READINESS) supports validators and developers. boing.network site live with docs, faucet, and investor materials. **Market:** Design aligns with user pain (scams, congestion, opacity). Six-pillar philosophy documented. Tokenomics and governance designed for sustainability. A possible future incentivized testnet ([TESTNET.md](TESTNET.md) Part 3, planning notes only; not currently running) outlines validator/developer incentives. Next: continued public testnet operation and early validator/developer feedback.

---

## Optional questions (max 1,000 characters per answer)

Use these for application forms that ask the three optional questions below. Replace **[YOU FILL]** placeholders (your background, start date) before submitting.

### 1. How do you know you can ship this product?

**Draft (≈980 characters):**

The core protocol is written and maintained by the founding team. Full codebase: **https://github.com/chiku524/boing.network** — Rust workspace (consensus, execution, state, P2P, CLI, node). We have shipped a working L1 stack from first principles: HotStuff BFT consensus, custom VM, libp2p networking, CLI (`boing init`, `boing dev`, `boing deploy`), TypeScript SDK, testnet with bootnodes and faucet, and docs (technical spec, runbook, testnet guide). **[YOU FILL: e.g. I/we have X years in distributed systems / web3 / Rust; previous roles at [companies]; built [prior products].]** **[YOU FILL: Outcome of previous companies or projects—e.g. shipped to production, users/revenue, acquisition, or open-source adoption.]** We are a small technical team that has already delivered an MVP; the roadmap (BUILD-ROADMAP.md) and READINESS checklist show what’s done and what’s next. We can ship because we have already built the hardest parts—consensus, execution, and protocol QA design—and we iterate in the open.

---

### 2. What problem are you solving? And why is it valuable?

**Draft (≈990 characters):**

**To a friend:** We’re building a blockchain where only quality, rule-compliant deployments are allowed on-chain—so fewer scams and less spam. **Ideal customer:** DeFi users and protocols who have lost trust (or funds) to malicious deployments, and validators who want transparent, sustainable incentives. **The one problem they’re desperate to solve:** “I want to use/build on a chain that doesn’t let every scam and spam contract onto the network.” **Unique insight from users:** People don’t want more filters inside apps—they want the chain itself to enforce a bar. Protocol-level QA shifts the burden from every dApp to the infrastructure. **Why now:** Regulatory and user expectations are rising; congestion and scam fatigue are acute on major L1s. Technology (efficient consensus, programmable execution, light clients) makes a quality-assured L1 feasible. We’re the first to enforce deployment standards at the protocol layer—no other L1 does this—so the timing and differentiation are clear.

---

### 3. Tell us about your progress.

**Required: Month and year started:** **[YOU FILL — e.g. March 2024 or January 2025]**

**Draft (≈970 characters):**

**Most important metric:** Protocol completeness and testnet readiness (nodes running, RPC, faucet, docs). **Progress:** Core protocol implemented: consensus (HotStuff), execution (custom VM), state (Sparse Merkle), P2P (libp2p). Node binary runs in single- and multi-node modes; CLI and TypeScript SDK support init, dev, and deploy. Testnet infrastructure is in place (bootnodes, faucet, public RPC at testnet-rpc.boing.network). Explorer (boing.observer) and wallet (boing.express) exist. Full documentation (specs, RUNBOOK, TESTNET, READINESS) supports validators and developers. **How we make money:** BOING is used for staking, governance, and fees; protocol gas fees default **100% to the treasury**; tips default to the proposer. Treasury funds audits, grants, and ecosystem growth. Cross-chain DeFi via boing.finance will generate swap/liquidity fees. **Users:** Testnet phase—early validators and developers; we hope user counts will grow with a future incentivized testnet (not currently running) and mainnet launch. We are at MVP stage with a live testnet and clear path to mainnet.

---

## [YOU FILL] — Team & founder (both programs)

**Team (200 words max):**  
**[YOU FILL]** — Founder/team backgrounds, technical expertise (blockchain, Rust, distributed systems), prior projects, why this team can ship and scale Boing.

**Why you care (200 words max):**  
**[YOU FILL]** — Founder motivation: personal experience with scams/congestion/trust in DeFi, vision for a transparent ecosystem with protocol-level quality checks, commitment to sustainability and community.

**What you hope to gain (3 months):**  
**[YOU FILL]** — e.g. mentorship on go-to-market and tokenomics, technical support/audits, validator and ecosystem introductions, help with legal/fundraising/product.

**Company incorporated?** **[YOU FILL]** Yes / No  

**Equity divide between founders?** **[YOU FILL]**  

**Have you raised capital to date?** **[YOU FILL]** Yes / No  

**Have you previously applied to Beacon?** **[YOU FILL]** Yes / No  

**How did you hear about Beacon / OV?** **[YOU FILL]**

---

## OV-specific: Form fields (copy-paste)

### Company / contact

- **Company name*** **Boing Network** (or your legal entity name)
- **Website URL** **https://boing.network**
- **First name*** **[YOU FILL]**
- **Last name*** **[YOU FILL]**
- **Email*** **[YOU FILL]**
- **Telegram** **[YOU FILL — optional]**
- **Calendly or similar** **[YOU FILL — optional]**
- **Where did you hear about us?** **[YOU FILL]**

### Pitch deck

Upload **`website/public/pdfs/Executive-Summary-Pitch-Deck.pdf`**. Generate with `npm run build:pdfs` in `website/`.

### Technical / GitHub

- **Technical documentation:** https://boing.network (docs linked from site)
- **GitHub:** **[YOU FILL]** — e.g. https://github.com/boing-network/boing (or “Available upon request”)

### Valuation / other

- **Most recent equity valuation?** **[YOU FILL]** — e.g. "Pre-seed / not yet raised", "€X post-money (Month Year)", or "N/A — bootstrapped"
- **Is there anything else you would like to tell us?** **[Optional]** — Key milestones, unique differentiator in one line, or why OV/Beacon is a fit.

---

## Form: Project name, website, deck, one-liner, blurb, traction

Use these for application forms that ask for project basics, blurb, traction, and supporting info. Fill **[YOU FILL]** where needed.

| Field | Copy-paste value |
|-------|------------------|
| **Project Name** | Boing Network |
| **Project Website** | https://boing.network |
| **Link to the deck** | https://boing.network/investors/ |
| **One-liner** | Boing Network — Quality, enforced by the protocol. |
| **Your title/role** | Founder / Developer |
| **Industry** | Web3+AI, DeFi, Infrastructure |

### One-liner (alternative, shorter)

**Option A (current):** Boing Network — Quality, enforced by the protocol.

**Option B (shorter):** An independent L1 with protocol-enforced quality assurance.

**Option C (outcome-focused):** An L1 built from first principles with transparent, protocol-level deployment checks and published tokenomics.

---

### Complete project blurb

Boing Network is an independent L1 built from first principles with **protocol-enforced quality assurance**: Boing introduces protocol-level deployment checks designed to identify and restrict deployments that fail transparent quality criteria before they are accepted on-chain. These checks run at the protocol layer rather than as app-level filters, designed to reduce malicious, deceptive and low-quality deployments through transparent, protocol-level checks. We combine this with adaptive / micro fees, published tokenomics, and cross-chain DeFi coordination (boing.finance). Account abstraction remains design-roadmap work. The stack is implemented in Rust (consensus: HotStuff BFT; execution: custom VM; networking: libp2p). Developer tools include the CLI (`boing init`, `boing dev`, `boing deploy`) and a TypeScript SDK. The public testnet is live with bootnodes, faucet, and public RPC (mainnet is not live); explorer (boing.observer) and wallet (boing.express) are available. We are building infrastructure for a sustainable DeFi ecosystem.

---

### Current traction, major milestones, partnerships

**Traction & milestones:**  
• **Core protocol shipped:** Consensus (HotStuff BFT), execution (custom VM), state (Sparse Merkle), P2P (libp2p)—Rust workspace, open source (GitHub).  
• **Testnet live:** Bootnodes, faucet, public RPC (testnet-rpc.boing.network); single- and multi-node modes.  
• **Developer tooling:** CLI for init, dev, and deploy; TypeScript SDK; comprehensive docs (technical spec, RUNBOOK, TESTNET, READINESS).  
• **Ecosystem apps:** Block explorer (boing.observer), non-custodial wallet (boing.express).  
• **Website & materials:** boing.network live with docs, faucet, and investor deck (boing.network/investors/).  
• **Validator integration:** Node specs, release packages and command templates documented for operators (public node listing API).

**Partnerships / ecosystem:**  
• Testnet infrastructure in place for validator onboarding; a possible future incentivized testnet (not currently running) and mainnet migration path are planned. Cross-chain DeFi (boing.finance) planned for post-mainnet. No formal partnerships to announce yet; seeking accelerator support for go-to-market, validator intros, and ecosystem development.

---

### Any more supporting information?

**Suggested (optional):**  
GitHub: https://github.com/chiku524/boing.network — full protocol codebase, MIT-licensed. No other L1 enforces deployment standards at the protocol layer; we are uniquely positioned for security-conscious DeFi users, protocols, and validators. Open to sharing technical deep-dives or a live testnet walkthrough.

*(Or leave blank if the form has no space.)*

---

### Are you working on this project full-time?

**[YOU FILL]** — Yes / No

*(Answer according to your situation. If yes, say so; if part-time or transitioning, you can add one line in “Any more supporting information?” e.g. “Transitioning to full-time upon [milestone/funding].”)*

---

## Form: Valuation, fundraising, TGE

Use these for application forms that ask about valuation, past/current fundraising, and token generation. Replace **[YOU FILL]** with your actual numbers or choices.

| Field | Copy-paste value |
|-------|------------------|
| **Current Project Valuation (USD)** | **[YOU FILL]** — e.g. *Pre-seed / not yet valued*, or *$X (post-money)* if you have a round. |
| **Has your project secured any fundraising in the past? (including grants)** | **[YOU FILL]** — Yes / No |
| **If so, how much have you raised? (USD)** | **[YOU FILL]** — e.g. *$0*, *$X*, or *N/A—no prior raise* |
| **Please list previous key backers/investors** | **[YOU FILL]** — e.g. *None*, or list names (e.g. *[Fund A], [Angel B], [Grant C]*). |
| **Are you looking to fundraise currently or in the near future?** | **[YOU FILL]** — Yes / No |
| **How much are you raising? (USD)** | **[YOU FILL]** — e.g. *$X* or *TBD / discussing with accelerator* |
| **Which round/stage is this fundraise?** | **[YOU FILL]** — e.g. *Pre-seed*, *Seed*, *Strategic*, *Accelerator grant* |
| **Will your project TGE?** | **[YOU FILL]** — A / B / C / D (see below) |
| **What is your TGE plan?** | **[YOU FILL]** — Short paragraph (draft below) |

---

### Current Project Valuation (USD)

**[YOU FILL]**  

*If you have not raised and have no formal valuation, use one of:*  
- *Pre-seed; no formal valuation yet*  
- *Bootstrapped; no prior round*  
- *Or a number if you have a cap/SAFE (e.g. $2M pre-money)*

---

### Has your project secured any fundraising in the past? (including grants)

**Yes** / **No**

*If No:* leave “How much” and “Key backers” blank or write *N/A*.

---

### If so, how much have you raised? (USD)

**[YOU FILL]** — e.g. *$0*, *$50,000*, *$500,000*

*Only if “Has your project secured any fundraising” = Yes.*

---

### Please list previous key backers/investors

**[YOU FILL]**  

*Examples:*  
- *None.*  
- *[Fund name], [Angel name], [Grant program name].*  
- *Grant from [Program] ($X).*

---

### Are you looking to fundraise currently or in the near future?

**Yes** / **No**

*Typical for accelerator: **Yes** — seeking grant, pre-seed, or seed as part of / after the program.*

---

### How much are you raising? (USD)

**[YOU FILL]** — e.g. *$300,000*, *$500K–$1M*, *TBD*, *Seeking accelerator grant first, then [round]*  

*Align with “Which round/stage” below.*

---

### Which round/stage is this fundraise?

**[YOU FILL]**  

*Examples:* *Pre-seed*, *Seed*, *Strategic*, *Accelerator / grant*, *Pre-seed with option to extend to seed*.

---

### Will your project TGE?

Choose **one** of:

| Option | Label | When to use |
|--------|--------|-------------|
| **A** | Yes | We plan a mainnet token launch (TGE) at or after mainnet. |
| **B** | No | We will not have a public token (rare for an L1). |
| **C** | Token is already live! | Only if BOING is already live on **mainnet** (testnet doesn’t count). |
| **D** | Undecided | Timing/format not yet fixed; will decide with advisors/accelerator. |

*For Boing: **A (Yes)** or **D (Undecided)** are the usual choices—BOING is the native chain token; testnet BOING exists; mainnet TGE would be at/after mainnet launch.*

---

### What is your TGE plan?

**Draft (copy-paste and edit as needed):**

BOING is the native token of Boing Network (staking, governance, fees). Testnet BOING is live for faucet and validator testing. **Mainnet TGE** is planned at or shortly after mainnet launch: initial distribution aligned with our tokenomics (emission, validator rewards, treasury, community/ecosystem). We will follow a phased approach—if a future incentivized testnet is run (none is running today), its participants might qualify for mainnet recognition under rules to be published—and will ensure compliance with applicable regulations. Exact date and mechanics will be set after security audits and mainnet readiness; we are open to accelerator/advisor input on timing and structure.

*Short version (if character limit):*  
*BOING is the native token; testnet live. Mainnet TGE at or after mainnet launch, with distribution per tokenomics and compliance. Timing to be finalized post-audits and with accelerator/advisor input.*

---

*Source slide content for the pitch deck is in [Executive-Summary-Pitch-Deck.md](Executive-Summary-Pitch-Deck.md).*
