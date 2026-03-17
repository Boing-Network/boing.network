# Wallet Connection and Sign-In

This doc describes the current testnet portal sign-in, what is needed to add **wallet connection** and **address + password** sign-in, and what the **boing.express wallet** (or any Boing-compatible wallet) needs to implement for secure connection to websites, web apps, and dApps.

---

## 1. Current sign-in (portal)

- **Flow:** User enters account ID (32-byte hex, `0x...`). Frontend calls `GET /api/portal/me?account_id_hex=...`. If the account is registered (developer / user / node_operator), the portal stores `{ account_id_hex, role }` in `localStorage` and redirects to the role’s hub.
- **No wallet:** No browser extension or wallet is involved; it’s address-only.
- **No password:** Anyone who knows the address can “sign in” as that account. Fine for testnet; not acceptable for mainnet or sensitive actions.

---

## 2. Adding wallet connection to sign-in

To support “Connect wallet” on the portal (and later dApps):

### 2.1 Wallet injector (browser extension)

- The wallet (e.g. boing.express) injects a provider into `window` (e.g. `window.ethereum` or `window.boing`).
- Standard pattern: **EIP-1193**, but for Boing wallets the preferred methods are **`boing_requestAccounts`** and **`boing_signMessage`**. Compatibility aliases like `eth_requestAccounts` and `personal_sign` may still exist.
- **Website/dApp responsibilities:**
  - Detect provider (e.g. `window.boing` or `window.ethereum`).
  - Call “request accounts” to get the current account (address).
  - For **authentication**, have the user sign a one-time or short-lived message (e.g. “Sign in to Boing Portal at {origin} at {timestamp}”), then send that signature (and address) to the backend; backend verifies the signature and issues a session (e.g. JWT or session cookie). Do **not** treat “I have the address” as proof of control; always verify with a signature.

### 2.2 What the portal would need

- **Frontend:** A “Connect wallet” button that (1) requests accounts from the injected provider, (2) fetches a backend nonce, (3) asks the wallet to sign the message, and (4) on success, stores session and redirects. **Implemented:** The portal prefers `boing_requestAccounts`, `boing_signMessage`, `boing_chainId`, and `boing_switchChain`, with alias fallback for compatibility.
- **Backend:** `GET /api/portal/auth/nonce?origin=...` issues a short-lived nonce, and `POST /api/portal/auth/sign-in` verifies Ed25519(`message`, `signature`) against `account_id_hex`. If the message includes a nonce, the backend validates origin, expiry, and one-time use before creating session. **Implemented:** Uses Node built-in `crypto` (Ed25519) via `nodejs_compat`; no external libraries.
- **Chain/RPC:** The wallet and the site must agree on the chain (e.g. Boing testnet/mainnet) and that the address format is 32-byte hex. The portal does not need to send transactions; only **message signing** for auth.

---

## 3. Adding address + password sign-in

To support “Sign in with address + password” (no wallet):

- **Backend:** Store a **hashed password** per account (e.g. in `portal_registrations` or a dedicated `portal_auth` table). Use a secure hash (e.g. Argon2 or bcrypt); never store plaintext passwords.
- **Registration:** When a user registers (or in a separate “Set password” step), they submit a password; backend hashes it and stores the hash linked to `account_id_hex`.
- **Sign-in:** User submits `account_id_hex` + password; backend hashes the submitted password, compares to stored hash, and if it matches and the account is registered, create session.
- **Security:** Use HTTPS only; consider rate limiting and lockout for failed attempts. Optional: combine with message signing (wallet) so that sensitive actions require a fresh signature even when session was created with password.

---

## 4. What boing.express wallet needs for secure dApp connection

For the Boing wallet to connect securely to websites/web apps/dApps, the following are typically required.

### 4.1 Provider API (injected or standard)

- **EIP-1193–style provider** so dApps can call:
  - `request({ method: 'eth_requestAccounts' })` → returns array of account addresses (e.g. 32-byte hex with `0x`).
  - `request({ method: 'personal_sign', params: [message, address] })` (or equivalent) → returns signature so the dApp or backend can verify the signer.
