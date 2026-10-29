# Boing Network — Website Specification and Deployment

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


> **Domain:** boing.network  
> **Hosting:** Cloudflare Pages  
> **Backend:** Cloudflare Workers + D1 + R2 + KV

---

## Part 1: Website Specification

### Design theme: Outerspace-Oceanic

All official Boing websites and applications share one visual system defined in [BRAND.md](../BRAND.md) and [BOING-DESIGN-SYSTEM.md](BOING-DESIGN-SYSTEM.md): a deep-navy foundation with a single electric-blue accent. (The earlier "outerspace-oceanic" teal/cyan theme is historical.)

- **Look & feel:** Deep navy, restrained glow, friendly and futuristic; status shown honestly (public testnet, mainnet not live).
- **Colors:** Page background `--boing-navy-950` `#050A1A`, surfaces `--boing-navy-900` `#0A1128`, cards `--boing-navy-800` `#101A38`, accent `--boing-accent` `#3B8CFF`.
- **Typography:** Font stacks are set in `website/src/styles/design-tokens-cosmic.css` (currently Comfortaa).
- **Motifs:** Engraved stone slab with vein SVG, shooting stars and mascot in brand assets.
- **Implementation:** `website/src/styles/design-tokens-cosmic.css`, `website/src/styles/boing-theme.css`, `website/src/components/StoneEngravingOverlay.astro`. Use the same tokens across Boing products.

### Site structure

```
boing.network/
├── /                    → Landing: hero (animated), tokenomics (charts), roadmap, ecosystem, innovations, resources
├── /about               → Design philosophy, pillars, innovation overview
├── /docs/               → Single-page documentation (table of contents, anchor navigation)
│   │                    → Sections: Overview, Getting Started, Network, Tokenomics, Architecture, RPC API, Operations, Security, Governance, Resources
│   ├── /getting-started → Redirects to /docs#getting-started
│   └── /rpc-api         → Redirects to /docs#rpc-api
├── /developers/         → Developer resources
│   ├── /quickstart      → CLI, SDK, local dev
│   ├── /sdk             → boing init, dev, deploy
│   ├── /automation      → Scheduler, triggers, executor incentives
│   └── /rpc-reference   → Full RPC docs + examples
├── /status              → Network status (uptime, validators, block height when API connected)
├── /faucet              → Request testnet BOING (RPC form)
├── /bootnodes           → What bootnodes are, how to use, official list
├── /single-vs-multi     → Single node vs multi-node testnet
├── /testnet/            → **Testnet Portal** + join hub
│   ├── /join            → Join testnet hub (bootnodes, single-vs-multi, faucet, link to portal)
│   ├── /                → Portal landing (role cards, dashboard links)
│   ├── /register        → Registration form
│   ├── /developers      → Developers community + dashboard (dApps, incentive pool)
│   ├── /users           → Users community + dashboard (quests, faucet, feedback)
│   └── /operators       → Node operators community + dashboard (leaderboard)
├── /community           → GitHub, Discord, governance, grants
└── /resources           → Tokenomics, roadmap, whitepapers, FAQs
```

### Content mapping (source → site)

