# docs.boing.network

Astro **Starlight** documentation site for Boing Network.

## Content policy (important)

- Pages under `src/content/docs/` are **authored** from audits of the live public RPC, `boing-node`, SDK, and product repos.
- They are **not** a sync/copy of monorepo `docs/*.md` or of the marketing site `/docs` page.
- Existing Markdown + **PDFs** remain accuracy references and printable artifacts. Link out; do not delete `website/public/pdfs/` or the `npm run build:pdfs` pipeline.

## Local

```bash
cd docs-site
npm ci
npm run dev
```

Build output: `docs-site/dist/`.

## Deploy (Cloudflare Pages)

| | |
|--|--|
| Project | `boing-docs` |
| Production | https://docs.boing.network |
| Fallback | https://boing-docs.pages.dev |

```bash
cd docs-site
npm run deploy
```

GitHub Actions: `.github/workflows/deploy-docs-pages.yml` (path filter: `docs-site/**`).

Uses the same `CLOUDFLARE_API_TOKEN` as the main website Pages deploy.

### DNS / custom domain

Already provisioned when possible:

- Pages custom domain: `docs.boing.network`
- DNS: CNAME `docs` → `boing-docs.pages.dev` (proxied) on zone `boing.network`

If SSL shows pending / HTTP 522, wait for certificate validation or re-check the CNAME in the Cloudflare dashboard (**Workers & Pages** → **boing-docs** → **Custom domains**).
