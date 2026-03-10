# Boing Observer & Boing Express — What’s in Repo vs What to Build

This doc answers: **what else is needed** for **boing.observer** (blockchain explorer) and **boing.express** (network wallet). Both are specified in this repo; the actual apps are separate projects (different repos or not yet created).

---

## boing.observer (Blockchain Explorer)

### In this repo (ready for the explorer)

- **Spec / build prompt:** [BOING-OBSERVER-EXPLORER-PROMPT.md](BOING-OBSERVER-EXPLORER-PROMPT.md)  
  Phased plan, RPC methods, design (boing.observer, “Boing Observer”), QA pillar visibility, MVP features (network selector, home with blocks, block/account pages, search).
- **RPC:** Node CORS already allows `https://boing.observer` (and localhost). No code changes needed for the explorer to call the public RPC from the browser.
- **References:** [RPC-API-SPEC.md](RPC-API-SPEC.md), [BOING-DESIGN-SYSTEM.md](BOING-DESIGN-SYSTEM.md), [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md), [READINESS.md](READINESS.md).

### What to build (outside this repo)

- **The explorer app itself** — a separate frontend (e.g. Next.js, Remix, Astro, or Vite) that:
  - Uses the one-shot prompt in §10 of [BOING-OBSERVER-EXPLORER-PROMPT.md](BOING-OBSERVER-EXPLORER-PROMPT.md).
  - Reads from the Boing JSON-RPC (e.g. testnet RPC URL from env): `boing_chainHeight`, `boing_getBlockByHeight`, `boing_getBlockByHash`, `boing_getBalance`, `boing_getAccount`.
  - Implements: network selector (Testnet/Mainnet), home (chain height + latest blocks), block detail (by height/hash), account page (balance, nonce, stake), search (height / hash / address), and “Protocol QA Passed” for ContractDeploy + QA explainer in footer/About.
- **Hosting:** Deploy the app and point **boing.observer** to it (e.g. Vercel, Cloudflare Pages). No backend required beyond calling the public RPC.

**Nothing else is required in the boing-network repo** for the explorer to work once the app is built and testnet RPC is live.

---

## boing.express (Network Wallet)

### In this repo (ready for the wallet)

- **Spec / bootstrap + integration:** [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)  
  Creation prompt, full Boing integration checklist (balance, send, faucet, signing, nonce, errors), Chrome Web Store packaging, RPC methods, Boing signing spec (BLAKE3 + Ed25519, bincode layout).
- **RPC:** Node CORS now allows `https://boing.express` (and localhost) so the wallet web app can call the RPC from the browser. See [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) § CORS.
- **References:** [RPC-API-SPEC.md](RPC-API-SPEC.md) (including `boing_getBalance` for wallets), [BOING-DESIGN-SYSTEM.md](BOING-DESIGN-SYSTEM.md) (Aqua Personal variant), `crates/boing-primitives` (types, signature, bincode).

### What to build (outside this repo)

- **The wallet app** — web app + optional Chrome extension:
  - Use the bootstrap prompt and Part 2 checklists in [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md).
  - Implement: create/import wallet (Ed25519), view/copy address (64-char hex), balance via `boing_getBalance` / `boing_getAccount`, send BOING (Transfer, Boing signing spec, `boing_submitTransaction`), simulate with `boing_simulateTransaction`, testnet faucet (`boing_faucetRequest`), network switch (Testnet/Mainnet), error mapping.
  - Chrome extension: Manifest V3, “Boing Express” naming, minimal permissions, chrome.storage for keys; see Part 2.2–2.4 of the wallet doc.
- **Hosting:** Deploy web app to **boing.express** (e.g. Cloudflare Pages). No server-side key handling; keys stay in browser/extension.

**Nothing else is required in the boing-network repo** for the wallet to work once the app is built and testnet RPC is live (and CORS is already set for boing.express).

---

## Optional (nice-to-have)

- **Website links:** When observer and wallet are live, add links from boing.network (e.g. nav or “Ecosystem” / “Tools”) to boing.observer and boing.express. See [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) for the full cross-linking checklist.
- **RPC method `boing_getSpendableBalance`:** [RPC-API-SPEC.md](RPC-API-SPEC.md) recommends it for wallets; if the node exposes it, the wallet can use it for display instead of deriving from full state.

---

## Summary

| Product           | Spec / prompt in repo | RPC CORS in node | What to build elsewhere |
|------------------|-----------------------|------------------|--------------------------|
| **boing.observer** | ✅ [BOING-OBSERVER-EXPLORER-PROMPT.md](BOING-OBSERVER-EXPLORER-PROMPT.md) | ✅ Already allowed | Explorer frontend + deploy at boing.observer |
| **boing.express**  | ✅ [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) | ✅ Now allowed     | Wallet web app + optional extension + deploy at boing.express |

No further changes are required in the boing-network repo for either product beyond building and deploying the respective apps using the existing docs.
