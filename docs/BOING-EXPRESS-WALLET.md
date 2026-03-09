# Boing Express — Wallet: Bootstrap, Integration & Chrome Web Store

Use this document to **bootstrap**, **integrate**, and **publish** **Boing Express** — the wallet for the Boing Network. Product name is always **Boing Express** (not "Boing Wallet"). Domain: **boing.express**. This doc merges the creation prompt, full Boing integration checklist, and Chrome Web Store readiness.

---

## Table of Contents

1. [Part 1: Bootstrap / Creation](#part-1-bootstrap--creation)
2. [Part 2: Full integration & Chrome Web Store](#part-2-full-integration--chrome-web-store)
3. [Network independence](#network-independence)
4. [Reference: Boing Network repo](#reference-boing-network-repo)
5. [Quick reference: RPC methods for the wallet](#quick-reference-rpc-methods-for-the-wallet)
6. [Session-based lock and unlock](#session-based-lock-and-unlock)

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
- Testnet faucet: boing_faucetRequest or link to boing.network/network/faucet with address.
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
- [ ] **Connection approval (required for every site):** When **any** website or dApp with Boing integration calls `boing_requestAccounts` (or `eth_requestAccounts`), the wallet **must** show a connection-approval UI (e.g. "Allow [origin] to view your address?" or "Connect to [site name]?") and **only return accounts after the user explicitly approves**. Do not return accounts without user approval, regardless of origin—this applies to boing.network, boing.express, and any other site that integrates Boing. Similarly, for `boing_signMessage` / `personal_sign`, show what is being signed (e.g. "Connect to Boing Portal" or the dApp’s message) and require the user to approve before returning a signature. Storing "connected sites" per origin is optional; the important part is that the **first** time a site requests accounts (or after the user disconnects), the wallet always prompts for approval.
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

## Network independence

**Boing Network is chain-independent.** It does not depend on EVM (Ethereum, Base, etc.), Solana, or any other external network for its core protocol, addresses, or signing.

- **Addresses:** 32-byte Ed25519 public keys (64 hex chars). Not EVM 20-byte addresses or Solana base58.
- **Signing:** Ed25519 only. Not secp256k1 (Ethereum) or Solana-specific formats.
- **RPC & transactions:** Boing-specific JSON-RPC and bincode serialization. No EVM ABI, no Solana SPL.
- **Wallet integration:** Boing Express (and any Boing-native wallet) implements `boing_requestAccounts`, `boing_signMessage`, and transaction signing with **Boing’s own formats**. For **portal/dApp message signing** (`boing_signMessage`), the wallet signs **BLAKE3(UTF-8 message)** with Ed25519 (same as Boing tx signing); the portal verifies Ed25519 over BLAKE3(message). No reliance on other chains’ signing (e.g. no EVM `personal_sign`).

When integrating with Boing (wallet, portal, or node), use Boing’s specs only. Do not assume Ethereum or Solana compatibility.

### Portal sign-in 401 (invalid_signature)

If the portal returns **401 Unauthorized** with `invalid_signature` when you click "Sign in" after connecting Boing Express:

1. **Redeploy the portal** — The live site (boing.network) must be running the sign-in API that verifies **BLAKE3(message)**. Push to `main` to trigger the GitHub Action, or run from repo: `cd website && npm run deploy` (requires `CLOUDFLARE_API_TOKEN`). After changing `website/functions/api/portal/auth/sign-in.js`, a deploy is required for the change to take effect.
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
| Faucet | RPC boing_faucetRequest; or boing.network/network/faucet |

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

---

*This document lives in the boing-network repo so the Boing Express team can align with the chain spec and ship the web app and Chrome extension.*
