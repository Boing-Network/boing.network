# Self-hosted Boing website and portal

Run the boing.network site (and optionally the portal API) on your own infrastructure so you don't depend on Cloudflare.

## Quick start (static site only)

```bash
# From repo root
cd website
npm ci
npm run build
npx serve dist -p 3000
```

Open http://localhost:3000. Docs and marketing pages work; portal sign-in and registration require the API (see below).

## With Docker (static site)

From the **repo root** (so the Dockerfile can see `website/`):

```bash
docker build -f website/self-host/Dockerfile .
docker run -p 3000:3000 <image-id>
```

Or tag the image: `docker build -f website/self-host/Dockerfile -t boing-website .` then `docker run -p 3000:3000 boing-website`.

## Adding the portal API (sign-in, registration, nonces)

The production API runs as Cloudflare Pages Functions and uses D1. To run the same logic on your own server:

1. **Database:** Apply `website/schema.sql` to SQLite or Postgres. The Functions expect a D1-like API: `db.prepare(sql).bind(...).run()` (returns `{ meta: { changes } }`), `.first()`, `.all()`.

2. **Wire the handlers:** The route handlers live in `website/functions/api/`. Each file exports `onRequestGet` or `onRequestPost(context)` where `context = { env: { DB }, request }`. Build a Node server that:
   - Creates a `Request` from the incoming HTTP request (same URL, method, body).
   - Provides `env.DB` as a D1-compatible object (e.g. use `better-sqlite3` with a thin wrapper that matches D1's `.prepare().bind().run()/.first()/.all()`).
   - Calls the corresponding handler, then sends the returned `Response` back.

3. **Run:** Start your Node server so it serves static files from `dist/` and forwards `/api/*` to the handlers. No Cloudflare or wrangler required.

Example D1-style wrapper for `better-sqlite3`: implement an object with `prepare(sql)` returning `{ bind(...).run() -> { meta: { changes } }, first(), all() }` using the sqlite3 statement API. Then pass it as `env.DB`.

## Environment

- **DB:** Your database connection or path (e.g. path to SQLite file). The Functions do not read other env vars for the portal; RPC URLs for the chain are configured in the frontend.

## TLS and DNS

Point your domain at your server and use your own TLS (e.g. Let's Encrypt with Caddy or nginx). No Cloudflare required.
