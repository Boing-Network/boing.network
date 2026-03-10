# Boing Infrastructure Independence

This document outlines what the Boing Network and Boing Express wallet **depend on** (other networks, libraries, hosting) and whether those dependencies can be replaced with **custom-built or self-hosted** solutions so Boing relies only on its own infrastructure where feasible.

---

## 1. What “independence” already means

- **Protocol / chain:** Boing does **not** depend on Ethereum, Solana, or any other chain for addresses, signing, or consensus. It uses its own:
  - **Addresses:** 32-byte Ed25519 public keys (64 hex chars).
  - **Signing:** Ed25519 + BLAKE3 (same as used in Rust node and wallet).
  - **RPC:** Boing-specific JSON-RPC; no EVM ABI, no Solana SPL.
- **Portal sign-in:** Ed25519-only; no `personal_sign` or other chain-specific methods.
- So **at the protocol level**, Boing is already independent of other networks.

The question below is about **code and operational** dependencies: libraries, runtimes, and hosting.

---

## 2. Dependency overview

### 2.1 Boing Network repo (chain + website)

| Category        | Dependency              | Used for                    | Replace with custom? |
|----------------|--------------------------|-----------------------------|----------------------|
| **Crypto (Rust)** | `blake3`, `ed25519-dalek`, `sha2` | Hashing, signing, tx format | Not recommended (see 3.1) |
| **Serialization** | `serde`, `bincode`       | Tx/block encoding           | Possible but large effort; bincode matches wallet |
| **Website (JS)**  | `@noble/ed25519`, `@noble/hashes` | Portal sign-in verification | Not recommended (see 3.1) |
| **Website**       | Astro, Node, npm         | Build, SSG, dev server       | Possible (see 3.2)   |
| **Hosting**       | Cloudflare Pages/Workers, D1 | Site + API + DB              | Yes: self-host (see 3.3) |

### 2.2 Boing Express (wallet)

| Category   | Dependency              | Used for                    | Replace with custom? |
|-----------|--------------------------|-----------------------------|----------------------|
| **Crypto**  | `@noble/ed25519`, `@noble/hashes` | Keys, BLAKE3, Ed25519 sign  | Not recommended (see 3.1) |
| **UI/runtime** | React, React-DOM, React-Router | Popup, approval, app UI     | Possible (see 3.2)   |
| **Build**  | Vite, TypeScript, pnpm   | Bundle extension and app    | Possible (see 3.2)   |
| **Hosting** | Cloudflare Pages, wrangler | Wallet web app, deploy      | Yes: self-host (see 3.3) |

---

## 3. Per-category guidance

### 3.1 Cryptography (BLAKE3, Ed25519, SHA-2)

- **Current:** Rust node uses `blake3`, `ed25519-dalek`, `sha2`; website and wallet use `@noble/hashes` and `@noble/ed25519`.
- **Replace with custom?** **Not recommended.**
  - Crypto is easy to get wrong; bugs can be catastrophic.
  - These libraries are widely used and audited; reimplementing BLAKE3/Ed25519 yourself would be a big, high-risk project.
- **Pragmatic approach:**
  - Treat them as **vendored “black boxes”**: pin versions, optionally ship copies in-repo so you’re not tied to a registry at build time.
  - You stay “independent” of *other chains*; you still rely on well-known, standard implementations of standard algorithms.

### 3.2 Build tools and UI frameworks (Astro, Vite, React, TypeScript)

- **Replace with custom?** **Technically possible, not advised for independence.**
  - You could replace Astro with a custom static generator, React with vanilla JS or a custom UI layer, Vite with a custom bundler. That would be a large rewrite and ongoing maintenance cost.
  - These are **tooling** dependencies, not protocol or “other network” dependencies; they don’t affect chain or wallet semantics.
- **Pragmatic approach:**
  - Keep current stack; pin versions and lockfiles.
  - If the goal is “no npm registry at build time,” use **vendoring** (e.g. commit `node_modules` or use a private mirror) rather than reimplementing the tools.

### 3.3 Hosting and infra (Cloudflare, CDN, D1)

- **Replace with custom?** **Yes — this is where “own infrastructure” makes the most sense.**
  - **Website/portal:** Run the built static site and serverless functions on your own servers (e.g. Node behind nginx, or your own FaaS).
  - **Database:** Replace D1 with your own Postgres/MySQL or other DB; keep the same schema and API contracts.
  - **CDN/DNS:** Use your own edge boxes or a different provider; no need to depend on Cloudflare specifically.