| Page / Section | Source | Notes |
|----------------|--------|-------|
| Landing | docs/BOING-BLOCKCHAIN-DESIGN-PLAN.md (Design Philosophy, Priority Pillars) | Hero, "Quality, enforced by the protocol" messaging |
| /about | docs/SIX-PILLARS.md, docs/BOING-NETWORK-ESSENTIALS.md | Written pillars and essentials as **PDFs** (`/pdfs/…`); page chrome only |
| /docs/getting-started | docs/BUILD-ROADMAP.md Quick Start, README | `cargo build`, `cargo run -p boing-node` |
| /docs/architecture | Design Plan (Tech Stack, Full Stack Architecture) | Mermaid diagrams, layer breakdown |
| /docs#rpc-api (and redirect `/docs/rpc-api`) | docs/RPC-API-SPEC.md | On-site: full method table + error codes aligned with `boing-node`; canonical prose + examples remain in the repo spec (also PDF in `/pdfs/`) |
| /docs/runbook | docs/RUNBOOK.md | Operator procedures |
| /docs/security | docs/SECURITY-STANDARDS.md | DDoS, rate limits, incident response |
| /developers/quickstart | README, docs/BUILD-ROADMAP.md | Crates, CLI, local dev |
| /developers/sdk | docs/DEVELOPMENT-AND-ENHANCEMENTS.md | boing init, dev, deploy |
| /developers/automation | docs/DEVELOPMENT-AND-ENHANCEMENTS.md (appendix) | Verification types, incentives |
| /status | API (D1 or live RPC) | Block height, validator count |
| /testnet/join | docs/TESTNET.md | Bootnodes, faucet, config |
| /faucet | docs/TESTNET.md | Testnet BOING request form |
| /resources | docs/BUILD-ROADMAP.md, docs/NETWORK-COST-ESTIMATE.md | Roadmap, cost estimates |

### Cloudflare services mapping

| Service | Use Case | Example |
|---------|----------|---------|
| **Cloudflare Pages** | Static site (HTML/JS/CSS) + SPA routing | Landing, docs, developer pages |
| **Cloudflare Workers** | API routes, serverless logic | `/api/status`, `/api/explorer`, redirects |
| **D1 Database** | Indexed data, analytics | Block explorer index, tx/account lookups, network stats |
| **R2 Storage** | Large assets, backups | Chain snapshots, archival data, docs PDFs |
| **KV** | Caching, session, rate-limit state | RPC response cache, rate-limit counters |

### Tech stack (website)

- **Framework:** Astro (static-first, Markdown/MDX, Cloudflare adapter)
- **Styling:** Tailwind CSS or minimal custom CSS
- **Deployment:** Cloudflare Pages (GitHub → auto deploy)
- **API:** Cloudflare Workers (optional) for `/api/*` routes
- **Future:** D1 for block explorer, KV for caching

### SEO & meta

- **Tagline:** Boing Network — Quality, enforced by the protocol.
- Title: `Boing Network — Quality, enforced by the protocol.`
- Description: `Boing is an independent Layer 1 where quality is enforced by the protocol.`
- Canonical URLs, Open Graph, Twitter cards, JSON-LD structured data
- OG image: `/og.png` (1200×630) — add for social sharing
- Favicon: `/favicon.svg`
- robots.txt, sitemap.xml

---

## Part 2: Cloudflare setup and deployment

### Overview

Create a Cloudflare project that includes:

| Resource | Purpose |
|----------|---------|
| **Cloudflare Pages** | Website (boing.network) — static + optional API routes |
| **D1 Database** | Indexed chain data for block explorer, stats, API |
| **R2 Storage** | Backups, archival data, large assets |
| **KV Namespace** | Caching, rate limiting, session data |

### Step 1: Add domain to Cloudflare

