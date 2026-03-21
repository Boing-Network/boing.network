# Boing Network Hub

All-in-one **desktop application** for the Boing Network ecosystem, built with **Tauri 2** (Rust + web frontend). It bundles access to:

- **[Observer](https://boing.observer)** — Block explorer (blocks, accounts, faucet, QA check)
- **Wallet** — Same app as the [boing.express Chrome extension](https://boing.express) (send, stake, dApp connect)
- **[Finance](https://boing.finance)** — DEX & DeFi (swap, liquidity, bridge, portfolio)
- **Testnet** — Testnet ecosystem (register, faucet, quests, developers). Later: general network hub for users.

Users get a single window with a sidebar to switch between these apps, each loaded in an embedded view (iframe) pointing to the live sites. No need to open multiple browser tabs.

**Desktop app experience:** On launch, the app shows a short intro animation (optional “Don’t show intro on next launch”), then checks for updates (with progress if an update is downloading). If the user has not yet dismissed the welcome screen, they see **Sign in**, **Register**, or **Continue without account**; otherwise they go straight to the home dashboard. In the sidebar footer: **Sign in** (when not signed in) opens the welcome screen again; **Sign out** clears session and returns to welcome; **Settings** offers “Show welcome on next launch”, “Show intro on next launch”, and “Check for updates”. The process plugin is used to relaunch the app after an update is installed.

## Prerequisites

- **Node.js** 18+ and **npm** (or pnpm/yarn)
- **Rust** (latest stable): [rustup](https://rustup.rs/)
- **Tauri 2 system deps**: [Tauri — Prerequisites](https://v2.tauri.app/start/prerequisites/)

If on Windows you see **"An Application Control policy has blocked this file" (os error 4551)** when running `tauri dev` or `tauri build`, allow Rust/cargo build scripts in your security policy (e.g. Windows Defender Application Control or corporate policy), or run from a path/folder that is not restricted.

**Installed app won’t open (shortcut does nothing, no process in Task Manager):** The app may be exiting immediately. On Windows it writes a **crash log** when it fails: check `%LOCALAPPDATA%\network.boing.hub\crash-log.txt` (e.g. open Run → `%LOCALAPPDATA%\network.boing.hub` and open `crash-log.txt`). Install or repair the **WebView2** runtime from [Microsoft’s WebView2 page](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (Evergreen Standalone Installer). If the log shows a path or permission error under that folder, try running the app once as Administrator so the folder can be created, then run normally. You can also check **Event Viewer** → Windows Logs → Application for a crash entry.

## Quick start

```bash
cd desktop-hub
npm install
```

Generate app icons (required for build; use the repo favicon):

```bash
npm run tauri icon public/favicon.svg
```

Run in development:

```bash
npm run tauri:dev
```

Build for production:

```bash
npm run tauri:build
```

Outputs (installers and binaries) are under `src-tauri/target/release/` and `src-tauri/target/release/bundle/`.

## Configuration

Embedded app URLs are set in `src/config.ts` and can be overridden with env vars at build time:

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_OBSERVER_URL` | `https://boing.observer` | Observer (block explorer) |
| `VITE_EXPRESS_URL` | `https://boing.express` | Wallet (same as Chrome extension web app) |
| `VITE_FINANCE_URL` | `https://boing.finance` | Finance (DEX) |
| `VITE_NETWORK_URL` | `https://boing.network/testnet` | Testnet ecosystem (later: network hub) |

Example for local/staging:

```bash
VITE_OBSERVER_URL=http://localhost:3000 VITE_EXPRESS_URL=http://localhost:5173 npm run tauri:dev
```

## Project layout

```
desktop-hub/
  src/                 # Vite + React frontend (hub shell)
    config.ts          # App URLs
    App.tsx            # Phase flow (intro → update → welcome/app) + sidebar
    lib/               # Storage helpers (welcome, signed-in, show intro)
    components/        # Intro, UpdateOverlay, HubFooter, AppIcons
    hooks/             # useUpdateCheck
    views/             # Home, Embed, Welcome
  src-tauri/           # Tauri 2 Rust backend
    tauri.conf.json    # Window, build, bundle config
    capabilities/     # Permissions (e.g. shell open)
    src/               # Rust entry (lib.rs, main.rs)
    icons/             # App icons (generate with `tauri icon`)
  public/
    favicon.svg        # Used for icon generation and shell branding
```

## Auto-updates

The hub uses [Tauri’s updater plugin](https://v2.tauri.app/plugin/updater/): **Rust** (`tauri-plugin-updater` in `Cargo.toml` + `lib.rs`), **`tauri.conf.json`** (`bundle.createUpdaterArtifacts`, `plugins.updater.pubkey` + `endpoints`), and **capabilities** (`updater:default`).

- On first launch (after intro), the app checks for updates with a **network timeout** so offline or slow networks do not hang indefinitely.
- **Settings → Check for updates** shows **“You’re on the latest version”** when nothing newer is available, or a **dismissible error** if the check fails.
- When an update is found, the UI shows progress; after install, **`tauri-plugin-process`** relaunches the app (Windows uses **passive** install mode by default).

**Update manifest URL:** `plugins.updater.endpoints` points at GitHub’s `…/releases/latest/download/latest.json`. That file is uploaded by [`tauri-action`](../.github/workflows/release-desktop-hub.yml) when builds are signed. Ensure the **latest** GitHub Release is a desktop-hub release when you ship hub updates (or publish `latest.json` from another HTTPS URL you control and change the endpoint).

**Signing:** Updates are signed with a minisign keypair. The **public** key is in `tauri.conf.json`. The **private** key must **never** be committed (`desktop-hub/.tauri/` is gitignored). Release CI reads it from GitHub Secrets (see below). To generate a new pair locally (empty password example):

```bash
cd desktop-hub
# Unset CI in some shells if the CLI prompts incorrectly
npx @tauri-apps/cli@2 signer generate -w .tauri/boing-hub.key -f -p ""
# Paste contents of .tauri/boing-hub.key.pub into plugins.updater.pubkey in tauri.conf.json
```

## Tech stack

- **Shell**: React 18 + TypeScript + Vite
- **Desktop**: Tauri 2 (Rust), with `tauri-plugin-shell` (open links), `tauri-plugin-process` (relaunch after update), and `tauri-plugin-updater` (signed updates)
- **Embedded apps**: Loaded via iframe from production (or configured) URLs; no code from observer/express/finance is bundled into the hub

## GitHub Release (CI)

Releases are built and published automatically via GitHub Actions.

### What you need to do once

1. **Workflow permissions**  
   In the repo: **Settings → Actions → General → Workflow permissions** → select **Read and write permissions** (so the workflow can create the release and upload assets). `GITHUB_TOKEN` is provided by GitHub.

2. **Updater signing secrets (required for release builds)**  
   `bundle.createUpdaterArtifacts` is enabled, so **`tauri build` needs the private key** to produce signatures and `latest.json`. Add these repository secrets (same values you would use locally as env vars):

   | Secret | Description |
   |--------|-------------|
   | `TAURI_SIGNING_PRIVATE_KEY` | Full text of the minisign **private** key file (e.g. contents of `desktop-hub/.tauri/boing-hub.key`) |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Optional; leave empty or omit if the key has no password |

   If this secret is missing or wrong, the release workflow will fail at build time. **Back up the private key** outside the repo; losing it means you cannot ship signed updates to existing installs until you rotate the public key in `tauri.conf.json` and ship a one-time manual upgrade.

3. **Create the release** (first time or after a new version):
   - **Option A — From a new tag:** Push a tag `desktop-hub/vX.Y.Z` (e.g. `desktop-hub/v0.1.0`). The workflow [release-desktop-hub.yml](../.github/workflows/release-desktop-hub.yml) runs, builds Windows (MSI), macOS (Intel + Apple Silicon DMG), and Linux (Debian + AppImage), and creates/updates the GitHub Release with those assets.
   - **Option B — Manual run:** In **Actions → Release Boing Network Hub** → **Run workflow**. This builds from the default branch and publishes to the tag `desktop-hub/v{VERSION}` from `tauri.conf.json` (e.g. `desktop-hub/v0.1.0`). Use this to backfill an existing tag with installers.

4. **Downloads page**  
   [boing.network/downloads](https://boing.network/downloads) uses direct-download URLs to these release assets. If Tauri outputs different filenames, update the `hubDownloads` list in `website/src/pages/downloads.astro`.

### Windows code signing (recommended)

Signing the Windows installer avoids SmartScreen warnings and “administrator has set policies to prevent the installation” on locked-down machines. The release workflow signs the Windows build when these GitHub secrets are set:

| Secret | Description |
|--------|-------------|
| `WINDOWS_CERTIFICATE` | Base64-encoded `.pfx` (PKCS#12) code signing certificate |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password used when creating the `.pfx` export |
| `WINDOWS_CERTIFICATE_THUMBPRINT` | Certificate thumbprint (from `certmgr.msc` → certificate → Details → Thumbprint) |

**Getting a code signing certificate:** Use a **code signing** certificate (not an SSL cert) from a provider such as [DigiCert](https://www.digicert.com/signing/code-signing-certificates), [Sectigo](https://sectigo.com/ssl-certificates-tls/code-signing), or others listed in [Microsoft’s docs](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/code-signing-cert-manage). The **publisher name** shown in Windows (SmartScreen, installer) is taken from the certificate—request the cert in the name you want users to see (e.g. **nico.builds** as organization or DBA).

**Preparing the certificate for CI:**

1. **Create a `.pfx` file** (if you have `.cer` + private key):
   ```bash
   openssl pkcs12 -export -in cert.cer -inkey private-key.key -out certificate.pfx
   ```
   Set an export password when prompted and store it for `WINDOWS_CERTIFICATE_PASSWORD`.

2. **Base64-encode the `.pfx`** (Windows):
   ```cmd
   certutil -encode certificate.pfx base64cert.txt
   ```
   Use the contents of `base64cert.txt` (single line or multi-line) as the value for the `WINDOWS_CERTIFICATE` secret.

3. **Get the thumbprint:** Import the `.pfx` on a Windows machine (e.g. `Import-PfxCertificate -FilePath certificate.pfx -CertStoreLocation Cert:\CurrentUser\My`), then open **certmgr.msc** → Personal → Certificates → double-click the cert → Details → **Thumbprint**. Use that value (with or without spaces) for `WINDOWS_CERTIFICATE_THUMBPRINT`.

Once these three secrets are set, the next Windows build in the release workflow will sign the installer and executable. If the secrets are not set, the Windows build still runs but the output is unsigned.

**Local signed build:** To sign locally, set `certificateThumbprint` and (if needed) `timestampUrl` in `desktop-hub/src-tauri/tauri.conf.json`, import the same `.pfx` into `Cert:\CurrentUser\My`, and run `npm run tauri:build` in `desktop-hub`. See [Tauri — Windows code signing](https://v2.tauri.app/distribute/sign/windows).

### Other builds

The workflow uses the default `GITHUB_TOKEN` only. No other repository secrets are required for building or publishing **unsigned** builds.

## Why Tauri over Electron

- Smaller binaries and lower memory use
- System WebView instead of bundling Chromium
- Rust backend for security and performance
- Same web frontend (React) as the rest of the ecosystem

## License

Same as the Boing Network repository.
