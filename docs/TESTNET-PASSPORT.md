# Testnet Passport

Boing-native contribution record for public testnet **6913** (Guild-like, no third-party service). Stage 1 shipped 2026-11-03.

> Verified activity **may be considered** for any future program. It is **not** a promise of tokens, an airdrop, or a mainnet BOING claim.

## Rules

- Single source of truth: `website/src/config/passport-rules-v1.json` (rules **v1.0.0**). The quests page, the submit API allowlist and the D1 seed all read it.
- Public copies: `https://boing.network/testnet/passport/rules-v1.json` (static, copied at build by `scripts/copy-passport-rules.mjs`) and `GET /api/passport/rules`.
- Policy: badges and tiers; credit from genesis (activity before registering counts once you register); one account can hold several tracks; verified socials optional (needed only for Gold); weekly snapshots; public tier-ordered leaderboard with shortened addresses; validator uptime by sampling; on-chain attestations deferred.

## Tracks

`user` (Users), `developer` (Builders), `node_operator` (Validators). Registering for another track adds it (`portal_tracks`); the most recent registration is the primary `role`. Every track can submit every quest.

## Quests

| Quest ID | Name | Tracks | Verification |
|----------|------|--------|--------------|
| `faucet` | First drip | Users | Auto (RPC) |
| `first_tx` | First transaction | Users, Builders, Validators | Auto (RPC) |
| `express_wallet` | Open Express | Users | Manual review |
| `finance_swap` | Try Finance | Users | Auto (RPC) |
| `observer_lookup` | Look up on Observer | Users | Manual review |
| `first_deploy` | Ship a contract | Builders | Auto (RPC) |
| `validator_connect` | Join the validator set | Validators | Auto (RPC) |
| `feedback` | Share feedback | Users, Builders, Validators | Manual review |
| `social` | Join community | Users, Builders, Validators | Manual review |

### Auto verification (read-only RPC, `functions/api/portal/_lib/passportVerify.js`)

Reads up to `max_history_pages` × 100 txs from `boing_getAccountTransactions`; only successful txs count.

- `faucet` — a transfer from the faucet account to you.
- `first_tx` — any tx you sent.
- `first_deploy` — a `contract_deploy*` you sent (it passed QA to be committed).
- `finance_swap` — a call (or batch step) you sent to a canonical DEX contract (factory, routers, multihop, LP vaults, custody router) or any pool from `boing_listDexPools`.
- `validator_connect` — you are in `boing_getValidators` (id or owner) or sent `validator_register`.

`POST /api/quests/submit` stores auto quests as `verification_status='verified'`, `verified_by='rpc'`, proof = tx id; if the chain shows nothing it returns `422 not_verified` and stores nothing. Manual quests are stored `pending` until reviewed. `GET /api/passport/check?account_id_hex=0x…` is a dry run (no writes).

## Data (D1 `boing-network-db`)

Migration `website/migrations/2026-11-03-testnet-passport-stage1.sql`: `portal_tracks` (backfilled from `portal_registrations.role`), `quest_completions.verification_status` / `verified_by`, quest rows upserted from the rules.

## Stage 2 — credibility indexer (shipped 2026-11-04, rules 1.1.0)

Worker `boing-credibility-indexer` (`workers/credibility-indexer`, cron `*/10 * * * *`, Free plan) writes to D1:
`passport_signals`, `passport_badges`, `passport_tiers`, `validator_uptime_samples`, `passport_snapshots`, `indexer_state`
(migration `website/migrations/2026-11-04-testnet-passport-stage2.sql`).

- Registered accounts are refreshed stale-first (≤4 per tick, hourly) from `boing_getAccountTransactions` (≤10 pages = most recent 1000 txs).
- Activity before registration is credited and flagged `historic`. Badges are permanent once earned.
- Validator uptime: one `boing_getValidators` sample per validator per tick; badges use the average of in-set samples.
- Weekly snapshot: first tick of each ISO week → JSON of (account, track, tier, score) + sha256 Merkle root (`GET /api/passport/snapshots/latest`).
- Tiers per track (see `tiers` in the rules JSON): Bronze / Silver / Gold = badge count + required badges; Gold also needs a verified social link.

APIs: `GET /api/passport/profile?account_id_hex=`, `GET /api/passport/leaderboard?track=`, `GET /api/passport/snapshots/latest`,
`GET /api/portal/operator-leaderboard` (now from uptime samples + live validator set).

## Stage 3 — UI (shipped 2026-11-04)

`/testnet/passport` (profile: tiers, progress, badges) and `/testnet/passport/leaderboard` (per-track, tier-ordered, shortened addresses).

## Stage 4 (planned)

- Discord OAuth linking (verified social → unlocks Gold); optional Discord role-sync bot (tier → role).
- GitHub/X links, sybil heuristics (funding graph, faucet-only accounts), reviewer queue for manual quests.
