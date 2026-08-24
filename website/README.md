# boing.network Website

> 👋 **Everyday users:** the site is [boing.network](https://boing.network) — faucet, testnet portal, docs, PDFs.  
> 🛠️ **Developers:** Astro → Cloudflare Pages. Content mapping: [docs/WEBSITE-AND-DEPLOYMENT.md](../docs/WEBSITE-AND-DEPLOYMENT.md).  
> 🛰️ **Operators:** `npm run build:pdfs` regenerates `/pdfs/*.pdf` from `docs/*.md` (including Mermaid).

Static site for [boing.network](https://boing.network) — built with Astro, deployed to Cloudflare Pages.

```mermaid
flowchart LR
  MD[docs/*.md] --> PDF[npm run build:pdfs]
  PDF --> Public[public/pdfs]
  Astro[website/src] --> Pages[Cloudflare Pages]
  Public --> Pages
```

## Setup

```bash
npm install
npm run dev
```

## Build

```bash
npm run build
```

Output: `dist/`

## Deploy to Cloudflare Pages

**From this repo (recommended):** Pushes to `main` trigger a GitHub Action that builds and deploys. You only need one secret in the repo: **Settings → Secrets and variables → Actions → `CLOUDFLARE_API_TOKEN`**. The workflow uses a fixed account ID.

**Local deploy (if you need to deploy without pushing):**

```bash
cd website
export CLOUDFLARE_API_TOKEN=your_token   # from Cloudflare Dashboard → My Profile → API Tokens
export CLOUDFLARE_ACCOUNT_ID=10374f367672f4d19db430601db0926b   # optional if using default
npm run deploy
```

Or build then deploy manually:

```bash
cd website
npm run build
npx wrangler pages deploy dist --project-name=boing-network
```

**Cloudflare Dashboard (alternative):** Workers & Pages → Create → Pages → Connect to Git; build command `cd website && npm run build`, output `website/dist`.

## Documentation

- **[docs/WEBSITE-AND-DEPLOYMENT.md](../docs/WEBSITE-AND-DEPLOYMENT.md)** — Site structure, content mapping, Cloudflare setup (D1, R2, KV), and deployment
- **schema.sql** — D1 schema for block explorer / network stats