- **Pragmatic approach:**
  - Document “we can run the same build on our own infra.”
  - Add a **self-host** option: e.g. Dockerfile or runbook that serves the built site + API + DB. Then Boing is not *reliant* on Cloudflare; it’s just one deployment target.

### 3.4 Other networks (EVM, Solana, etc.)

- **Already done:** Portal and wallet are Ed25519/BLAKE3-only; no dependency on other chains for core flows.
- **Optional:** If any docs or UI still mention “EVM/Solana compatibility,” treat that as optional or legacy; core Boing infrastructure does not rely on them.

---

## 4. Recommended path: “own infrastructure” without reimplementing crypto

1. **Keep**  
   - All cryptography from current, audited libraries (Rust and JS).  
   - Current build and UI stack (Astro, Vite, React, TypeScript), with versions pinned.

2. **Vendor / pin**  
   - Lock dependency versions (Cargo.lock, package-lock.json / pnpm-lock.yaml).  
   - Optionally vendor npm/crates (e.g. commit or mirror) so builds don’t *require* public registries.

3. **Self-host option**  
   - Add a **self-hosted** deployment path for the website and API (e.g. Docker + your own DB), so Boing can run entirely on your own servers and not depend on Cloudflare.

4. **Document**  
   - In this doc and in BOING-EXPRESS-WALLET.md, state clearly:
     - **Protocol:** Boing does not depend on any other blockchain.
     - **Crypto:** Uses standard, audited implementations of BLAKE3 and Ed25519 (no custom crypto).
     - **Ops:** Can run on own infra; Cloudflare is one supported deployment target.

That gives you **maximum practical independence** (own chain, own hosting, optional own build mirror) without the risk and cost of custom crypto or custom tooling.

---

## 5. Why is “no external libraries at all” discouraged?

Short answer: **yes — it’s both very complicated and very costly**, and for crypto it’s also **high risk**. So “no external libraries” is discouraged for good reason.

### Cryptography (BLAKE3, Ed25519, SHA-2)

- **Complexity:** These algorithms have strict specs (constant-time ops, curve math, encoding). A small bug (e.g. a non–constant-time comparison or wrong byte order) can break security entirely (e.g. key recovery or signature forgery). Getting them right from scratch is a specialist, long-term effort.
- **Cost:** You’d need to implement, test, and maintain the code, and pay for **security audits**. Audited, widely used libraries (e.g. `blake3`, `ed25519-dalek`, `@noble/*`) have already had that investment.
- **Risk:** Rolling your own crypto is famously where bugs appear. The industry recommendation is: **don’t implement crypto yourself**; use well-known, audited implementations. So “no external libs” for crypto is discouraged because the **risk and cost are high** and the **benefit** (slightly fewer dependencies) is small.

### Tooling (bundlers, compilers, frameworks)

- **Complexity:** Building something equivalent to Vite, Astro, or TypeScript is a **multi-year** project for a team. These tools are large and nuanced.
- **Cost:** Huge engineering time and ongoing maintenance. You’d be redoing work that the ecosystem already maintains.
- **Benefit:** Almost none for “independence” in the sense that matters (your chain not depending on another chain). Your build tools don’t affect protocol security or chain sovereignty; they’re just how you produce the website and wallet binaries.

So: **“no external libraries at all” is discouraged because** for crypto it’s **too risky and costly**, and for tooling it’s **too costly** with **no real gain** for Boing’s independence. Vendoring + self-hosting gives you “we don’t depend on a third-party host or registry at runtime” without that risk or cost.

---

## 6. Summary table

