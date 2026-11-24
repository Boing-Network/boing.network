# docs.boing.network

Astro **Starlight** documentation site for Boing Network.

## Content policy (important)

- Pages under `src/content/docs/` are **authored** guides, written from the live public RPC, `boing-node`, the SDK, and the product repos.
- They are **not** a sync/copy of monorepo `docs/*.md` or of the marketing site `/docs` page. Link to those for exhaustive reference.
- Existing Markdown + **PDFs** remain accuracy references and printable artifacts. Link out; do not delete `website/public/pdfs/` or the `npm run build:pdfs` pipeline.

## Structure

| Sidebar group | Folder | Audience |
|---------------|--------|----------|
| Start here | `start/`, `testnet/join` | Everyone |
| Build apps | `developers/`, `apps/` | dApp and wallet developers |
| Run a node | `nodes/`, `testnet/operators` | Node operators and validators |
| Protocol | `protocol/` | Anyone who wants the details |
| Reference | `resources/` | Links to specs and PDFs |

## Style

- Open each page with one or two sentences that say who it is for and what it covers.
- Use `.mdx` with Starlight's `<Steps>`, `<Tabs>` and `<Aside>` for procedures; plain `.md` otherwise.
- Write node commands with the **four testnet consensus settings** (`BOING_CHAIN_ID=6913`, `BOING_VALIDATOR_SET=open`, `BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT=400`, `BOING_NAME_REGISTRY_ACTIVATION_HEIGHT=640`) and the current bootnode from `website/src/config/testnet.ts`.
- Link monorepo docs with full `https://github.com/Boing-Network/boing.network/blob/main/...` URLs; link pages on this site with root-relative paths (`/nodes/run-a-node/`).
- Brand colors and fonts live in `src/styles/custom.css` (Comfortaa headings, Inter body, light and dark tokens).

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
