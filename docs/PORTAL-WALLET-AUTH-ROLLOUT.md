# Portal Wallet Auth Rollout

This doc covers the rollout steps for the nonce-backed wallet sign-in flow added to `boing.network`.

## Scope

This rollout adds:

- Boing-native wallet connect on the portal sign-in page
- Replay-resistant wallet auth using a backend nonce
- Ed25519 verification using built-in Node crypto in Cloudflare Functions
- Testnet chain enforcement for the portal wallet flow

## Files Added or Updated

- `website/src/pages/testnet/sign-in.astro`
- `website/src/pages/testnet/set-password.astro` (new)
- `website/src/pages/testnet/register/user.astro`, `developer.astro`, `operator.astro`
- `website/functions/api/portal/auth/nonce.js`
- `website/functions/api/portal/auth/sign-in.js`
- `website/functions/api/portal/auth/set-password.js` (new)
- `website/functions/api/portal/auth/password.js` (new, shared hashing)
- `website/functions/api/portal/register.js`
- `website/schema.sql`
- `website/migrations/2026-03-06-portal-auth-nonces.sql`
- `website/migrations/2026-03-06-portal-password.sql` (new)
- `website/wrangler.toml`
- `website/wrangler.worker.toml`

## Database Migration

Apply the nonce-table migration before deploying the updated portal:

```bash
cd "C:\Users\chiku\Desktop\vibe-code\boing.network\website"
wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-auth-nonces.sql
```

Apply the portal-password columns (for wallet sign-in password requirement):

```bash
wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-password.sql
```

If you apply full schema from scratch instead, the same tables/columns are already included in `website/schema.sql`.

## Deployment Notes

The auth endpoint uses Node built-in `crypto`, so Cloudflare config must include:

```toml
compatibility_date = "2024-09-23"
compatibility_flags = ["nodejs_compat"]
```

This is already set in:

- `website/wrangler.toml`
- `website/wrangler.worker.toml`

## Portal Sign-In Contract

### Nonce endpoint

`GET /api/portal/auth/nonce?origin=https://boing.network`

Returns:

```json
{
  "ok": true,
  "nonce": "<server nonce>",
  "origin": "https://boing.network",
  "issued_at": "2026-03-06T12:00:00.000Z",
  "expires_at": "2026-03-06T12:05:00.000Z",
  "message_template": "Sign in to Boing Portal\nOrigin: {origin}\nTimestamp: {timestamp}\nNonce: {nonce}"
}
```

### Sign-in message

The portal now asks the wallet to sign this exact UTF-8 string:

```txt
Sign in to Boing Portal
Origin: https://boing.network
Timestamp: 2026-03-06T12:00:00.000Z
Nonce: <server nonce>
```

### Sign-in endpoint

`POST /api/portal/auth/sign-in`

Request body:

```json
{
  "account_id_hex": "0x...",
  "message": "Sign in to Boing Portal\nOrigin: https://boing.network\nTimestamp: 2026-03-06T12:00:00.000Z\nNonce: <server nonce>",
  "signature": "0x..."
}
```

Backend checks:

1. `account_id_hex` is a 32-byte public key
2. `signature` is a 64-byte Ed25519 signature
3. `message` verifies against the public key using Ed25519
4. timestamp is recent
5. nonce exists, matches origin, is not expired, and is not already used
6. account is registered in `portal_registrations`

## Smoke Test Checklist

After migration and deploy:

1. Open `/testnet/sign-in`.
2. Confirm wallet connect UI appears.
3. Confirm it prefers `window.boing` and works with `boing_requestAccounts`.
4. Confirm it attempts testnet chain `0x1b01`.
5. Confirm `/api/portal/auth/nonce` returns a nonce.
6. Confirm signing succeeds with a connected and unlocked wallet.
7. Confirm sign-in fails if the same signed nonce is replayed.
8. Confirm sign-in fails if the wallet is locked.
9. Confirm sign-in fails for an unregistered address.
10. Confirm the legacy address-only sign-in path still works for testnet.

## Known Compatibility Behavior

- The portal prefers `boing_requestAccounts`, `boing_signMessage`, `boing_chainId`, and `boing_switchChain`.
- It falls back to `eth_requestAccounts`, `personal_sign`, `eth_chainId`, and `wallet_switchEthereumChain` only when the Boing-native method is unsupported.
- The backend still accepts the older timestamp-only message format, but nonce-based sign-in is now the preferred flow.

## Follow-Up Recommendations

- Remove or de-emphasize address-only sign-in before mainnet.
- Add session invalidation or server-backed sessions if portal auth becomes sensitive.
- Periodically clean up expired rows from `portal_auth_nonces`.
- Add browser-level or edge rate limiting for `/api/portal/auth/nonce` and `/api/portal/auth/sign-in`.
