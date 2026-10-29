# Boing Network — Node listing API (`GET /api/networks`)

> 👋 **Everyday users:** you do not need this file. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with [website/functions/api/networks.js](../website/functions/api/networks.js).
> 🛰️ **Operators:** upgrading `boing-node` on a laptop does **not** change [boing.observer](https://boing.observer) — that uses public RPC. See [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) §2.1.

**Purpose:** Public, read-only JSON that lists the official testnet node packages (download URL, SHA-256, command template) plus a `meta` block of defaults (RPC URL, bootnodes, chain id, release tag). It is used by [boing.express](https://boing.express) for runtime network discovery and by the website's [Join Testnet](https://boing.network/testnet/join) page; node runners and other tools can use it too.

---

## Where to get bootnodes and RPC URLs

| Network | Bootnodes | Public RPC (for faucet / read-only) |
|---------|-----------|-------------------------------------|
| **Testnet** | [TESTNET.md](TESTNET.md) §6; website [boing.network/testnet/join](https://boing.network/testnet/join) | `https://testnet-rpc.boing.network/` |
| **Mainnet** | To be published at mainnet launch | To be published |

**Testnet bootnode (current):** `/ip4/169.155.48.188/tcp/4001` (Fly.io `boing-testnet-1`; see [TESTNET.md](TESTNET.md) §6 and `website/src/config/testnet.ts`). Override via env `PUBLIC_BOOTNODES` when building the website.

**Testnet public RPC:** `https://testnet-rpc.boing.network/` — used for the faucet (`boing_faucetRequest`) and read-only queries (`boing_chainHeight`, `boing_getBlockByHeight`, etc.). Override via env `PUBLIC_TESTNET_RPC_URL`.

If a client shows no nodes or cannot connect, bootnodes and/or the public RPC are not live; see [READINESS.md](READINESS.md) §3.

---

## Response shape

| Mechanism | Shape |
|-----------|--------|
| **This API + D1** | Top-level **`meta`** (download tag, RPC URL, bootnodes, chain id, CLI flag style, doc links) plus **`networks[]`**: `boing-devnet` (Windows), `boing-devnet-linux`, `boing-devnet-macos` (each with `platform`, optional `node_*` fields from D1). |

**`meta` fields (stable contract; extra keys may appear):**

| Field | Meaning |
|-------|---------|
| **`boing_testnet_download_tag`** | GitHub release tag for the official zips. |
| **`chain_id_hex`** | Testnet chain id (`0x1b01`). |
| **`public_testnet_rpc_url`** | Public JSON-RPC (faucet / read-only); same as each row's **`rpc_url`**. |
| **`official_bootnodes`** | Multiaddr strings; comma-join for `--bootnodes`. |
| **`cli_long_flags`** | Always **`kebab-case`** (`--p2p-listen`, not `--p2p_listen`). |
| **`docs`** | URLs to [NODE-OPERATOR-COMMANDS.md](NODE-OPERATOR-COMMANDS.md) (**`node_operator_commands`**) and [**`tools/boing-node-public-testnet.env.example`**](https://github.com/Boing-Network/boing.network/blob/main/tools/boing-node-public-testnet.env.example) (**`boing_node_public_env_example`**) — operator copy-paste for chain id, **`BOING_CANONICAL_NATIVE_*`**, and **`BOING_DEX_*`** discovery tuning. |
| **`ecosystem`** | Canonical **`wallet_url`** (boing.express), **`explorer_url`** (boing.observer), **`website_url`**, **`discord_url`** (community Discord invite), and GitHub links for [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md), [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md), [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md). Clients may surface these for "Get wallet" / "View explorer" without hard-coding domains. |

**Canonical GitHub org in JSON:** [website/functions/api/networks.js](../website/functions/api/networks.js) normalizes official **`boing.network`** repo URLs in **`meta`** and in **`node_download_url`** (when D1 still stores legacy **`chiku524/boing.network`**) to **`Boing-Network/boing.network`**.

**Source of truth order:** live **`/api/networks`** → GitHub release assets → this repo's **`networks.js`** constant **`BOING_TESTNET_DOWNLOAD_TAG`**.

---

## Refreshing D1 after a release

After a new `boing-node` GitHub release tag (replace the tag with the one you published):

```bash
cd website
node scripts/network-listings-release-sql.mjs testnet-v0.1.19
# Optional: apply (use CLOUDFLARE_API_TOKEN if OAuth fails on D1 import)
node scripts/network-listings-release-sql.mjs testnet-v0.1.19 --apply
```

Canonical hand-maintained SQL: [website/migrations/insert-boing-devnet-listing.sql](../website/migrations/insert-boing-devnet-listing.sql). For new tags, regenerate with **`node scripts/network-listings-release-sql.mjs <tag>`** (see [TESTNET.md](TESTNET.md) §9.1).

---

## Command template notes

- Use kebab-case flags only (`--p2p-listen`, `--data-dir`, `--rpc-port`, `--bootnodes`, `--faucet-enable`, `--validator`). Underscore forms are **rejected** by current `boing-node` (clap).
- Example: `boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable` (add `--validator` for a validator; Linux/macOS use `boing-node-linux-x86_64` / `boing-node-macos-aarch64`).
- **Windows:** build with `--no-default-features` to disable mDNS (see [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md)).
- **Rate limits:** for a **local** node behind heavy wallet/indexer traffic, operators may set env **`BOING_RATE_PROFILE=dev`** or pass **`--dev-rate-limits`** (see [RUNBOOK.md](RUNBOOK.md) §2). Do **not** use the dev profile for **public** RPC hosts.
- **Firewall:** open TCP 4001 for P2P; 8545 only if you expose RPC.

---

*Boing Network — Quality, enforced by the protocol.*