| Goal                              | Feasible? | Approach                                      |
|-----------------------------------|-----------|-----------------------------------------------|
| No dependency on other blockchains| Yes       | Already done (Ed25519/BLAKE3, Boing RPC only) |
| No dependency on npm/crates       | Partial   | Vendor / private mirror; don’t reimplement    |
| Replace crypto libs with custom  | Not recommended | Keep audited blake3, ed25519-dalek, @noble/* |
| Replace Astro/React/Vite with custom | Possible  | Large rewrite; low benefit for “independence”  |
| Run without Cloudflare           | Yes       | Self-host site + API + DB (Docker/runbook)    |

---

## 7. Self-host runbook (website + portal API)

You can run the boing.network **site and portal API** on your own infrastructure so you don’t depend on Cloudflare.

### 7.1 Build the site

From the repo root:

```bash
cd website
npm ci
npm run build
```

This produces `website/dist/` (static site) and the same logic used for `/api/*` lives in `website/functions/`.

### 7.2 Option A: Static site only (no portal API)

Serve the static files from `dist/` with any HTTP server. No sign-in or registration; docs and marketing pages work.

```bash
cd website
npx serve dist -p 3000
# Or: docker run -v "$(pwd)/dist:/usr/share/nginx/html:ro" -p 3000:80 nginx:alpine
```

### 7.3 Option B: Site + portal API (self-hosted DB)

To run the **portal** (sign-in, registration, nonces) yourself:

1. **Database:** Use the same schema as D1. Apply `website/schema.sql` to your own SQLite or Postgres (adjust dialect if needed).
2. **API routes:** The handlers live in `website/functions/api/`. On Cloudflare they run as serverless Functions; on your own infra you need a Node server that:
   - Serves static files from `dist/` for non-`/api` paths.
   - For `/api/*`, implements the same endpoints (see 7.4) and reads/writes your DB.
3. **Environment:** Set the same env the Functions expect (e.g. DB connection). No `CLOUDFLARE_*` required.

A minimal **self-host server** is in `website/self-host/` (see 7.4). It serves `dist/` and mounts a small adapter so the same Function code can run against a SQLite file. You can replace SQLite with Postgres by changing the DB binding.

### 7.4 Minimal Node server (website/self-host/)

- **`server.js`** — Node HTTP server: serves `dist/` as static (path from `DIST_PATH` or `../dist`), and exposes **`/api/health`** (returns `{ ok: true, self_hosted: true }`). For full portal API (sign-in, nonce, register), implement the same routes as in `website/functions/api/` using your own DB; see README in that folder.
- **`Dockerfile`** — Builds the site and runs the Node server so the whole app runs in one container. From repo root: `docker build -f website/self-host/Dockerfile .` then `docker run -p 3000:3000 <image>`.

See **`website/self-host/README.md`** for run instructions and how to add the full portal API with your own database.

### 7.5 DNS and TLS

Point your domain at your server(s). Use your own TLS (e.g. Let’s Encrypt) or a reverse proxy (nginx, Caddy). No Cloudflare required.

---

## 8. Vendoring (build without public registries)

To reduce dependence on npm and crates.io at **build time**, you can vendor dependencies (ship them in-repo or in a private mirror).

### 8.1 npm (website and wallet)

**Option A: Commit `node_modules` (full vendor)**

- After `npm ci` or `pnpm install`, commit `node_modules/` (and omit it from `.gitignore` for that tree, or use a separate vendor commit).
- Builds then need no network: `npm run build` uses the committed deps.
- Downside: large repo size and noisy diffs when you update deps.

**Option B: Private npm mirror**

- Run a private registry (e.g. Verdaccio, npm-registry-proxy) that caches or mirrors npm. CI and dev install from the mirror.
- You still “depend” on the mirror, but not on the public registry at build time.

**Option C: Offline bundle**

- Archive `node_modules` (and lockfile) into a tarball; unpack in CI or on air-gapped machines before `npm run build`.

**Recommended:** Keep `package-lock.json` (or `pnpm-lock.yaml`) committed so builds are reproducible. Add a CI check that `npm ci` (or `pnpm install --frozen-lockfile`) succeeds. For full “no npm at build time,” use Option A or C.

### 8.2 Cargo (Rust node and crates)

**Option A: Vendor crates**

```bash
cargo vendor
```

This writes all dependencies to a `vendor/` directory. Then build with:

```bash
cargo build --release --frozen
```

and in `.cargo/config.toml`:

```toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

Commit `vendor/` (and the config) so `cargo build` no longer hits crates.io.

**Option B: Private crate registry**

- Run a private Cargo registry (e.g. crates.io mirror or self-hosted registry) and point the workspace at it. Same idea as npm mirror.

**Recommended:** Commit `Cargo.lock` and run `cargo vendor` when you want an offline/air-gapped build. Add to the repo root `.cargo/config.toml` (or document in CONTRIBUTING):

```toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

Then after `cargo vendor`, commit the `vendor/` directory so `cargo build --frozen` no longer hits crates.io.

---

*This document lives in the boing-network repo so both the chain and the wallet (boing.express) can align on what “infrastructure independence” means and what to replace vs keep.*