- Optional: `request({ method: 'eth_chainId' })` and/or `request({ method: 'wallet_switchEthereumChain', params: [{ chainId: ... }] })` if the wallet supports multiple chains (e.g. Boing testnet vs mainnet).
- Expose this on a well-known object (e.g. `window.boing` or register as an EIP-6963 provider) so dApps can discover and use it.
- **Connection approval is the wallet’s responsibility:** For **every** site that has Boing integration (not just boing.network), the wallet must show an approval UI when a site calls `boing_requestAccounts` / `eth_requestAccounts` and only return accounts after the user approves. See [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) Part 2.2 (Chrome extension) for the checklist item.

### 4.2 Account format and chain

- **Account format:** Boing uses 32-byte (64 hex chars + `0x`) account addresses. The wallet must expose these in the same format the chain/RPC expects.
- **Chain ID:** Define a chain ID for Boing testnet and mainnet (if not already). dApps will use this to request the correct chain and RPC URL.

### 4.3 RPC and signing

- **RPC:** The wallet needs the correct Boing RPC URL (testnet/mainnet) to submit transactions and optionally to verify state. The dApp may pass this, or the wallet may have it built in for known chains.
- **Signing:** For auth, dApps only need **message signing** (e.g. `personal_sign`). For transactions, the wallet must support signing the Boing transaction format (whatever the node expects: e.g. raw tx bytes or a structured payload).

### 4.4 Security and UX

- **Origin and prompts:** Before signing a message or sending a tx, show the user the **origin** (domain) and a clear description (e.g. “Sign in to Boing Portal” or “Approve transfer”). Never sign blindly.
- **Phishing resistance:** Validate that the request is coming from a page the user is viewing (e.g. same tab/window). Optional: support WalletConnect or similar for mobile/second device so the same principles apply there.
- **Key storage:** Private keys must be stored and used in a secure way (e.g. encrypted, not exportable by default). This is entirely inside the wallet implementation.

### 4.5 Optional: WalletConnect / EIP-6963

- **WalletConnect:** If the wallet is on mobile or as a separate app, WalletConnect (v2) allows dApps to connect to it via QR or deep link. The wallet then needs to implement the WalletConnect provider side and the same RPC/signing methods.
- **EIP-6963:** Multi-injector discovery so dApps can list “available wallets” instead of relying on a single `window.ethereum`. Boing wallet can advertise itself via this so it appears in “Connect wallet” modals that support the standard.

---

## 5. Summary

| Feature | Portal/dApp side | Wallet (boing.express) side |
|--------|-------------------|------------------------------|
| **Current sign-in** | Address only; no proof of control | N/A |
| **Wallet connect** | Request accounts + sign-in message; verify signature on backend | EIP-1193 provider; `eth_requestAccounts`; `personal_sign` (or equivalent); expose on `window` or EIP-6963 |
| **Password sign-in** | Submit address + password; backend compares hash | N/A |
| **Secure dApp connection** | Use provider for accounts + signing; never trust “I have the address” without a signature | Provider API; 32-byte hex accounts; Boing chain ID + RPC; secure key storage; clear user prompts with origin |

Implementing wallet connection on the portal requires a backend sign-in endpoint that verifies a signature and then creates a session. Implementing address + password requires storing and checking a hashed password per account. The Boing wallet needs an EIP-1193–style provider with account discovery and message (and transaction) signing, plus clear UX and secure key handling, to support secure connections to any website or dApp.

---

## 6. Portal wallet sign-in API (implemented)

The portal and developer tools use **no external wallet libraries**. Connection uses the standard EIP-1193 pattern against the injected provider (`window.boing` or `window.ethereum`).

### 6.0 Provider discovery and errors (portal-wallet.js)

