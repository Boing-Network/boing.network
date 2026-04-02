# Testnet node release tags (`boing-node` zips)

GitHub Actions builds **`release-{linux-x86_64,macos-aarch64,windows-x86_64}.zip`** when you push a tag matching `testnet*` or `v*` (see `.github/workflows/release.yml`).

## Ship a new testnet binary (e.g. after new JSON-RPC)

1. Ensure `main` has the code you want (e.g. `boing_getQaRegistry`).
2. Create and push an **annotated** tag (example `testnet-v0.1.7`):

   ```bash
   git checkout main
   git pull
   git tag -a testnet-v0.1.7 -m "Testnet node: consensus round fix after persistence"
   git push origin testnet-v0.1.7
   ```

3. Wait for workflow **Release binaries** to finish. For **`testnet*`** tags the workflow **publishes** the release immediately so `https://github.com/.../releases/download/<tag>/...` works. For **`v*`** tags it still creates a **draft** until you publish manually.
4. If you ever see HTTP 404 from the refresh script, the release is almost certainly still a **draft** (or the workflow has not finished uploading). Open **GitHub → Releases** and click **Publish release** on the draft.
5. Refresh Boing website D1 listing SHAs (either from repo root or `website/`):

   ```bash
   # from repo root:
   node scripts/network-listings-release-sql.mjs testnet-v0.1.7
   node scripts/network-listings-release-sql.mjs testnet-v0.1.7 --apply
   # or from website/: cd website && node scripts/network-listings-release-sql.mjs testnet-v0.1.7 [--apply]
   ```

7. Bump **`BOING_TESTNET_DOWNLOAD_TAG`** in **`website/functions/api/networks.js`** (and **`BOING_ZIP_SHA`** if you use URL rewrite helpers) so **`GET /api/networks`** returns the new **`meta.boing_testnet_download_tag`**. Deploy **boing.network** Pages/Workers. Then bump **VibeMiner** defaults (`BOING_TESTNET_DEFAULT_DOWNLOAD_TAG`, `networks.ts`) to match **`meta`** — see [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) §6.
