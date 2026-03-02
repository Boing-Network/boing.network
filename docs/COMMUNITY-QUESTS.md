# Community Quests — Incentivized Testnet

> **Purpose:** Define and implement community quests for the incentivized testnet so users can earn rewards by completing on-chain and off-chain tasks.  
> **References:** [INCENTIVIZED-TESTNET.md](INCENTIVIZED-TESTNET.md), [TESTNET.md](TESTNET.md), [TESTNET-PORTAL.md](TESTNET-PORTAL.md), [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md).
>
> **Portal:** Quests are part of the **Testnet Portal** under **Users**: [/testnet/users](https://boing.network/testnet/users). The standalone `/network/quests` page redirects to the portal.

---

## 1. Overview

**Community quests** are user-facing tasks (e.g. “Use the faucet”, “Send a transaction”, “Share feedback”) that qualify participants for testnet rewards. Completion can be:

- **Auto-verifiable on-chain** — e.g. faucet receipt, first tx; we query RPC or D1 indexer.
- **Manual with proof** — user submits account ID + proof (tx hash, link, Discord handle); team verifies later.

Publish a single **Quests** page (e.g. `/network/quests`) listing tasks, verification type, and reward eligibility. Collect submissions via a form (website form → API → D1, or external form + spreadsheet) and process them before testnet end.

---

## 2. Quest Types and Examples

| Quest ID | Name | Description | Verification | Reward tier |
|----------|------|--------------|--------------|-------------|
| `faucet` | First drip | Request testnet BOING from the faucet | On-chain: faucet tx or balance > 0 | Base user |
| `first_tx` | First transaction | Send any transaction on testnet | On-chain: account has nonce ≥ 1 or tx in block | Base user |
| `validator_connect` | Join the network | Run a node connected to testnet bootnodes | Manual: submit node ID / multiaddr or screenshot | Validator track |
| `feedback` | Share feedback | Answer 3–5 short questions (UX, docs, bugs) | Manual: form submission + optional account ID | Bonus |
| `social` | Join community | Join Discord and post in #testnet-intros | Manual: Discord handle + account ID | Bonus |
| `docs` | Read and confirm | Visit Getting Started + Testnet docs, confirm checkbox | Manual: form with “I have read” + account ID | Base user |

**Recommendation for Phase 1:** Start with 3–5 quests: `faucet`, `first_tx`, `feedback`, and optionally `social` and `validator_connect`. Add more in a second phase if you run a longer testnet.

---

## 3. Verification Methods

### 3.1 On-chain (auto)

- **Faucet:** After user requests from faucet, either (a) call testnet RPC to check balance for `account_id`, or (b) if you index faucet txs in D1, query by `to = account_id` and `method = faucet`.
- **First tx:** RPC `eth_getTransactionCount(account_id, "latest")` > 0, or D1 query for any tx where `from = account_id`.

Use a **Cloudflare Worker** (or cron) that:

1. Reads pending submissions from D1 where `verification_type = 'on_chain'` and `verified_at IS NULL`.
2. For each, calls testnet RPC or D1 explorer tables.
3. Updates `verified_at` and `proof_value` (e.g. tx hash) on success.

### 3.2 Manual (proof submitted)

- User submits **account ID (32-byte hex)** plus **proof**: tx hash, Discord username, link to screenshot, or form answers.
- Team (or a simple admin UI) reviews and sets `verified_at` and optionally `rejected_reason`.

Store in D1: `quest_id`, `account_id`, `proof_type`, `proof_value`, `submitted_at`, `verified_at`, `rejected_reason`.

---

## 4. Data Model (D1)

Use the same D1 database as the rest of the site (e.g. `boing-network-db`). Run the migrations below when you’re ready to enable the quest backend.

```sql
-- Quests definition (static; can be seeded once)
CREATE TABLE IF NOT EXISTS quests (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  verification_type TEXT NOT NULL, -- 'on_chain' | 'manual'
  reward_tier TEXT,                -- 'base' | 'validator' | 'bonus'
  active INTEGER DEFAULT 1,
  created_at TEXT
);

-- User submissions and verification state
CREATE TABLE IF NOT EXISTS quest_completions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  quest_id TEXT NOT NULL,
  account_id_hex TEXT NOT NULL,
  proof_type TEXT,                 -- 'tx_hash' | 'discord' | 'form' | 'screenshot_url' | null if auto
  proof_value TEXT,
  submitted_at TEXT NOT NULL,
  verified_at TEXT,                -- set when verified (auto or manual)
  rejected_reason TEXT,
  FOREIGN KEY (quest_id) REFERENCES quests(id)
);

CREATE INDEX IF NOT EXISTS idx_quest_completions_account ON quest_completions(account_id_hex);
CREATE INDEX IF NOT EXISTS idx_quest_completions_quest ON quest_completions(quest_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_quest_completions_unique ON quest_completions(quest_id, account_id_hex);
```

---

## 5. API (Cloudflare Worker)

If you add a Worker in front of Pages or use `functions/` for serverless routes:

| Method | Path | Purpose |
|--------|------|--------|
| GET | `/api/quests` | List active quests (id, name, description, verification_type, reward_tier). |
| POST | `/api/quests/submit` | Submit a completion: `{ "quest_id", "account_id_hex", "proof_type?", "proof_value?" }`. Validate account_id format; insert into `quest_completions`; return `{ "ok": true, "id" }`. Rate limit per IP/account. |
| GET | `/api/quests/status?account_id_hex=0x...` | Return list of quest completions for that account (for “My progress” UI). Optional: only if you want to show status without revealing full DB. |

For **on-chain verification**, either:

- A **scheduled Worker** (Cron Trigger) that periodically runs the verification step and updates `verified_at`, or
- Verification on-demand when the user clicks “Check status” and you call RPC and then update D1.

---

## 6. Website UI

### 6.1 Quests page: `/testnet/users` (Testnet Portal — Users)

- **Content:**
  - Short intro: “Complete quests to qualify for testnet rewards. Rewards and caps are defined in the [Incentivized Testnet Rules](/network/incentivized-rules).”
  - List of quests (from config or `/api/quests`) with: name, description, “How to complete”, “Verification: Automatic / Manual”.
  - For each quest:
    - If **auto:** “Use the [faucet](/network/faucet) then come back and click ‘Verify’.” → button that calls API to trigger verification (or “Check status”).
    - If **manual:** “Submit your account ID and proof below.” → form (account ID + proof field) that POSTs to `/api/quests/submit`.
  - Optional: “My progress” section (if you implement `/api/quests/status`) showing completed quests and pending verification.

### 6.2 Without backend (Phase 0)

If you don’t have the Worker/D1 quest tables yet:

- Publish a **static** `/network/quests` page that lists the same quests and explains how to complete them.
- For **manual** quests: link to an **external form** (Google Form, Typeform, Discord form) that collects:
  - Account ID (32-byte hex)
  - Quest ID (or “Which quest(s) did you complete?”)
  - Proof (tx hash, Discord handle, or short feedback text).
- Export responses to a spreadsheet; at testnet end, deduplicate by account and assign rewards per your published rules. Optionally run RPC checks for `faucet` / `first_tx` by looking up each account ID.

---

## 7. Incentive Rules and Caps

- Define in a single **Incentivized Testnet Rules** page (e.g. `/network/incentivized-rules`):
  - Which quests are live and their reward tier (base / bonus).
  - Cap per user (e.g. max N bonus rewards per account).
  - That “quest completion” qualifies for Community & Grants pool rewards as described in INCENTIVIZED-TESTNET.md.
- Link to this page from the Quests page and from announcements.

---

## 8. Implementation Checklist

- [ ] **Phase 0 (no backend):** Add static `/network/quests` page; define 3–5 quests in copy; add link to external form for manual submissions; document in INCENTIVIZED-TESTNET.md that “quest submissions” are collected via form + spreadsheet.
- [ ] **Phase 1 (backend):** Add D1 tables (`quests`, `quest_completions`); seed `quests`; implement GET `/api/quests` and POST `/api/quests/submit`; add form on Quests page that POSTs to API; optional GET `/api/quests/status`.
- [ ] **Phase 2 (auto-verify):** Implement on-chain verification (scheduled or on-demand) for `faucet` and `first_tx`; update `verified_at` in D1.
- [ ] **Launch:** Publish Incentivized Testnet Rules; link Quests from testnet hub and Reddit/announcements; at testnet end, export completions and distribute rewards per rules.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
