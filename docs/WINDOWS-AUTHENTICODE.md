# Windows Authenticode signing (Boing Network Hub)

**Status:** CI and docs scaffolding are in place. Public Hub Windows builds stay **unsigned**
until the org provisions a real code-signing `.pfx` into repository secrets. This document does
**not** include a certificate — never commit a `.pfx` or private key.

## Required GitHub Actions secrets

| Secret | Purpose |
|--------|---------|
| `WINDOWS_CERTIFICATE` | Base64-encoded PKCS#12 (`.pfx`) |
| `WINDOWS_CERTIFICATE_PASSWORD` | Export password for that `.pfx` |
| `WINDOWS_CERTIFICATE_THUMBPRINT` | Cert thumbprint injected into `desktop-hub/src-tauri/tauri.conf.json` at release time |

Release wiring lives in [`.github/workflows/release-desktop-hub.yml`](../.github/workflows/release-desktop-hub.yml)
(import PFX → set thumbprint → Tauri build). If any secret is missing, the Windows job **skips
signing** and still publishes an unsigned installer.

`certificateThumbprint` in `tauri.conf.json` remains `null` in git; CI overwrites it only when
`WINDOWS_CERTIFICATE_THUMBPRINT` is set. Timestamp URL is HTTPS DigiCert
(`https://timestamp.digicert.com`).

## Readiness check (no cert required)

From the repo root (or in CI):

```bash
# Locally: reports which env vars are unset (does not invent secrets)
./desktop-hub/scripts/check-windows-signing-ready.sh

# Or dispatch the workflow: Actions → "Desktop Hub — Windows signing readiness"
```

Exit codes:

| Code | Meaning |
|-----:|---------|
| 0 | All three signing inputs present (non-empty) |
| 1 | One or more missing — unsigned builds expected |
| 2 | Script usage / environment error |

## Operator steps once a cert exists

1. Obtain a **code signing** certificate (not TLS) from a CA Windows trusts.
2. Export PKCS#12, base64-encode, and set the three secrets above (see
   [desktop-hub/README.md](../desktop-hub/README.md#windows-code-signing-recommended)).
3. Re-run **Desktop Hub — Windows signing readiness** (expect exit 0).
4. Cut a `desktop-hub/v*` release; confirm the Windows job log shows signing enabled.

## Refs

- Audit leftover: https://github.com/Boing-Network/boing.network/issues/61
- Prior note: [#150](https://github.com/Boing-Network/boing.network/pull/150)
- Tauri docs: https://v2.tauri.app/distribute/sign/windows
