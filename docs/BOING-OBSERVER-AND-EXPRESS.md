# Boing Observer & Boing Express — Build guide and explorer spec

This document combines **what is already in the boing-network repo versus what to build in separate projects** for **boing.observer** (explorer) and **boing.express** (wallet), and the full **boing.observer** explorer specification (RPC, QA UI, MVP, one-shot prompt).

- **Wallet (Boing Express):** Full bootstrap, integration, Chrome Web Store, and portal sign-in are in **[BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)** (single doc).

---

## Part 1: What’s in this repo vs what to build

### boing.observer (blockchain explorer)

**In this repo (ready for the explorer)**

- **Spec / build prompt:** [Part 2: Explorer specification](#part-2-explorer-specification-boingobserver) below — phased plan, RPC methods, design (boing.observer, “Boing Observer”), QA pillar visibility, MVP features (network selector, home with blocks, block/account pages, search).
- **RPC:** Node CORS already allows `https://boing.observer` (and localhost). No code changes needed for the explorer to call the public RPC from the browser.
- **References:** [RPC-API-SPEC.md](RPC-API-SPEC.md), [BOING-DESIGN-SYSTEM.md](BOING-DESIGN-SYSTEM.md), [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md), [READINESS.md](READINESS.md).

**What to build (outside this repo)**

- **The explorer app itself** — a separate frontend (e.g. Next.js, Remix, Astro, or Vite) that:
  - Uses the one-shot prompt in [§10. One-shot prompt you can paste](#10-one-shot-prompt-you-can-paste) (Part 2 below).
  - Reads from the Boing JSON-RPC (e.g. testnet RPC URL from env): `boing_chainHeight`, `boing_getBlockByHeight`, `boing_getBlockByHash`, `boing_getBalance`, `boing_getAccount`.
  - Implements: network selector (Testnet/Mainnet), home (chain height + latest blocks), block detail (by height/hash), account page (balance, nonce, stake), search (height / hash / address), and “Protocol QA Passed” for ContractDeploy + QA explainer in footer/About.
- **Hosting:** Deploy the app and point **boing.observer** to it (e.g. Vercel, Cloudflare Pages). No backend required beyond calling the public RPC.

**Nothing else is required in the boing-network repo** for the explorer to work once the app is built and testnet RPC is live.

### boing.express (network wallet)

**In this repo (ready for the wallet)**

- **Spec / bootstrap + integration + portal:** [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) — creation prompt, full Boing integration checklist (balance, send, faucet, signing, nonce, errors), Chrome Web Store packaging, RPC methods, Boing signing spec (BLAKE3 + Ed25519, bincode layout), portal wallet connection and sign-in API.
- **RPC:** Node CORS allows `https://boing.express` (and localhost) so the wallet web app can call the RPC from the browser. See [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) § CORS.
- **References:** [RPC-API-SPEC.md](RPC-API-SPEC.md) (including `boing_getBalance` for wallets), [BOING-DESIGN-SYSTEM.md](BOING-DESIGN-SYSTEM.md) (Aqua Personal variant), `crates/boing-primitives` (types, signature, bincode).

**What to build (outside this repo)**

- **The wallet app** — web app + optional Chrome extension:
  - Use the bootstrap prompt and Part 2 checklists in [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md).
  - Implement: create/import wallet (Ed25519), view/copy address (64-char hex), balance via `boing_getBalance` / `boing_getAccount`, send BOING (Transfer, Boing signing spec, `boing_submitTransaction`), simulate with `boing_simulateTransaction`, testnet faucet (`boing_faucetRequest`), network switch (Testnet/Mainnet), error mapping.
  - Chrome extension: Manifest V3, “Boing Express” naming, minimal permissions, chrome.storage for keys; see Part 2.2–2.4 of the wallet doc.
- **Hosting:** Deploy web app to **boing.express** (e.g. Cloudflare Pages). No server-side key handling; keys stay in browser/extension.

**Nothing else is required in the boing-network repo** for the wallet to work once the app is built and testnet RPC is live (and CORS is already set for boing.express).

### Optional (nice-to-have)

- **Website links:** When observer and wallet are live, add links from boing.network (e.g. nav or “Ecosystem” / “Tools”) to boing.observer and boing.express. See [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) for the full cross-linking checklist.
- **RPC method `boing_getSpendableBalance`:** [RPC-API-SPEC.md](RPC-API-SPEC.md) recommends it for wallets; if the node exposes it, the wallet can use it for display instead of deriving from full state.

### Summary

| Product | Spec in repo | RPC CORS in node | What to build elsewhere |
|--------|----------------|------------------|--------------------------|
| **boing.observer** | Part 2 below (this doc) | Already allowed | Explorer frontend + deploy at boing.observer |
| **boing.express** | [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) | Already allowed | Wallet web app + optional extension + deploy at boing.express |

No further changes are required in the boing-network repo for either product beyond building and deploying the respective apps using the existing docs.

---

## Part 2: Explorer specification (boing.observer)

> **Use Part 2** when asking an AI or team to start the **boing.observer** blockchain explorer project. It gives context, constraints, and a phased plan so the explorer works for both **devnet/testnet** and **mainnet**.

### 1. Project goal

Build a **blockchain explorer** for **Boing Network** at the domain **boing.observer**. The explorer must support:

- **Devnet / testnet** (current public testnet and local dev).
- **Mainnet** (when it launches), with minimal extra work.

Users should be able to browse blocks, transactions, and accounts, and search by block height, block hash, and account address (and later by transaction hash if the node/RPC supports it).

The explorer must also **visually surface the automated quality assurance (QA) gate** — Boing’s **sixth pillar**. Nothing deploys on-chain without passing protocol QA; the explorer should make this visible and understandable to users.

### 2. Boing Network technical context

#### 2.1 Chain basics

- **Chain type:** L1 blockchain (Rust, BLAKE3, Ed25519, PoS + HotStuff BFT).
- **RPC:** JSON-RPC 2.0 over **HTTP POST**, default port **8545**.
- **Base URL:** `http://<host>:<rpc_port>/` (e.g. `https://testnet-rpc.boing.network/` for testnet, or a mainnet RPC when available).
- **Address (AccountId):** 32-byte value, represented as **64 hex characters** (optional `0x` prefix).
- **Block hash:** 32-byte BLAKE3 hash, hex-encoded (64 hex chars).
- **Transaction format:** Bincode-serialized; signed with Ed25519; transaction ID is BLAKE3 of the serialized transaction (see repo for exact hashing).

#### 2.2 RPC methods the explorer will use

Use the **Boing JSON-RPC API** as the source of truth. Key methods:

| Method | Params | Returns | Use in explorer |
|--------|--------|--------|------------------|
| `boing_chainHeight` | `[]` | `u64` | Latest block number, “chain tip” indicator. |
| `boing_getBlockByHeight` | `[height]` (u64) | Block object or `null` | Block list/detail by height. |
| `boing_getBlockByHash` | `[hex_block_hash]` (32 bytes hex) | Block object or `null` | Block detail by hash. |
| `boing_getBalance` | `[hex_account_id]` | `{ balance: string }` (u128 as decimal string) | Account balance. |
| `boing_getAccount` | `[hex_account_id]` | `{ balance, nonce, stake }` (strings/numbers) | Account page: balance, nonce, stake. |

**Block object** (from RPC): The node returns a JSON object with at least:

- `hash` — hex-encoded block hash (32 bytes).
- `header` — object with: `parent_hash`, `height`, `timestamp`, `proposer`, `tx_root`, `state_root` (all hex or numbers as per node serialization).
- `transactions` — array of transaction objects (each has `nonce`, `sender`, `payload`, `access_list`; IDs can be derived from serialized tx hash if no `boing_getTransactionByHash` exists).

**Note:** The official spec does not define `boing_getTransactionByHash`. For “transaction by hash” the first version can either (a) not offer it, or (b) scan recent blocks by height and match by derived tx hash. Plan for a possible future RPC method.

**Reference:** All methods, error codes, and params are in the repo at **`docs/RPC-API-SPEC.md`**. Implement against that spec; if the node returns extra fields, the explorer can display them.

#### 2.3 Transaction payload types (for decoding)

Transactions have a `payload` field. When displaying “type” and details, decode:

- **Transfer** — `to` (AccountId), `amount` (u128).
- **Bond** — `amount` (stake).
- **Unbond** — `amount`.
- **ContractCall** — `contract`, `calldata`.
- **ContractDeploy** — `bytecode`.

Balances and amounts are in **smallest units** (u128); the explorer may need a known decimals (e.g. 18) and format for human-readable BOING.

#### 2.4 Networks (devnet vs mainnet)

- **Testnet / devnet:** Public RPC URL is (or will be) published on the Boing website (e.g. `https://testnet-rpc.boing.network/`). Faucet: `boing_faucetRequest` on testnet only. Until bootnodes and public RPC are live, use `http://127.0.0.1:8545` for local testing (see [READINESS.md](READINESS.md) §3).
- **Mainnet:** RPC and bootnodes will be published at launch; same RPC methods, no faucet.
- The explorer should **switch networks** (e.g. dropdown or subdomain) and call the corresponding RPC base URL. No API key required for read-only RPC in the spec.

### 3. Design and branding

- **Domain:** **boing.observer** (explorer only).
- **Brand:** “Boing Observer” or “Boing Explorer” — clearly part of the Boing ecosystem but focused on observation/exploration.
- **Visual:** Prefer consistency with Boing’s design system where it helps recognition:
  - Shared tokens: dark backgrounds (`--boing-black`, `--boing-navy`), Orbitron for display, Inter for body, glassmorphism-style cards.
  - Full design system: **`docs/BOING-DESIGN-SYSTEM.md`** (boing.network variant is “Cosmic Foundation”). You may reuse tokens and typography; the explorer can have a distinct “observer” feel (e.g. data-dense, tables, monospace for hashes and addresses).
- **Accessibility:** Respect reduced motion and contrast requirements from the design system; ensure tables and links are keyboard- and screen-reader friendly.

### 4. Quality Assurance (6th pillar) — visual integration

Boing’s **automated quality assurance** is a core differentiator: **no asset deploys without passing protocol QA first**. The explorer must visually reflect this.

#### 4.1 Why it matters

- QA is one of Boing’s six pillars; the explorer should communicate that the network enforces quality at the protocol layer.
- Every `ContractDeploy` transaction included in a block has already passed the automated QA gate (otherwise it would have been rejected and never mined).
- Users should understand that Boing is not “deploy anything” — there is a protocol-level gate for specs and purpose (no scams, legitimate use).

**Reference:** `docs/QUALITY-ASSURANCE-NETWORK.md` — full QA design (Allow/Reject/Unsure, community pool, blocklist, meme leniency, etc.).

#### 4.2 QA elements to implement

| Element | Where | What to show |
|--------|--------|---------------|
| **ContractDeploy badge** | Block detail page, transaction list rows | For each `ContractDeploy` tx: a badge such as “Protocol QA Passed” or “QA Gate Passed” (with tooltip: “This deployment passed the automated QA gate before inclusion in the block”). |
| **QA explainer section** | Footer, About page, or persistent info panel | Short copy: “All contract deployments on Boing pass the automated QA gate before they are included. No scams. No deploy-first-fix-later.” Link to `QUALITY-ASSURANCE-NETWORK.md` or a public URL (e.g. boing.network/docs) for full details. |
| **QA in nav or header** | Global nav or sidebar | A small “Quality Assured” or “Protocol QA” link/indicator that opens the explainer or an `/about#qa` section. |
| **Transaction type + QA** | Transaction detail view | When showing a `ContractDeploy` payload, add a clear line: “Deployment passed protocol QA (Allow)” — reinforcing that inclusion in the block implies approval. |

#### 4.3 Optional (post-MVP)

- If the RPC or block/transaction structure ever includes QA metadata (e.g. `rule_id`, `purpose_category`), display it.
- Dedicated “QA Overview” page summarizing how many deployments have passed and linking to the QA doc.
- Stats such as “X contract deployments, all QA-verified” on the home or block listing.

### 5. Core features (MVP)

Implement in this order:

1. **Network selector** — Switch between **Testnet** and **Mainnet** (mainnet can point to a “coming soon” or same RPC until launch). Store selection in URL or localStorage.
2. **Home / dashboard** — Current chain height (from `boing_chainHeight`). List of **latest blocks** (e.g. last N blocks by height, using `boing_getBlockByHeight` in a loop or one call per height). Optional: latest transactions aggregated from those blocks.
3. **Block detail page** — URL pattern: e.g. `/block/:height` or `/block/:hash`. Show: block hash, height, timestamp (convert to local time), proposer (AccountId with link to account page), parent hash (link to block), state root, tx root. List of transactions in the block with: tx type (Transfer/Bond/Unbond/ContractCall/ContractDeploy), sender, key payload fields, and link to account(s). **For each `ContractDeploy` transaction:** show a “Protocol QA Passed” (or similar) badge — inclusion in a block means it passed the automated QA gate. If transaction hash is available (derived or from RPC), show it and optionally a future `/tx/:txHash` page.
4. **Account page** — URL pattern: `/account/:address` (address = 32-byte hex, 64 chars, optional `0x`). Show: balance (from `boing_getBalance` or `boing_getAccount`), nonce, stake (from `boing_getAccount`). Optional: list of recent transactions involving this account (if you have an index or can derive from recent blocks).
5. **Search** — Input: block height (number), block hash (64 hex), or account address (64 hex). Dispatch to: block by height, block by hash, or account page.
6. **QA pillar visibility** — Footer or About section: short explainer that “All contract deployments on Boing pass the automated QA gate before inclusion. No scams. No deploy-first-fix-later.” with link to QA docs. Optional: “Quality Assured” or “Protocol QA” link in nav/header that surfaces this.
7. **Config and env** — **Testnet RPC URL** and **Mainnet RPC URL** from environment or config (e.g. `NEXT_PUBLIC_TESTNET_RPC`, `NEXT_PUBLIC_MAINNET_RPC` or similar). No hardcoded production RPC URLs in the repo; use placeholders in `.env.example`.

### 6. Tech stack suggestions (flexible)

- **Frontend:** Next.js, Remix, or Astro for SSR/SSG and clean URLs; or a SPA (e.g. Vite + React) with client-side routing.
- **Styling:** Tailwind or the same approach as boing.network (see `website/src/styles/` in the repo) for consistency.
- **RPC client:** Simple `fetch()` to the chosen RPC base URL with JSON-RPC body; no SDK required for read-only. Example:

  ```json
  {"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]}
  ```

- **Hosting:** Any (Vercel, Cloudflare Pages, etc.). Point **boing.observer** to this app.

### 7. Repo and docs to reference

- **RPC spec:** `docs/RPC-API-SPEC.md` — methods, params, errors.
- **Design system:** `docs/BOING-DESIGN-SYSTEM.md` — colors, typography, card style.
- **Quality assurance:** `docs/QUALITY-ASSURANCE-NETWORK.md` — sixth pillar, Allow/Reject/Unsure, community pool, blocklist; use for copy and links.
- **Testnet/mainnet:** `docs/TESTNET.md`, `docs/VIBEMINER-INTEGRATION.md` — RPC URLs, bootnodes, faucet (testnet only).
- **Essentials:** `docs/BOING-NETWORK-ESSENTIALS.md` — chain overview, tech stack, address format.
- **Existing website:** `website/` in the same repo is the main Boing site (boing.network); explorer is a **separate app** (boing.observer) but can reuse design tokens and patterns.

### 8. Out of scope for MVP

- Faucet UI (already at boing.network/faucet).
- Submitting transactions (explorer is read-only).
- Wallet connection (optional later).
- Historical analytics, charts, or indexing beyond what the RPC provides (can be a later phase).

### 9. Success criteria

- Users can open **boing.observer**, select Testnet (or Mainnet when available), and see the latest block height and a list of recent blocks.
- Users can open a block by height or hash and see header + transactions.
- Users can open an account by 32-byte hex address and see balance, nonce, stake.
- Search by block height, block hash, or account address works.
- **ContractDeploy** transactions display a “Protocol QA Passed” (or equivalent) badge; the QA pillar is visible (explainer in footer/About, optional nav link).
- RPC URLs are configurable; the app works against the official testnet RPC (and a future mainnet RPC) without code changes.

### 10. One-shot prompt you can paste

Copy the following into an AI or brief:

```
I need to start a blockchain explorer for Boing Network at the domain boing.observer. It should work for both devnet/testnet and mainnet.

Context:
- Boing is an L1 chain with a JSON-RPC API (HTTP POST, port 8545). Full method list and params are in docs/RPC-API-SPEC.md in the boing-network repo.
- Key methods: boing_chainHeight, boing_getBlockByHeight, boing_getBlockByHash, boing_getBalance, boing_getAccount.
- Addresses are 32-byte AccountIds as 64 hex chars; block hashes are 32-byte hex. Block object has header (parent_hash, height, timestamp, proposer, tx_root, state_root) and transactions array.
- Transaction payloads: Transfer, Bond, Unbond, ContractCall, ContractDeploy. Balances are u128 in smallest units (e.g. 18 decimals for BOING).
- Design: follow docs/BOING-DESIGN-SYSTEM.md (dark theme, Orbitron/Inter, glassmorphism). Domain is boing.observer; brand “Boing Observer”.
- Quality Assurance (6th pillar): Boing enforces protocol-level QA — no contract deploys without passing the automated gate. See docs/QUALITY-ASSURANCE-NETWORK.md. The explorer must visually show this.

MVP features:
1. Network selector (Testnet / Mainnet) with configurable RPC URLs via env.
2. Home: latest chain height and list of latest blocks.
3. Block page: block by height or hash — header fields and list of transactions with type/sender/payload summary.
4. For each ContractDeploy transaction: show a "Protocol QA Passed" badge (inclusion in a block means it passed the QA gate).
5. Account page: balance, nonce, stake for a 32-byte hex address.
6. Search: by block height, block hash, or account address.
7. QA explainer in footer or About: "All contract deployments on Boing pass the automated QA gate before inclusion. No scams. No deploy-first-fix-later." with link to QA docs.

Please scaffold the project (e.g. Next.js or Astro), add config for TESTNET_RPC and MAINNET_RPC, and implement the above pages, search, and QA visuals. Use the RPC spec in the repo for exact request/response shapes.
```

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