- **Discovery:** The portal loads `/portal-wallet.js`, which (1) prefers `window.boing`, (2) discovers Boing-compatible wallets via **EIP-6963** (`eip6963:announceProvider` with name/rdns containing "boing"), and (3) falls back to `window.ethereum`. Use `BoingPortalWallet.getProvider()` when present.
- **Connection approval:** Before the portal treats a wallet as "connected" (e.g. filling the account on register or set-password), the user must **approve the connection in the wallet**. The portal requests a signature of a one-time message: `Connect to Boing Portal\nOrigin: {origin}\nTimestamp: {iso}`. Only after the wallet returns a signature (user approved in the extension) does the portal fill the account. If the user rejects, the portal shows: *"Connection was not approved in your wallet. Please approve the request in your wallet to connect."* For **sign-in**, the user approves and signs the sign-in message in the wallet; no portal password is required (signature is proof of control).
- **Friendly errors:** If the wallet returns an error such as *"No wallet found. Create or import a wallet in Boing Express"*, the portal maps it to a clear message: *"Create or import a wallet in Boing Express first. Click the Boing Express extension icon…"* so users know to create/import a wallet before connecting. Other known messages (e.g. user rejected) are normalized for consistency.
- **Disconnect / Clear:** Sign-in page has a "Disconnect" control after signing (before clicking Sign in); register and set-password pages show "Clear account" after filling from the wallet so the user can switch account or wallet.
- **Boing Express alignment:** For seamless connection, the wallet should inject `window.boing` and/or announce via EIP-6963 with `name` or `rdns` containing "boing". Implement `boing_requestAccounts` (and optionally `boing_signMessage`, `boing_chainId`, `boing_switchChain`); the portal falls back to `eth_requestAccounts` / `personal_sign` etc. when the Boing-named methods are not found.

### 6.1 `POST /api/portal/auth/sign-in`

- **Body (JSON):** `{ account_id_hex, message, signature }` — no password. The signature is proof of control.
  - `account_id_hex`: 32-byte address as hex with `0x` (66 chars).
  - `message`: Exact UTF-8 string that was shown to the user and signed (e.g. nonce-backed `Sign in to Boing Portal\nOrigin: ...\nTimestamp: ...\nNonce: ...`).
  - `signature`: Ed25519 signature as **64-byte hex** (128 hex chars, optional `0x` prefix). The wallet must return raw Ed25519 for the account's public key (same as Boing account ID).
- **Verification:** Backend verifies Ed25519(`message`, `signature`) with public key = `account_id_hex` using Node built-in `crypto` (no npm packages). Messages with a timestamp are rejected if older than 5 minutes; nonce-based messages also require a valid unconsumed nonce from `GET /api/portal/auth/nonce`.
- **Response:** Same shape as `GET /api/portal/me` on success (e.g. `ok`, `registered`, `account_id_hex`, `role`, …). Frontend then sets session via `BoingPortalSession.setSession(account_id_hex, role)` and redirects.

### 6.2 Frontend (sign-in page)

- **Sign in with wallet:** "Connect wallet" prefers `provider.request({ method: 'boing_requestAccounts' })` and `provider.request({ method: 'boing_signMessage', params: [message, address] })`, with alias fallback to `eth_requestAccounts` and `personal_sign`. After the user signs the message, the frontend calls `POST /api/portal/auth/sign-in` with `{ account_id_hex, message, signature }` (no password) and redirects on success.
- On the testnet portal, frontend checks `boing_chainId` and attempts `boing_switchChain` to `0x1b01` before signing.
- Message format now uses a nonce-backed multiline form:
  `Sign in to Boing Portal`
  `Origin: {origin}`
  `Timestamp: {new Date().toISOString()}`
  `Nonce: {serverNonce}`
- **Sign in with account ID:** User enters account ID and portal password; frontend calls `POST /api/portal/auth/sign-in-account` with `{ account_id_hex, password }`. If the account has no password set, API returns `403` with `need_password: true` and the frontend redirects to `/testnet/set-password`.
- No third-party SDKs (e.g. no ethers, viem, web3.js); only the injected provider and `fetch`.

### 6.3 Portal password (wallet vs account ID sign-in)

