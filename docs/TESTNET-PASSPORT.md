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

Worker `boing-credibility-indexer` (`workers/credibility-indexer`, every 15 min via a service binding from boing-native-dex-indexer's cron (Free plan 5-cron cap)) writes to D1:
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

## Stage 4 — Discord (shipped 2026-11-04, disabled until configured)

**Linking (Pages Functions).** `POST /api/passport/discord/start` (portal password or signed `Authorize Boing Portal` message, action `discord_link`)
creates a single-use 10-minute OAuth state in D1 `discord_oauth_states`, bound to the Boing account and to the browser via an HttpOnly cookie,
and returns the Discord authorize URL (scope `identify` only). `GET /api/passport/discord/callback` checks state + cookie, exchanges the code,
reads `/users/@me`, revokes the token, and stores `discord_links (account_id_hex, discord_id UNIQUE, discord_username, linked_at, ...)`.
One Discord account can link to one Boing account. `POST /api/passport/discord/unlink` (action `discord_unlink`) deletes the link and queues
removal of synced roles. The public profile shows a masked username only. A link counts as the verified social required for Gold (rules 1.2.0).

**Role sync (credibility indexer tick, no extra cron).** With `DISCORD_BOT_TOKEN` + `DISCORD_GUILD_ID` + `DISCORD_ROLE_*` set, each tick makes at most
`DISCORD_MAX_CALLS` (8) sequential role add/remove calls, stops on HTTP 429, and only touches roles it assigned (`discord_links.roles_synced`).
`DISCORD_ROLE_BRONZE/SILVER/GOLD` = highest tier across tracks; optional `DISCORD_ROLE_<USER|DEVELOPER|NODE_OPERATOR>_<TIER>` per track.
Members not in the guild are retried every 6 h. Roles must be created by an admin; the bot role must sit above them and have Manage Roles.

**Configuration**

| Where | Name | Kind |
|---|---|---|
| Pages project `boing-network` (Production) | `DISCORD_CLIENT_ID` | variable |
| Pages project `boing-network` (Production) | `DISCORD_CLIENT_SECRET` | secret |
| Pages project `boing-network` (optional) | `DISCORD_REDIRECT_URI` | variable (default `https://boing.network/api/passport/discord/callback`) |
| Worker `boing-credibility-indexer` | `DISCORD_BOT_TOKEN` | secret (`wrangler secret put`) |
| Worker `boing-credibility-indexer` | `DISCORD_GUILD_ID`, `DISCORD_ROLE_BRONZE`, `DISCORD_ROLE_SILVER`, `DISCORD_ROLE_GOLD` (+ optional per-track) | variables (dashboard; `keep_vars = true`) |

Discord developer portal → OAuth2 → Redirects: `https://boing.network/api/passport/discord/callback`.

## Later

- GitHub/X links, sybil heuristics (funding graph, faucet-only accounts), reviewer queue for manual quests.
