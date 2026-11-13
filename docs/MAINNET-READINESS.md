# Mainnet readiness (go / no-go)

**Audience:** Nico / Boing ops  
**Date:** 2026-11-13  
**Status:** Working checklist — not a launch calendar  
**Criteria:** Project store `docs/mainnet-launch-criteria.md` (six must-pass gates)  
**Scorecard (detailed evidence):** Project store `docs/mainnet-readiness-scorecard.md` — keep in sync when this file changes.

This is the in-repo operator checklist. Scores below are **current** as of the date above against public testnet + shipped `main` code. **Do not invent tokenomics** — open economics stay on BN-155.

---

## Current scorecard (summary)

| # | Gate | Score | Blocking? |
|---|------|-------|-----------|
| 1 | Protocol trust | **Partial** | Yes — H2/H10/M6 **closed** this pass; still M9/M13 + upgrade owners; H5 origin gate **closed** |
| 2 | Economic finality | **Fail** | Yes — BN-155 open items (genesis pie, vesting, fee path, emission floor, year-1 block emission) |
| 3 | Operational readiness | **Partial** | Yes — ops pack drafted; live keys / named on-call / scheduled freeze / alerts / multi-validator still open; testnet ops strong |
| 4 | Thin product path | **Partial** | Soft — Express / Finance / Observer / network live on **testnet**; mainnet RPC intentionally unset |
| 5 | Bootstrap plan | **Partial** | Soft — bridge live; Chainquiry live; mainnet liquidity / incentives undecided |
| 6 | Legal / reputational | **Partial** (note only) | Nico comfort + any counsel review — agents do not invent legal advice |

**Overall:** **No-go** for mainnet until gates **1–3** are green (or residuals explicitly accepted in writing) **and** BN-155 locks genesis-touching economics.

---

## Gate 1 — Protocol trust

### Go criteria

- [x] Issue #61 Critical consensus/runtime blockers fixed or activation-gated on `main`
- [x] High pass H1–H10 code/docs shipped; live tip has mainnet rate profile, blob get-auth, ACL height **1317**, QA v1–v7 blocklist
- [x] Public origin gate live (`BOING_ORIGIN_GATE_TOKEN` / `X-Boing-Origin-Gate`) — naked `*.fly.dev` JSON-RPC **401** (H5 closed via companion gate + Access break-glass)
- [x] Stake-mode **liveness evidence** tx (H2) shipped activation-gated ([LIVENESS-SLASH-ACTIVATION.md](LIVENESS-SLASH-ACTIVATION.md)) — Closed 2026-11-12
- [x] Second validator / dual-proposer soak (H10) — not BFT HA (`n ≥ 4`); see [SECOND-VALIDATOR-RUNBOOK.md](SECOND-VALIDATOR-RUNBOOK.md) — Closed 2026-11-12
- [x] M6 Blake3 / governance **deferred past genesis** (null heights; owner Nico) ([BLAKE3-AND-GOVERNANCE-ACTIVATION.md](BLAKE3-AND-GOVERNANCE-ACTIVATION.md)) — Closed 2026-11-12
- [ ] M9 ACL-in-SMT accepted as best-effort privacy **or** designed
- [ ] Upgrade / emergency path named (halt owners, client pin versions) for **mainnet** specifically

### Residual risk list (intentional / open)

| ID | Residual | Disposition |
|----|----------|-------------|
| **H2** | Stake-mode liveness slash | **Closed** (2026-11-12) — `ReportLiveness` #40 activation-gated; [LIVENESS-SLASH-ACTIVATION.md](LIVENESS-SLASH-ACTIVATION.md) |
| **H5** | Fly origin bypass | **Closed** (2026-11-08) — origin gate + Access break-glass |
| **H6** | Permissionless `register_pair` | Uniqueness on-chain; no owner/bond — product may accept for launch |
| **H10** | Single hosted validator residual | **Closed** (2026-11-12) as second validator live / dual-proposer soak — **not** BFT HA until `n ≥ 4`; [SECOND-VALIDATOR-RUNBOOK.md](SECOND-VALIDATOR-RUNBOOK.md) |
| **M6** | Governance / Blake3 activation heights | **Closed** (2026-11-12) deferred past genesis / null until Nico height; [BLAKE3-AND-GOVERNANCE-ACTIVATION.md](BLAKE3-AND-GOVERNANCE-ACTIVATION.md) |
| **M8** | Download-proof nonce cache | Process-local only (restart resets) |
| **M9** | ACL not in account SMT | Best-effort privacy until designed |
| **M11** | Indexer `/` + `/stats` RPC amplification | Soft RPM limit shipped on Worker; prefer D1 `/v1/directory/*` |
| **M13** | Ledger/multihop custody | Prefer custody routers for user swaps |

---

## Gate 2 — Economic finality

### Locked (do not re-open casually)

- **1B BOING** genesis base / `INITIAL_SUPPLY`
- **Uncapped** ongoing emissions after genesis (BN-155 supply ceiling decision — shipped)

### Still open (BN-155 — Nico)

1. Genesis pie (55/30/15 vs 50/20/15/15 + Team)
2. Team % / equal Nico–partner split / cliff & vesting
3. Community sub-pots (quest / bounty / grants)
4. Mainnet day-one fee path (treasury-default gas vs schedule to 80/20)
5. Emission floor (1% vs fee-triggered 0%)
6. Year-1 absolute BOING/block + decay constants
7. Incentivized-testnet framing vs ≥2 validators
8. Bug-bounty intake + severity table
9. Rules-page / claim-language review before any “rewards live”

**Agents must not invent these numbers.**

---

## Gate 3 — Operational readiness

### Already strong on public testnet