- **Wallet sign-in:** No portal password. The user approves and signs the sign-in message in the wallet; the backend verifies the signature and returns session data. No password field or step on the sign-in page for the wallet flow.
- **Account ID sign-in:** Requires the **portal password**. User enters account ID + password; backend verifies password via `POST /api/portal/auth/sign-in-account`. If the account has no password set, the API returns `403` with `need_password: true` and the frontend redirects to `/testnet/set-password`.
- **Registration** includes "Connect wallet" to fill the account ID and **Portal password** + **Confirm password** (min 8 characters). The backend stores a scrypt hash (salt + hash) in `portal_registrations.password_salt` and `portal_registrations.password_hash`.
- **Set password:** `POST /api/portal/auth/set-password` with `{ account_id_hex, message, signature, new_password }`. Message format: `Set portal password for Boing Portal\nOrigin: {origin}\nTimestamp: {ts}\nNonce: {nonce}`. Same nonce as sign-in; backend verifies Ed25519 signature then updates the stored hash.

### 6.4 `POST /api/portal/auth/sign-in-account`

- **Body (JSON):** `{ account_id_hex, password }`. Used for "Sign in with account ID" (no wallet).
- **Verification:** Backend loads `portal_registrations` for `account_id_hex`; if no row or no `password_hash`, returns `403` (with `need_password: true` and `account_id_hex` if registered but no password). Otherwise verifies password with stored salt/hash (scrypt) and returns the same payload as `POST /api/portal/auth/sign-in` on success.

### 6.5 Dependencies (network side)

- **Website/Functions:** No wallet-related npm packages. Portal uses only Astro and Wrangler; auth sign-in uses Node built-in crypto (Ed25519) via nodejs_compat in wrangler.toml. No ethers, viem, web3.js, MetaMask SDK, or similar.
- **Rust/node:** The chain uses ed25519-dalek for transaction and faucet signing; RPC does not implement personal_sign (signing is done in the wallet). CORS allows boing.express and boing.network origins for dApp to node communication.

---

## 7. Rollout, migrations, and smoke test (nonce-backed wallet auth)

This section covers deployment steps for the nonce-backed wallet sign-in flow.

### 7.1 Files involved

- `website/src/pages/testnet/sign-in.astro`, `set-password.astro`, register pages
- `website/functions/api/portal/auth/nonce.js`, `sign-in.js`, `set-password.js`, `password.js`, `register.js`
- `website/schema.sql`, `website/migrations/2026-03-06-portal-auth-nonces.sql`, `website/migrations/2026-03-06-portal-password.sql`
- `website/wrangler.toml`, `website/wrangler.worker.toml`

### 7.2 Database migrations

Apply before deploying the updated portal:

```bash
cd website
wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-auth-nonces.sql
wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-password.sql
```

(If applying full schema from scratch, these are already in `website/schema.sql`.)

### 7.3 Deployment

Auth uses Node built-in `crypto`; ensure `compatibility_flags = ["nodejs_compat"]` in `wrangler.toml` and `wrangler.worker.toml`.

### 7.4 Nonce and sign-in contract (reference)

- **`GET /api/portal/auth/nonce?origin=https://boing.network`** — Returns `{ ok, nonce, origin, issued_at, expires_at, message_template }`.
- **Sign-in message (exact UTF-8):**  
  `Sign in to Boing Portal`  
  `Origin: https://boing.network`  
  `Timestamp: <ISO>`  
  `Nonce: <server nonce>`
- **`POST /api/portal/auth/sign-in`** — Body: `{ account_id_hex, message, signature }`. Backend validates 32-byte address, 64-byte Ed25519 signature, message verification, timestamp, nonce (exists, matches origin, not expired, not used), and that the account is registered.

### 7.5 Smoke test checklist

After migration and deploy:

1. Open `/testnet/sign-in`; confirm wallet connect UI and preference for `window.boing` / `boing_requestAccounts`.
2. Confirm testnet chain `0x1b01` is requested; `/api/portal/auth/nonce` returns a nonce.
3. Confirm signing succeeds with connected, unlocked wallet; sign-in fails on replayed nonce, locked wallet, or unregistered address.
4. Confirm legacy address-only sign-in still works for testnet.

### 7.6 Compatibility

- Portal prefers `boing_requestAccounts`, `boing_signMessage`, `boing_chainId`, `boing_switchChain`; falls back to `eth_requestAccounts`, `personal_sign`, etc. when Boing methods are unsupported.
- Backend accepts older timestamp-only message format; nonce-based sign-in is the preferred flow.
