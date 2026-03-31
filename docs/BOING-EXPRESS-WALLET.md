# Boing Express — Wallet: Bootstrap, Integration & Chrome Web Store

Use this document to **bootstrap**, **integrate**, and **publish** **Boing Express** — the wallet for the Boing Network. Product name is always **Boing Express** (not "Boing Wallet"). Domain: **boing.express**. This doc merges the creation prompt, full Boing integration checklist, Chrome Web Store readiness, and **Part 3:** portal wallet connection, sign-in APIs, rollout, and smoke tests.

**Cross-repo alignment:** For canonical URLs, RPC endpoints, chain IDs, and cross-linking between boing.network, boing.express, and boing.observer, see [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md).

---

## Table of Contents

1. [Part 1: Bootstrap / Creation](#part-1-bootstrap--creation)
2. [Part 2: Full integration & Chrome Web Store](#part-2-full-integration--chrome-web-store)
3. [Part 3: Portal connection, sign-in API, and rollout](#part-3-portal-connection-sign-in-api-and-rollout)
4. [Network independence](#network-independence)
5. [Session-based lock and unlock](#session-based-lock-and-unlock)
6. [Reference: Boing Network repo](#reference-boing-network-repo)
7. [Quick reference: RPC methods for the wallet](#quick-reference-rpc-methods-for-the-wallet)

---

## Part 1: Bootstrap / Creation

Copy-paste prompt for Cursor / AI or human devs to **create** the wallet project:

```
Create a new project for a crypto wallet with the following specs.

**Project name & domain**
- Product name: **Boing Express** (use everywhere: UI, extension title, store listing). In descriptions, explain that Boing Express is a wallet for the Boing Network.
- Domain: boing.express (already purchased)
- Hosting: Cloudflare — Cloudflare Pages for the main wallet app; optionally Workers for API routes. Deploy to Cloudflare with custom domain boing.express.

**Primary chain: Boing Network**
- Support Boing Network first: balance, send/receive BOING, testnet faucet, (later) staking (bond/unbond).
- Boing is not EVM-compatible:
  - **Address / AccountId:** 32 bytes, Ed25519 public key. Display as 64-char hex (optional 0x).
  - **Signing:** Ed25519. Transactions: specific serialization, BLAKE3 hash, then sign. See "Boing signing spec" below.
  - **RPC:** JSON-RPC HTTP. Methods: boing_getBalance([hex_account_id]), boing_getAccount([hex_account_id]); boing_submitTransaction([hex_signed_tx]), boing_chainHeight([]), boing_simulateTransaction([hex_signed_tx]), boing_faucetRequest([hex_account_id]). Reference: docs/RPC-API-SPEC.md.
- **Transaction format:** nonce, sender, payload (Transfer | Bond | Unbond | ContractCall | ContractDeploy), access_list. Submit hex(bincode(SignedTransaction)); Signature 64-byte Ed25519. bincode layout must match boing-primitives.

**Boing signing spec (must match boing-network)**
- Signable message = BLAKE3(nonce_le || sender_32 || bincode(payload) || bincode(access_list)).
- Signature = Ed25519(signable_message). SignedTransaction = { tx, signature }; submit hex(bincode(SignedTransaction)).
- Payload types: Transfer, Bond, Unbond, ContractCall, ContractDeploy. AccessList: read/write AccountId arrays. Same bincode layout as boing-primitives.

**Multi-chain readiness**
- Pluggable "networks": e.g. network adapter interface (get balance, build/sign tx, submit, get nonce). Boing first and default. UI: switch networks without full rewrite. Config-driven RPC URLs, chain id, Boing adapter.

**Security & key management**
- Keys generated and stored on client (browser). Web Crypto or audited lib for Ed25519. Encrypted private key in localStorage/sessionStorage or IndexedDB; user password/PIN. Never send private keys to server. Cloudflare only proxy RPC or static assets.

**UX (minimum)**
- Create wallet (Ed25519 keypair, backup/export).
- Import wallet (seed/phrase or hex key).
- View Boing address (64-char hex), copy button.
- Balance: boing_getBalance or boing_getAccount (decimal strings for u128).
- Send BOING: form (to, amount), build Transfer, sign, boing_submitTransaction.
- Testnet faucet: boing_faucetRequest or link to boing.network/faucet with address.
- Network selector: Mainnet / Testnet (RPC URLs).

**Tech stack**
- Modern front-end (React/Vue/Svelte), TypeScript. Static build (+ optional Workers) for Cloudflare Pages. Env vars for RPC URLs.

**Deliverables**
- Cloudflare-oriented project (Pages config, boing.express domain). Boing integration: address, send, faucet, correct signing. Clean separation for adding another chain adapter later.
```

---

## Part 2: Full integration & Chrome Web Store

Use this when **preparing for production**: full Boing integration and Chrome Web Store packaging.

### Part 2.1 — Boing Network integration checklist

- [ ] **Balance:** boing_getBalance or boing_getAccount; decimal strings for balance/stake; refresh on account/network switch.
- [ ] **Nonce:** From boing_getAccount when building next tx; do not guess or cache across sessions.
- [ ] **Send:** Transfer payload, Boing signing spec, boing_submitTransaction([hex_signed_tx]).
- [ ] **Simulate before send:** boing_simulateTransaction; show user if simulation fails.
- [ ] **Faucet (testnet):** boing_faucetRequest; handle -32016 (rate limit), -32601 (faucet not enabled).
- [ ] **Network switch:** Testnet vs Mainnet; persist selection; correct RPC URL.
- [ ] **Chain height:** Optional boing_chainHeight for sync status.
- [ ] **Errors:** Map -32600, -32601, -32602, -32000, -32016 to user-friendly messages.
- [ ] **Address:** 64-char hex, copy, QR. **Keys:** Ed25519; private key never to server; chrome.storage or IndexedDB, encrypted.
- [ ] **Bincode:** Transaction/payload/AccessList match boing-primitives. Signable message: BLAKE3(nonce_le + sender_32 + bincode(payload) + bincode(access_list)); Ed25519 sign.

### Part 2.2 — Chrome extension (Manifest V3)

- [ ] **Manifest:** "name" = "Boing Express"; "manifest_version": 3. Icons 128x128, 48x48, 16x16 from package.
- [ ] **Service worker:** background.service_worker; no eval/remote code execution.
- [ ] **Storage:** chrome.storage.local (or session) for keys/settings.
- [ ] **Permissions:** Minimal; host permissions only for RPC URLs used.
- [ ] **CSP:** No unsafe-inline or remote script unless documented.
- [ ] **Connection approval (required for every site):** When **any** website or dApp with Boing integration calls `boing_requestAccounts` (or `eth_requestAccounts`), the wallet **must** show a connection-approval UI (e.g. "Allow [origin] to view your address?" or "Connect to [site name]?") and **only return accounts after the user explicitly approves**. Do not return accounts without user approval, regardless of origin—this applies to boing.network, boing.express, and any other site that integrates Boing. Similarly, for `boing_signMessage` / `personal_sign`, show what is being signed (e.g. "Connect to Boing Portal" or the dApp’s message) and require the user to approve before returning a signature. Storing "connected sites" per origin is optional; the important part is that the **first** time a site requests accounts (or after the user disconnects), the wallet always prompts for approval. (See **Part 3** for portal APIs and rollout.)
- [ ] **Session-based lock/unlock:** See [Session-based lock and unlock](#session-based-lock-and-unlock) below so users do not need to enter their password on every use.

### Part 2.3 — Chrome Web Store listing

- [ ] **Short description:** One line; product name Boing Express.
- [ ] **Detailed description:** What the wallet does; Boing Express throughout.
- [ ] **Screenshots:** At least one; main flows (balance, send, settings).
- [ ] **Privacy:** Single purpose; privacy policy URL; data usage accurate (e.g. keys local, no user data to developer servers).
- [ ] **Category:** e.g. Productivity or Finance; match single purpose.

### Part 2.4 — Pre-submission checklist

- [ ] Production bundle; no dev-only code or test keys.
- [ ] Test on clean Chrome profile: install, create/import wallet, balance, send testnet tx, faucet. No console errors; txs accepted by node.
- [ ] ZIP: extension directory only (manifest, scripts, assets); manifest_version 3; all Dashboard tabs filled.

---

## Part 3: Portal connection, sign-in API, and rollout

This section describes the current testnet portal sign-in, what is needed for **wallet connection** and **address + password** sign-in, what the **boing.express** wallet (or any Boing-compatible wallet) must implement for secure dApp connection, and the **implemented** portal APIs (nonce-backed wallet auth, migrations, smoke test).

### 3.1 Current sign-in (portal)

- **Flow:** User enters account ID (32-byte hex, `0x...`). Frontend calls `GET /api/portal/me?account_id_hex=...`. If the account is registered (developer / user / node_operator), the portal stores `{ account_id_hex, role }` in `localStorage` and redirects to the role’s hub.
- **No wallet:** No browser extension or wallet is involved; it’s address-only.
- **No password:** Anyone who knows the address can “sign in” as that account. Fine for testnet; not acceptable for mainnet or sensitive actions.

### 3.2 Adding wallet connection to sign-in

To support “Connect wallet” on the portal (and later dApps):

#### 3.2.1 Wallet injector (browser extension)

- The wallet (e.g. boing.express) injects a provider into `window` (e.g. `window.ethereum` or `window.boing`).
- Standard pattern: **EIP-1193**, but for Boing wallets the preferred methods are **`boing_requestAccounts`** and **`boing_signMessage`**. Compatibility aliases like `eth_requestAccounts` and `personal_sign` may still exist.
- **Website/dApp responsibilities:**
  - Detect provider (e.g. `window.boing` or `window.ethereum`).
  - Call “request accounts” to get the current account (address).
  - For **authentication**, have the user sign a one-time or short-lived message (e.g. “Sign in to Boing Portal at {origin} at {timestamp}”), then send that signature (and address) to the backend; backend verifies the signature and issues a session (e.g. JWT or session cookie). Do **not** treat “I have the address” as proof of control; always verify with a signature.

#### 3.2.2 What the portal implements

- **Frontend:** A “Connect wallet” button that (1) requests accounts from the injected provider, (2) fetches a backend nonce, (3) asks the wallet to sign the message, and (4) on success, stores session and redirects. **Implemented:** The portal prefers `boing_requestAccounts`, `boing_signMessage`, `boing_chainId`, and `boing_switchChain`, with alias fallback for compatibility.
- **Backend:** `GET /api/portal/auth/nonce?origin=...` issues a short-lived nonce, and `POST /api/portal/auth/sign-in` verifies Ed25519(`message`, `signature`) against `account_id_hex`. If the message includes a nonce, the backend validates origin, expiry, and one-time use before creating session. **Implemented:** Uses Node built-in `crypto` (Ed25519) via `nodejs_compat`; no external libraries.
- **Chain/RPC:** The wallet and the site must agree on the chain (e.g. Boing testnet/mainnet) and that the address format is 32-byte hex. The portal does not need to send transactions; only **message signing** for auth.

### 3.3 Adding address + password sign-in

To support “Sign in with address + password” (no wallet):

- **Backend:** Store a **hashed password** per account (e.g. in `portal_registrations` or a dedicated `portal_auth` table). Use a secure hash (e.g. Argon2 or bcrypt); never store plaintext passwords.
- **Registration:** When a user registers (or in a separate “Set password” step), they submit a password; backend hashes it and stores the hash linked to `account_id_hex`.
- **Sign-in:** User submits `account_id_hex` + password; backend hashes the submitted password, compares to stored hash, and if it matches and the account is registered, create session.
- **Security:** Use HTTPS only; consider rate limiting and lockout for failed attempts. Optional: combine with message signing (wallet) so that sensitive actions require a fresh signature even when session was created with password.

### 3.4 What boing.express wallet needs for secure dApp connection

For the Boing wallet to connect securely to websites/web apps/dApps, the following are typically required.

#### 3.4.1 Provider API (injected or standard)

- **EIP-1193–style provider** so dApps can call:
  - `request({ method: 'eth_requestAccounts' })` → returns array of account addresses (e.g. 32-byte hex with `0x`).
  - `request({ method: 'personal_sign', params: [message, address] })` (or equivalent) → returns signature so the dApp or backend can verify the signer.
- Optional: `request({ method: 'eth_chainId' })` and/or `request({ method: 'wallet_switchEthereumChain', params: [{ chainId: ... }] })` if the wallet supports multiple chains (e.g. Boing testnet vs mainnet).
- Expose this on a well-known object (e.g. `window.boing` or register as an EIP-6963 provider) so dApps can discover and use it.
- **Connection approval is the wallet’s responsibility:** For **every** site that has Boing integration (not just boing.network), the wallet must show an approval UI when a site calls `boing_requestAccounts` / `eth_requestAccounts` and only return accounts after the user approves. See **Part 2.2** above (Chrome extension checklist).

#### 3.4.2 Account format and chain

- **Account format:** Boing uses 32-byte (64 hex chars + `0x`) account addresses. The wallet must expose these in the same format the chain/RPC expects.
- **Chain ID:** Define a chain ID for Boing testnet and mainnet (if not already). dApps will use this to request the correct chain and RPC URL.

#### 3.4.3 RPC and signing

- **RPC:** The wallet needs the correct Boing RPC URL (testnet/mainnet) to submit transactions and optionally to verify state. The dApp may pass this, or the wallet may have it built in for known chains.
- **Signing:** For auth, dApps only need **message signing** (e.g. `personal_sign`). For transactions, the wallet must support signing the Boing transaction format (whatever the node expects: e.g. raw tx bytes or a structured payload).

#### 3.4.4 Security and UX

- **Origin and prompts:** Before signing a message or sending a tx, show the user the **origin** (domain) and a clear description (e.g. “Sign in to Boing Portal” or “Approve transfer”). Never sign blindly.
- **Phishing resistance:** Validate that the request is coming from a page the user is viewing (e.g. same tab/window). Optional: support WalletConnect or similar for mobile/second device so the same principles apply there.
- **Key storage:** Private keys must be stored and used in a secure way (e.g. encrypted, not exportable by default). This is entirely inside the wallet implementation.

#### 3.4.5 Optional: WalletConnect / EIP-6963

- **WalletConnect:** If the wallet is on mobile or as a separate app, WalletConnect (v2) allows dApps to connect to it via QR or deep link. The wallet then needs to implement the WalletConnect provider side and the same RPC/signing methods.
- **EIP-6963:** Multi-injector discovery so dApps can list “available wallets” instead of relying on a single `window.ethereum`. Boing wallet can advertise itself via this so it appears in “Connect wallet” modals that support the standard.

### 3.5 Summary (wallet vs portal)

| Feature | Portal/dApp side | Wallet (boing.express) side |
|--------|-------------------|------------------------------|
| **Current sign-in** | Address only; no proof of control | N/A |
| **Wallet connect** | Request accounts + sign-in message; verify signature on backend | EIP-1193 provider; `eth_requestAccounts`; `personal_sign` (or equivalent); expose on `window` or EIP-6963 |
| **Password sign-in** | Submit address + password; backend compares hash | N/A |
| **Secure dApp connection** | Use provider for accounts + signing; never trust “I have the address” without a signature | Provider API; 32-byte hex accounts; Boing chain ID + RPC; secure key storage; clear user prompts with origin |

Implementing wallet connection on the portal requires a backend sign-in endpoint that verifies a signature and then creates a session. Implementing address + password requires storing and checking a hashed password per account. The Boing wallet needs an EIP-1193–style provider with account discovery and message (and transaction) signing, plus clear UX and secure key handling, to support secure connections to any website or dApp.

### 3.6 Portal wallet sign-in API (implemented)

The portal and developer tools use **no external wallet libraries**. Connection uses the standard EIP-1193 pattern against the injected provider (`window.boing` or `window.ethereum`).

#### 3.6.0 Provider discovery and errors (portal-wallet.js)

- **Discovery:** The portal loads `/portal-wallet.js`, which (1) prefers `window.boing`, (2) discovers Boing-compatible wallets via **EIP-6963** (`eip6963:announceProvider` with name/rdns containing "boing"), and (3) falls back to `window.ethereum`. Use `BoingPortalWallet.getProvider()` when present.
- **Connection approval:** Before the portal treats a wallet as "connected" (e.g. filling the account on register or set-password), the user must **approve the connection in the wallet**. The portal requests a signature of a one-time message: `Connect to Boing Portal\nOrigin: {origin}\nTimestamp: {iso}`. Only after the wallet returns a signature (user approved in the extension) does the portal fill the account. If the user rejects, the portal shows: *"Connection was not approved in your wallet. Please approve the request in your wallet to connect."* For **sign-in**, the user approves and signs the sign-in message in the wallet; no portal password is required (signature is proof of control).
- **Friendly errors:** If the wallet returns an error such as *"No wallet found. Create or import a wallet in Boing Express"*, the portal maps it to a clear message: *"Create or import a wallet in Boing Express first. Click the Boing Express extension icon…"* so users know to create/import a wallet before connecting. Other known messages (e.g. user rejected) are normalized for consistency.
- **Disconnect / Clear:** Sign-in page has a "Disconnect" control after signing (before clicking Sign in); register and set-password pages show "Clear account" after filling from the wallet so the user can switch account or wallet.
- **Boing Express alignment:** For seamless connection, the wallet should inject `window.boing` and/or announce via EIP-6963 with `name` or `rdns` containing "boing". Implement `boing_requestAccounts` (and optionally `boing_signMessage`, `boing_chainId`, `boing_switchChain`); the portal falls back to `eth_requestAccounts` / `personal_sign` etc. when the Boing-named methods are not found.

#### 3.6.1 `POST /api/portal/auth/sign-in`

- **Body (JSON):** `{ account_id_hex, message, signature }` — no password. The signature is proof of control.
  - `account_id_hex`: 32-byte address as hex with `0x` (66 chars).
  - `message`: Exact UTF-8 string that was shown to the user and signed (e.g. nonce-backed `Sign in to Boing Portal\nOrigin: ...\nTimestamp: ...\nNonce: ...`).
  - `signature`: Ed25519 signature as **64-byte hex** (128 hex chars, optional `0x` prefix). The wallet must return raw Ed25519 for the account's public key (same as Boing account ID).
- **Verification:** Backend verifies Ed25519(`message`, `signature`) with public key = `account_id_hex` using Node built-in `crypto` (no npm packages). Messages with a timestamp are rejected if older than 5 minutes; nonce-based messages also require a valid unconsumed nonce from `GET /api/portal/auth/nonce`.
- **Response:** Same shape as `GET /api/portal/me` on success (e.g. `ok`, `registered`, `account_id_hex`, `role`, …). Frontend then sets session via `BoingPortalSession.setSession(account_id_hex, role)` and redirects.

#### 3.6.2 Frontend (sign-in page)

- **Sign in with wallet:** "Connect wallet" prefers `provider.request({ method: 'boing_requestAccounts' })` and `provider.request({ method: 'boing_signMessage', params: [message, address] })`, with alias fallback to `eth_requestAccounts` and `personal_sign`. After the user signs the message, the frontend calls `POST /api/portal/auth/sign-in` with `{ account_id_hex, message, signature }` (no password) and redirects on success.
- On the testnet portal, frontend checks `boing_chainId` and attempts `boing_switchChain` to `0x1b01` before signing.
- Message format now uses a nonce-backed multiline form:
  `Sign in to Boing Portal`
  `Origin: {origin}`
  `Timestamp: {new Date().toISOString()}`
  `Nonce: {serverNonce}`
- **Sign in with account ID:** User enters account ID and portal password; frontend calls `POST /api/portal/auth/sign-in-account` with `{ account_id_hex, password }`. If the account has no password set, API returns `403` with `need_password: true` and the frontend redirects to `/testnet/set-password`.
- No third-party SDKs (e.g. no ethers, viem, web3.js); only the injected provider and `fetch`.

#### 3.6.3 Portal password (wallet vs account ID sign-in)

- **Wallet sign-in:** No portal password. The user approves and signs the sign-in message in the wallet; the backend verifies the signature and returns session data. No password field or step on the sign-in page for the wallet flow.
- **Account ID sign-in:** Requires the **portal password**. User enters account ID + password; backend verifies password via `POST /api/portal/auth/sign-in-account`. If the account has no password set, the API returns `403` with `need_password: true` and the frontend redirects to `/testnet/set-password`.
- **Registration** includes "Connect wallet" to fill the account ID and **Portal password** + **Confirm password** (min 8 characters). The backend stores a scrypt hash (salt + hash) in `portal_registrations.password_salt` and `portal_registrations.password_hash`.
- **Set password:** `POST /api/portal/auth/set-password` with `{ account_id_hex, message, signature, new_password }`. Message format: `Set portal password for Boing Portal\nOrigin: {origin}\nTimestamp: {ts}\nNonce: {nonce}`. Same nonce as sign-in; backend verifies Ed25519 signature then updates the stored hash.

#### 3.6.4 `POST /api/portal/auth/sign-in-account`

- **Body (JSON):** `{ account_id_hex, password }`. Used for "Sign in with account ID" (no wallet).
- **Verification:** Backend loads `portal_registrations` for `account_id_hex`; if no row or no `password_hash`, returns `403` (with `need_password: true` and `account_id_hex` if registered but no password). Otherwise verifies password with stored salt/hash (scrypt) and returns the same payload as `POST /api/portal/auth/sign-in` on success.

#### 3.6.5 Dependencies (network side)

- **Website/Functions:** No wallet-related npm packages. Portal uses only Astro and Wrangler; auth sign-in uses Node built-in crypto (Ed25519) via nodejs_compat in wrangler.toml. No ethers, viem, web3.js, MetaMask SDK, or similar.
- **Rust/node:** The chain uses ed25519-dalek for transaction and faucet signing; RPC does not implement personal_sign (signing is done in the wallet). CORS allows boing.express and boing.network origins for dApp to node communication.

### 3.7 Rollout, migrations, and smoke test (nonce-backed wallet auth)

#### 3.7.1 Files involved

- `website/src/pages/testnet/sign-in.astro`, `set-password.astro`, register pages
- `website/functions/api/portal/auth/nonce.js`, `sign-in.js`, `set-password.js`, `password.js`, `register.js`
- `website/schema.sql`, `website/migrations/2026-03-06-portal-auth-nonces.sql`, `website/migrations/2026-03-06-portal-password.sql`
- `website/wrangler.toml`, `website/wrangler.worker.toml`

#### 3.7.2 Database migrations

Apply before deploying the updated portal:

```bash
cd website
wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-auth-nonces.sql
wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-password.sql
```

(If applying full schema from scratch, these are already in `website/schema.sql`.)

#### 3.7.3 Deployment

Auth uses Node built-in `crypto`; ensure `compatibility_flags = ["nodejs_compat"]` in `wrangler.toml` and `wrangler.worker.toml`.

#### 3.7.4 Nonce and sign-in contract (reference)

- **`GET /api/portal/auth/nonce?origin=https://boing.network`** — Returns `{ ok, nonce, origin, issued_at, expires_at, message_template }`.
- **Sign-in message (exact UTF-8):**  
  `Sign in to Boing Portal`  
  `Origin: https://boing.network`  
  `Timestamp: <ISO>`  
  `Nonce: <server nonce>`
- **`POST /api/portal/auth/sign-in`** — Body: `{ account_id_hex, message, signature }`. Backend validates 32-byte address, 64-byte Ed25519 signature, message verification, timestamp, nonce (exists, matches origin, not expired, not used), and that the account is registered.

#### 3.7.5 Smoke test checklist

After migration and deploy:

1. Open `/testnet/sign-in`; confirm wallet connect UI and preference for `window.boing` / `boing_requestAccounts`.
2. Confirm testnet chain `0x1b01` is requested; `/api/portal/auth/nonce` returns a nonce.
3. Confirm signing succeeds with connected, unlocked wallet; sign-in fails on replayed nonce, locked wallet, or unregistered address.
4. Confirm legacy address-only sign-in still works for testnet.

#### 3.7.6 Compatibility

- Portal prefers `boing_requestAccounts`, `boing_signMessage`, `boing_chainId`, `boing_switchChain`; falls back to `eth_requestAccounts`, `personal_sign`, etc. when Boing methods are unsupported.
- Backend accepts older timestamp-only message format; nonce-based sign-in is the preferred flow.

---

## Network independence

**Boing Network is chain-independent.** It does not depend on EVM (Ethereum, Base, etc.), Solana, or any other external network for its core protocol, addresses, or signing.

- **Addresses:** 32-byte Ed25519 public keys (64 hex chars). Not EVM 20-byte addresses or Solana base58.
- **Signing:** Ed25519 only. Not secp256k1 (Ethereum) or Solana-specific formats.
- **RPC & transactions:** Boing-specific JSON-RPC and bincode serialization. No EVM ABI, no Solana SPL.
- **Wallet integration:** Boing Express (and any Boing-native wallet) implements `boing_requestAccounts`, `boing_signMessage`, and transaction signing with **Boing’s own formats**. For **portal/dApp message signing** (`boing_signMessage`), the wallet signs **BLAKE3(UTF-8 message)** with Ed25519 (same as Boing tx signing); the portal verifies Ed25519 over BLAKE3(message). No reliance on other chains’ signing (e.g. no EVM `personal_sign`).

When integrating with Boing (wallet, portal, or node), use Boing’s specs only. Do not assume Ethereum or Solana compatibility.

For **dependency and infrastructure** independence (what can be custom-built vs kept, self-hosting, vendoring), see **[BOING-INFRASTRUCTURE-INDEPENDENCE.md](BOING-INFRASTRUCTURE-INDEPENDENCE.md)** in the boing-network repo.

### Portal sign-in 401 (invalid_signature)

If the portal returns **401 Unauthorized** with `invalid_signature` when you click "Sign in" after connecting Boing Express:

1. **Redeploy the portal** — The live site (boing.network) must be running the sign-in API that verifies **BLAKE3(message)**. Push to `main` to trigger the GitHub Action, or run from repo: `cd website && npm run deploy` (requires `CLOUDFLARE_API_TOKEN`). After changing `website/functions/api/portal/auth/sign-in.js`, a deploy is required for the change to take effect. **Check:** After a 401, look at the response header `X-Portal-Sign-In-Version` (e.g. in DevTools → Network → sign-in → Headers). If it is missing or not `blake3-noble-v1`, the new code is not deployed yet. The portal uses **@noble/ed25519** for verification (same library as Boing Express) so sign-in matches the wallet exactly.
2. **Use the latest Boing Express** — The extension must sign **BLAKE3(UTF-8 message)** with Ed25519 (see `src/crypto/keys.ts`). Rebuild the extension (`pnpm run build:extension:load` in boing.express), load the unpacked build from the output folder, then **reload the extension** in `chrome://extensions` and try again.
3. **Same message** — The exact string the page sends to the wallet for signing must be the same as the `message` in the POST body to `/api/portal/auth/sign-in`. The sign-in page does this by storing the signed message and reusing it; do not modify or re-build the message between signing and the sign-in request.

---

## Session-based lock and unlock

Users should **not** have to enter their password every time they open the wallet (e.g. each time they open the extension popup or the web app). Implement **session-based** lock and unlock:

### Unlock once per session

- **Unlock:** When the user enters their password (or PIN) to unlock the wallet, treat it as the start of a **session**. For the duration of that session, the wallet can use the decrypted key material in memory for signing and for serving `boing_requestAccounts` / `boing_signMessage` to dApps **without** asking for the password again.
- **Session scope:** Define the session so it lasts until one of:
  - **User locks:** The user explicitly clicks "Lock" (or "Lock wallet") in the UI.
  - **Inactivity timeout (recommended):** After a period of no user interaction (e.g. 5, 15, or 30 minutes), automatically lock. Reset the timer on any user action (viewing balance, signing, opening popup, etc.). Make the timeout configurable in settings if possible (e.g. "Lock after: 5 min / 15 min / 30 min / Never").
  - **Context teardown (extension):** When the extension’s service worker or popup context is unloaded (browser restart, extension reload), the in-memory key is gone—next open is a new session and the user must unlock again. Optionally use `chrome.storage.session` (or similar) to persist "session unlocked" state only for the current browser session so that reopening the popup in the same browser session does not require password again, until one of the conditions above.

### Lock state

- **When locked:** Do not keep the decrypted private key (or decryption key) in memory. Show a lock screen (e.g. "Boing Express is locked" with an "Unlock" button). Requests from dApps (`boing_requestAccounts`, `boing_signMessage`) should either queue and trigger the unlock UI, or return a clear error so the dApp can ask the user to unlock the wallet first.
- **When unlocked:** Full functionality; user can view balance, send, sign messages, and approve dApp connections without re-entering the password until the session ends.

### Web app (boing.express)

- Same idea: one unlock per session. Session can be "until tab/window close" or "until inactivity timeout" or "until user locks." Avoid asking for the password on every page navigation or every time the user switches back to the tab.

### Checklist

- [ ] Unlock once; then no password required for subsequent actions within the same session.
- [ ] "Lock" button (or equivalent) so the user can lock immediately.
- [ ] Optional: inactivity timeout that auto-locks after N minutes (configurable).
- [ ] When locked, no decrypted key material in memory; dApp requests trigger unlock prompt or clear error.
- [ ] Extension: session survives popup close/reopen within the same browser session (e.g. via chrome.storage.session or in-memory in service worker until it goes idle).

---

## Reference: Boing Network repo

| What | Where |
|------|--------|
| RPC methods, params, errors | docs/RPC-API-SPEC.md |
| Address, Transaction, AccountState, bincode | crates/boing-primitives/src/types.rs |
| Signable hash, SignedTransaction | crates/boing-primitives/src/signature.rs |
| Faucet | RPC boing_faucetRequest; or boing.network/faucet |

---

## Quick reference: RPC methods for the wallet

| Method | Params | Use |
|--------|--------|-----|
| boing_getBalance | [hex_account_id] | Balance (decimal string) |
| boing_getAccount | [hex_account_id] | Balance, nonce, stake (UI and next tx) |
| boing_submitTransaction | [hex_signed_tx] | Submit signed Transfer (or other) |
| boing_simulateTransaction | [hex_signed_tx] | Pre-flight before submit |
| boing_faucetRequest | [hex_account_id] | Testnet only |
| boing_chainHeight | [] | Optional: chain height / sync |

### EIP-6963 (optional multi-wallet discovery)

[EIP-6963](https://eips.ethereum.org/EIPS/eip-6963) lets wallets announce themselves without clobbering `window.ethereum`. **Boing Express** should participate like any multi-chain wallet: dispatch `eip6963:announceProvider` with the standard `info` object (`uuid`, `name`, `icon`, `rdns`) and an [EIP-1193](https://eips.ethereum.org/EIPS/eip-1193)-shaped `provider` that implements Boing methods (`boing_chainHeight`, `boing_sendTransaction`, etc.).

**Optional, non-breaking hint:** dApps may treat `rdns` (e.g. reverse-DNS for `boing.express`) as the stable identifier. If Express adds an **optional** vendor field, use a **single nested key** so standard EIP-6963 parsers ignore it safely, for example:

```json
{
  "uuid": "...",
  "name": "Boing Express",
  "icon": "data:image/svg+xml;base64,...",
  "rdns": "express.boing.network",
  "boing": {
    "chainFamily": "boing",
    "nativeAccountIdBytes": 32,
    "signing": "ed25519"
  }
}
```

The `boing` object is **not** part of the EIP-6963 core schema; it is a documented extension for capability UX (show “Connect with Boing” only when `rdns` / `boing` matches). Probing `provider.request({ method: 'boing_chainHeight', params: [] })` remains the definitive runtime check.

### Injected provider (dApp — Boing Express extension)

| Method | Params | Returns / notes |
|--------|--------|-------------------|
| boing_signTransaction | `[txObject]` | `0x` + hex(bincode `SignedTransaction`). Requires connected origin. User approves in extension UI. `txObject.type`: `transfer`, `bond`, `unbond`, `contract_deploy_purpose`, `contract_deploy_meta`, `contract_call` (fields per **boing.express** `src/boing/dappTxRequest.ts`). **`contract_deploy` (bare) is rejected** — use a purpose-bearing deploy so declarations match protocol QA. `purpose_category` must be one of the categories accepted by **boing_qa** (e.g. `dapp`, `token`, `nft`, `meme`, `community`, `entertainment`, `tooling`, `other`). Omit `nonce` to use `boing_getAccount` on the wallet’s selected RPC. |
| boing_sendTransaction | `[txObject]` | Sign, then `boing_simulateTransaction` when supported, then `boing_submitTransaction`; returns **tx hash** string from the node. Mempool **always** runs QA on contract deploy payloads before acceptance. |

**Node JSON-RPC errors (W2):** When simulation or submit fails with a Boing node JSON-RPC error, the rejected promise from `window.boing.request` includes **`code`** and **`message`** from the node, and **`data.boingCode === 'BOING_NODE_JSONRPC'`** with **`data.rpc`** `{ code, message, data }` so dApps can read structured fields (e.g. QA **`rule_id`** / **`doc_url`** in `data.rpc.data`). Wallet-native failures still use other `data.boingCode` values. See [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md).

**Bincode note:** Wallet encoding must match **Rust** `boing-primitives` (serde/bincode 1.3): `TransactionPayload` enum uses **u32 LE** variant indices in this order: Transfer(0), ContractCall(1), ContractDeploy(2), ContractDeployWithPurpose(3), ContractDeployWithPurposeAndMetadata(4), Bond(5), Unbond(6). All three deploy variants include a trailing `create2_salt: Option<[u8; 32]>` (`None` for legacy nonce-based addresses; `Some(salt)` for CREATE2-style — see `TECHNICAL-SPECIFICATION.md` §4.4). Full field layout and signable-hash rule: [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md) (and `boing-sdk` golden tests).

---

*This document lives in the boing-network repo so the Boing Express team can align with the chain spec and ship the web app and Chrome extension.*