- [x] Public RPC: `https://testnet-rpc.boing.network/` (Worker → Fly + origin gate)
- [x] `BOING_RATE_PROFILE=mainnet` on hosted validator
- [x] RUNBOOK monitoring + incident sections
- [x] Hybrid heavy-ops + Fly origin lockdown runbooks (Project store + `docs/FLY-IO.md`)
- [x] Faucet refused on chain id **6914** without operator token / faucet flag

### Ops pack (Project store — BN-221)

Checklists only; **not executed**. Do **not** rotate live testnet secrets or enable Workers Paid from this pack.

| Topic | Project store doc |
|-------|-------------------|
| Key ceremony + offline backup (nkc-desktop OK) | `docs/mainnet-key-ceremony-checklist.md` |
| Freeze window (what / how long / thaw / allowed PRs) | `docs/mainnet-freeze-window.md` |
| Week-1 on-call channel + SEV + first hour → RUNBOOK §5–6 / §8 | `docs/mainnet-oncall-week1.md` |
| Scorecard gate 3 links | `docs/mainnet-readiness-scorecard.md` |

### Still missing for mainnet day-0

- [ ] Mainnet validator / treasury / operator **keys** generated, backed up offline, never in git (ceremony checklist ready; date / custodians = **Nico**)
- [ ] Named **on-call** + week-1 incident channel (template ready; create Discord/Slack + name roster = **Nico**)
- [ ] External monitoring/alerts (height stall, gateway health, Free Worker 1027 cliff)
- [ ] Mainnet RPC hostname + Access/gate plan (mirror testnet pattern; do **not** enable Workers Paid from this checklist)
- [x] Second testnet validator path executed (H10 closed 2026-11-12; still not BFT HA) ([SECOND-VALIDATOR-RUNBOOK.md](SECOND-VALIDATOR-RUNBOOK.md))
- [ ] Freeze window **scheduled and executed** (procedure below + store freeze doc; calendar = **Nico**)

---

## Gate 4 — Thin product path

| Surface | Testnet | Mainnet intent |
|---------|---------|----------------|
| Express wallet | Live (CWS / site) | Chain id **6914** reserved in apps; mainnet RPC unset until genesis |
| Finance DEX / bridge | Live; LI.FI `hasApiKey: true`, fee recipients set | Keep “mainnet not live” copy until cutover |
| Observer | Live; graceful fallback if mainnet RPC unset | `NEXT_PUBLIC_MAINNET_RPC` only when distinct endpoint exists |
| Network site / faucet / docs | Live | Docs + `/api/networks` must list mainnet only after genesis |

---

## Gate 5 — Bootstrap plan

- [x] In-app bridge fee model live (0.5% + portal wallets)
- [x] Chainquiry discovery badge on network landing
- [ ] Mainnet liquidity / inventory policy (if any) decided
- [ ] Optional builder incentives timed **at or after** mainnet (not a substitute for gates 1–4)

---

## Gate 6 — Legal / reputational (note only)

- Permanent public ledger + public RPC abuse surface + real-money support load: **Nico comfort** required.
- No agent-authored legal advice. Flag outstanding rename / hard-fork-economics surprises before genesis messaging.

---

## Freeze window (pre-genesis)

Suggested ritual (from launch criteria). Full procedure: Project store `docs/mainnet-freeze-window.md`.

1. Freeze **economics / consensus parameters** for **N ≥ 7 days** before genesis (no thrash on BN-155 locks).
2. Freeze **CREATE2 / QA registry** changes that would invalidate published mainnet addresses.
3. Tag client pins: Express min version, `@boing-network/boing-sdk` recommended, Observer build with mainnet RPC.
4. Publish week-1 incident channel + builder onboarding the same day as genesis (channel template: Project store `docs/mainnet-oncall-week1.md`).

**Thaw / exception approver:** Nico (name a backup before freeze starts). **Dates:** not scheduled until BN-155 + gate 1–2 residuals allow.

**PRs during freeze:** Critical/High security, testnet-only ops, clarifying docs, and CI/tooling are allowed; economics reopen and address-invalidating CREATE2/QA changes are not. See the store freeze doc for the full allow/deny list.

---

## Quick live probes (testnet — not mainnet)

```bash
curl -fsS https://testnet-rpc.boing.network/__gateway/health
curl -fsS -A boing-sdk/json-rpc -X POST https://testnet-rpc.boing.network/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]}'
# Naked Fly JSON-RPC should 401 without X-Boing-Origin-Gate:
curl -sS -o /dev/null -w '%{http_code}\n' -A boing-sdk/json-rpc -X POST https://boing-testnet-1.fly.dev/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]}'
```

---

## Related docs

- [RUNBOOK.md](RUNBOOK.md) — node ops, monitoring, incidents  
- [FLY-IO.md](FLY-IO.md) — hosted testnet + origin gate  
- [TESTNET.md](TESTNET.md) — public testnet honesty bounds  
- [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) — H2 liveness slash and roadmap  
- [LIVENESS-SLASH-ACTIVATION.md](LIVENESS-SLASH-ACTIVATION.md) — H2 blocker + patch path  
- [SECOND-VALIDATOR-RUNBOOK.md](SECOND-VALIDATOR-RUNBOOK.md) — H10 companion / path toward HA  
- [BLAKE3-AND-GOVERNANCE-ACTIVATION.md](BLAKE3-AND-GOVERNANCE-ACTIVATION.md) — M6 one-step checklist  
- [BOING-INITIAL-TOKENOMICS.md](BOING-INITIAL-TOKENOMICS.md) — economics source (open items → BN-155)  
- Project store (Cursor): `docs/mainnet-key-ceremony-checklist.md`, `docs/mainnet-freeze-window.md`, `docs/mainnet-oncall-week1.md`, `docs/mainnet-readiness-scorecard.md`  

