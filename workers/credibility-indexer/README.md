# boing-credibility-indexer

Testnet Passport stage 2 Worker (Cloudflare Free plan). Every 15 min, via `POST /tick` from boing-native-dex-indexer's `*/15` cron (service binding; the account is at the Workers Free cap of 5 crons). Not publicly routable.

Each tick:
1. `boing_getValidators` → one row per validator in `validator_uptime_samples` (pruned after 30 days).
2. `boing_listDexPools` → DEX contract set.
3. Up to `ACCOUNTS_PER_TICK` (4) registered accounts not refreshed in `REFRESH_MINUTES` (60), stale-first:
   `boing_getAccountTransactions` (≤10 pages each, ≤`PAGE_BUDGET`=36 pages per tick) → `passport_signals`,
   `passport_badges` (first earned_at wins; `historic=1` when earned before registration), `passport_tiers`.
4. First tick of each ISO week with ranked accounts → `passport_snapshots` (JSON + sha256 Merkle root).
5. `indexer_state.credibility_last_tick` (also served at `GET /health`).

Scoring is shared with the Pages API: `website/functions/api/portal/_lib/passportScore.js`; rules in
`website/src/config/passport-rules-v1.json`.

Budget: ≤ ~40 subrequests/tick, ~10–40 D1 row writes/tick (≈2–6k/day), CPU dominated by JSON parse (~0.3 ms/page); ~96 ticks/day.

Deploy: `.github/workflows/credibility-indexer-worker.yml` on push, or `npm ci && npx wrangler deploy`.
Tables: `website/migrations/2026-11-04-testnet-passport-stage2.sql`.

## Discord role sync (stage 4)

No-op until configured. Secret: `npx wrangler secret put DISCORD_BOT_TOKEN`. Vars (dashboard, kept across deploys by `keep_vars = true`):
`DISCORD_GUILD_ID`, `DISCORD_ROLE_BRONZE`, `DISCORD_ROLE_SILVER`, `DISCORD_ROLE_GOLD`, optional `DISCORD_ROLE_<TRACK>_<TIER>`.
At most `DISCORD_MAX_CALLS` (8) role calls per tick; stops on 429. See `src/discord.js` and `docs/TESTNET-PASSPORT.md`.