1. Log in to [Cloudflare Dashboard](https://dash.cloudflare.com/)
2. **Add a site** → Enter `boing.network`
3. Select a plan (Free is fine for Pages + Workers)
4. Update nameservers at your registrar to Cloudflare's

### Step 2: Create D1 database

```bash
npm install -g wrangler
wrangler d1 create boing-network-db
```

Use the output `database_id` in `wrangler.toml`. Example schema: see `website/schema.sql`. Apply with:

```bash
wrangler d1 execute boing-network-db --file=./website/schema.sql
```

**Node listing rows (`network_listings`):** After publishing new `boing-node` release zips, refresh URLs and zip SHA-256 in D1 from `website/`. **Order of operations** (tag → GitHub release → D1 → `networks.js`): [TESTNET.md](TESTNET.md) §9.

```bash
cd website
node scripts/network-listings-release-sql.mjs <release-tag>
node scripts/network-listings-release-sql.mjs <release-tag> --apply
```

Use a **Cloudflare API token** with D1 write if `wrangler login` OAuth returns authentication error `10000` on `--apply`. The script downloads each zip from GitHub, hashes it, and upserts `boing-devnet`, `boing-devnet-linux`, and `boing-devnet-macos`.

### Step 3: Create R2 bucket

```bash
wrangler r2 bucket create boing-network-assets
```

Add binding to `wrangler.toml`. Typical uses: chain snapshots, archived block data, large docs.

### Step 4: Create KV namespace

```bash
wrangler kv:namespace create "BOING_CACHE"
wrangler kv:namespace create "BOING_CACHE" --preview
```

Add bindings to `wrangler.toml`. Typical uses: RPC response cache, rate-limit counters, feature flags.

### Step 5: Cloudflare Pages project

**GitHub integration (recommended):**

1. **Workers & Pages** → **Create** → **Pages** → **Connect to Git**
2. Select the `boing-network` repo
3. Configure:
   - **Build command:** `cd website && npm run build`
   - **Build output:** `website/dist`
   - **Root directory:** (leave blank or `/`)
4. Add environment variables:
   - `PUBLIC_TESTNET_RPC_URL`: `https://testnet-rpc.boing.network/`
   - `PUBLIC_BOOTNODES`: `/ip4/PRIMARY_IP/tcp/4001,/ip4/SECONDARY_IP/tcp/4001` (comma-separated multiaddrs)

**Wrangler deploy:**

```bash
cd website
wrangler pages project create boing-network --production-branch main
wrangler pages deploy dist --project-name=boing-network
```

The repo `website/wrangler.toml` sets `compatibility_flags = ["nodejs_compat"]` so Functions can use `node:crypto` (e.g. portal auth). If deploy fails with **No such module "node:crypto"**, run the deploy from the `website` directory so Wrangler picks up `wrangler.toml`, or in the dashboard: **Workers & Pages** → **boing-network** → **Settings** → **Functions** → **Compatibility flags** → add **nodejs_compat**.

If you see **504 Gateway Timeout** or "upstream request timeout" from the Cloudflare API, the API was temporarily overloaded. Retry the deploy in a few minutes; the GitHub Actions workflow retries up to 3 times with a 30s delay.

### Step 6: Custom domain

1. **Workers & Pages** → **boing-network** → **Custom domains**
2. Add `boing.network` and `www.boing.network`
3. Cloudflare provisions SSL automatically

### Step 7: Workers with D1 + R2 + KV (optional API)

For API routes (e.g. `/api/status`, `/api/blocks/:height`), create a Worker with D1/R2/KV bindings and route via **Workers Routes** to `api.boing.network` or `boing.network/api/*`.

### Wrangler config layout

```
boing-network/
├── website/
│   ├── wrangler.toml     # Pages or Pages + Worker
│   ├── schema.sql
│   └── ...
└── docs/
    └── WEBSITE-AND-DEPLOYMENT.md   # This file
```

Example minimal `website/wrangler.toml` for Pages only:

```toml
name = "boing-network"
pages_build_output_dir = "dist"
compatibility_date = "2024-01-01"
```

For Pages + Functions with D1/KV/R2, use the [Cloudflare adapter for Astro](https://docs.astro.build/en/guides/deploy/cloudflare/).

### Checklist

- [ ] Domain `boing.network` added to Cloudflare
- [ ] D1 database `boing-network-db` created
- [ ] R2 bucket `boing-network-assets` created
- [ ] KV namespace `BOING_CACHE` created
- [ ] Pages project linked to GitHub
- [ ] Custom domain `boing.network` on Pages
- [ ] `PUBLIC_TESTNET_RPC_URL` and `PUBLIC_BOOTNODES` set for testnet pages
- [ ] (Optional) Worker with D1/R2/KV bindings for API

---

*Boing Network — Quality, enforced by the protocol.*
