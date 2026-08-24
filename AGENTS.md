## Learned User Preferences

- Treat developer experience as a primary constraint: prefer integration paths that feel as straightforward as common EVM-style flows when the protocol allows, including clear deploy and wiring steps for dApps.
- Keep `/docs`, website pages, and API documentation aligned with what is actually shipped; documentation drift should be fixed alongside code changes.
- Expect periodic requests for thorough cleanups: remove unused or redundant code and files, and merge overlapping markdown only when it clearly reduces duplication without losing needed detail.
- After substantive work, the user often wants changes validated, then committed and pushed so deployments and downstream repos can pick them up.

## Learned Workspace Facts

- `docs/HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md` is the written source of truth for native DEX directory operations and protocol follow-ups; the in-repo worker is `workers/native-dex-indexer` with the public JSON directory routes described there (for example `/v1/directory/*`).
- `docs/HANDOFF_Boing_Network_Global_Token_Discovery.md` captures the global DEX discovery RPC surface (listing pools and tokens with pagination, token detail, indexer alignment notes, and related acceptance expectations); `boing_getNetworkInfo` exposes `developer.dex_discovery_methods` so clients can discover L1 DEX directory JSON-RPC names alongside the method catalog.
- Tutorial-style pool and native DEX bootstrap flows live under `examples/native-boing-tutorial/scripts/`; operators run these against a live RPC. The `deploy-native-dex-full-stack.mjs` orchestrator defaults `register_pair` when `BOING_BOOTSTRAP_REGISTER_PAIR` is unset, can include ledger router v1 when `BOING_FULL_STACK_INCLUDE_LEDGER_V1=1`, and kickstarts reserves via vault `deposit_add` or pool `add_liquidity` (skip with `BOING_FULL_STACK_SKIP_SEED=1`). The `boing-node` binary does not run that orchestrator on startup and does not automatically create pools or seed reserves on first start.
- The tutorial package depends on `file:../../boing-sdk`; after SDK source or public export surface changes, run `npm run build` in `boing-sdk/` so Node resolves a fresh `dist/` (stale builds can surface as missing named exports from `boing-sdk`).
- Contract deploy QA (`crates/boing-qa`) can reject bytecode via governance-configured deploy-bytecode hash lists and byte-pattern checks, and optional metadata content rules on deploy `asset_name` / `asset_symbol`; default registries ship with empty lists until configured.
- Browser-facing apps that call the public RPC over HTTPS require correct CORS configuration on the RPC or its tunnel or proxy; missing headers commonly show up as blocked fetches from sites like the explorer.
- Canonical public testnet RPC is `https://testnet-rpc.boing.network/` via `workers/public-rpc-gateway` (Cloudflare Worker failover to Fly `boing-testnet-1` / `boing-testnet-2`). Do not point that hostname at a home Cloudflare Tunnel. Hosted bootnodes are `/ip4/169.155.48.188/tcp/4001` and `/ip4/109.105.220.118/tcp/4001` ([docs/FLY-IO.md](docs/FLY-IO.md)).
- VibeMiner-related network listing and operator expectations are documented in `docs/VIBEMINER-INTEGRATION.md`.
