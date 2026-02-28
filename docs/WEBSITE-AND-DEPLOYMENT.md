# Boing Network — Website Specification and Deployment

> **Domain:** boing.network  
> **Hosting:** Cloudflare Pages  
> **Backend:** Cloudflare Workers + D1 + R2 + KV

---

## Part 1: Website Specification

### Design theme: Outerspace-Oceanic

All official Boing websites and applications use the **outerspace-oceanic** theme to match the [Boing Network promotional launch video](https://youtu.be/sSP6wsugauo) and Marketing Asset Package.

- **Look & feel:** Deep ocean / cosmic depth; bioluminescent teal and electric blue; friendly, futuristic.
- **Colors:** Primary `#00E5CC` (Teal/Cyan), Secondary `#00B4FF` (Electric Blue), Dark background `#0A0E1A` (Deep Navy).
- **Typography:** Comfortaa (sans), JetBrains Mono (code).
- **Motifs:** Hexagonal grid (network/circuit), circuit-style lines, neon glow, floating orbs, bubbles, shooting stars; robot mascot in brand assets.
- **Implementation:** `website/src/styles/boing-theme.css`, `website/src/components/EnhancedAnimatedBackground.astro`. Use the same theme and environment across Boing products for a consistent brand experience.

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
├── /network/            → Network status & explorer
│   ├── /status          → Uptime, validators, block height (D1 / API)
│   ├── /testnet         → Join testnet hub (links to bootnodes, single-vs-multi, faucet)
│   ├── /single-vs-multi → Dedicated page: single node vs multi-node
│   ├── /bootnodes       → Dedicated page: what bootnodes are, how to use, official list
│   ├── /faucet          → Dedicated page: request testnet BOING (RPC form)
│   └── /explorer        → Block/tx/account lookup (when available)
├── /community           → GitHub, Discord, governance, grants
└── /resources           → Tokenomics, roadmap, whitepapers, FAQs
```

### Content mapping (source → site)

| Page / Section | Source | Notes |
|----------------|--------|-------|
| Landing | docs/BOING-BLOCKCHAIN-DESIGN-PLAN.md (Design Philosophy, Priority Pillars) | Hero, "Authentic L1" messaging |
| /about | Design Plan §1–5, Innovation table | Philosophy, unique features |
| /docs/getting-started | docs/BUILD-ROADMAP.md Quick Start, README | `cargo build`, `cargo run -p boing-node` |
| /docs/architecture | Design Plan (Tech Stack, Full Stack Architecture) | Mermaid diagrams, layer breakdown |
| /docs/rpc-api | docs/RPC-API-SPEC.md | Full RPC spec, methods, error codes |
| /docs/runbook | docs/RUNBOOK.md | Operator procedures |
| /docs/security | docs/SECURITY-STANDARDS.md | DDoS, rate limits, incident response |
| /developers/quickstart | README, docs/BUILD-ROADMAP.md | Crates, CLI, local dev |
| /developers/sdk | docs/DEVELOPMENT-AND-ENHANCEMENTS.md | boing init, dev, deploy |
| /developers/automation | docs/AUTOMATION-VERIFICATION.md | Verification types, incentives |
| /network/status | API (D1 or live RPC) | Block height, validator count |
| /network/testnet | docs/TESTNET.md | Bootnodes, faucet, config |
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

- **Slogan:** The DeFi that always bounces back
- Title: `Boing Network | The DeFi that always bounces back`
- Description: `The DeFi that always bounces back. Authentic L1 blockchain built from first principles.`
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

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
