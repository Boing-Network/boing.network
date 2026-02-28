# boing.network Website

Static site for [boing.network](https://boing.network) — built with Astro, deployed to Cloudflare Pages.

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

1. Connect the repo in Cloudflare dashboard (Workers & Pages → Create → Pages → Connect to Git)
2. Build command: `cd website && npm run build`
3. Build output: `website/dist`
4. Add custom domain: `boing.network`

Or via Wrangler:

```bash
cd website
npm run build
wrangler pages deploy dist --project-name=boing-network
```

## Documentation

- **[docs/WEBSITE-AND-DEPLOYMENT.md](../docs/WEBSITE-AND-DEPLOYMENT.md)** — Site structure, content mapping, Cloudflare setup (D1, R2, KV), and deployment
- **schema.sql** — D1 schema for block explorer / network stats
