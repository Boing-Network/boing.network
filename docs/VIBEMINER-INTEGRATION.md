# Boing Network — VibeMiner Integration

> **Purpose:** How Boing testnet (and mainnet) can be offered as a one-click mining/validator experience in VibeMiner desktop apps.  
> **Audience:** VibeMiner maintainers and network operators who want to list Boing in the app.

---

## 1. What VibeMiner needs from Boing

To support "one-click" running of a Boing node (validator or full node) from VibeMiner, the app typically needs:

| Item | What Boing provides |
|------|----------------------|
| **Node binary** | `boing-node` (single executable; build from this repo or use a published release). |
| **How to run** | CLI flags: `--validator`, `--rpc-port`, `--data-dir`, `--p2p_listen`, `--bootnodes`, `--faucet-enable` (testnet). |
| **RPC** | JSON-RPC over HTTP on `--rpc-port` (default 8545). Methods: `boing_chainHeight`, `boing_submitTransaction`, etc. See [RPC-API-SPEC.md](RPC-API-SPEC.md). |
| **Testnet faucet** | RPC method `boing_faucetRequest([hex_account_id])` when node is started with `--faucet-enable`; or point users to the web faucet. |
| **Bootnodes** | Comma-separated multiaddrs for testnet/mainnet; published on [TESTNET.md](TESTNET.md) and website `/network/testnet`. |

No separate "miner" binary: **validating** is done by running `boing-node --validator`. PoS: validators stake BOING (bond/unbond via transactions).

---

## 2. Suggested integration flow in VibeMiner

1. **Discovery**  
   User selects "Boing Network" (testnet or mainnet) in the app.

2. **Binary**  
   - Either: bundle or download `boing-node` for the user's OS (Windows, macOS, Linux).  
   - Or: prompt user to install from [releases](https://github.com/boing-network/boing-network/releases) and detect `boing-node` in PATH.  
   - **Windows:** Build with `cargo build --release -p boing-node --no-default-features` to disable mDNS (avoids EADDRINUSE). Bootnodes use explicit `--bootnodes`, so mDNS is not needed. See [docs/INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md).

3. **One-click "Start node"**  
   VibeMiner runs something equivalent to:

   **Testnet:**

   ```text
   boing-node --p2p_listen /ip4/0.0.0.0/tcp/4001 \
     --bootnodes <OFFICIAL_TESTNET_BOOTNODES> \
     --validator \
     --rpc-port 8545 \
     --data-dir <USER_DATA_DIR>
   ```

   **Mainnet (when live):**

   ```text
   boing-node --p2p_listen /ip4/0.0.0.0/tcp/4001 \
     --bootnodes <OFFICIAL_MAINNET_BOOTNODES> \
     --validator \
     --rpc-port 8545 \
     --data-dir <USER_DATA_DIR>
   ```

   Omit `--validator` if the user only wants a full node. Use a dedicated `--data-dir` per network (e.g. `./boing-testnet-data` vs `./boing-mainnet-data`).

4. **Ports and firewall**  
   - **P2P:** port 4001 (TCP) — must be open for multi-node / testnet.  
   - **RPC:** port 8545 (default) — only needs to be reachable locally for VibeMiner; open publicly only if exposing RPC.

5. **Health / status**  
   Poll `http://127.0.0.1:8545/` (or the user's chosen RPC port) with:

   ```json
   {"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]}
   ```

   Show chain height and "Synced" / "Syncing" in the UI. See [RPC-API-SPEC.md](RPC-API-SPEC.md) for full method list.

6. **Faucet (testnet only)**  
   - **Option A (recommended):** In-app "Get testnet BOING" that calls the public testnet RPC `https://testnet-rpc.boing.network/` with `boing_faucetRequest([user_account_hex])`. No need to run a faucet locally.  
   - **Option B:** Link to the web faucet [boing.network/network/faucet](https://boing.network/network/faucet).

7. **Staking (validator)**  
   User must hold BOING and submit a `Bond` transaction (via RPC or a wallet that supports Boing). Validator set is derived from top stakers. VibeMiner can link to [TESTNET.md](TESTNET.md) or a "How to stake" page.

---

## 3. Where to get bootnodes and RPC URLs

| Network | Bootnodes | Public RPC (for faucet / read-only) |
|---------|-----------|-------------------------------------|
| **Testnet** | [TESTNET.md](TESTNET.md) §6; website [boing.network/network/testnet](https://boing.network/network/testnet) | `https://testnet-rpc.boing.network/` |
| **Mainnet** | To be published at mainnet launch | To be published |

**Testnet bootnodes (current):** Comma-separated multiaddrs, e.g. `/ip4/73.84.106.121/tcp/4001` (see [TESTNET.md](TESTNET.md) §6 and `website/src/config/testnet.ts`). Override via env `PUBLIC_BOOTNODES` when building the website.

**Testnet public RPC:** `https://testnet-rpc.boing.network/` — used for faucet (`boing_faucetRequest`) and read-only queries (`boing_chainHeight`, `boing_getBlockByHeight`, etc.). Override via env `PUBLIC_TESTNET_RPC_URL`.

**Why "no nodes" or "cannot connect"?** If VibeMiner shows no nodes or cannot join the testnet, it means bootnodes and/or the public RPC are not yet live. The Boing team must complete the steps in [LAUNCH-BLOCKING-CHECKLIST.md](LAUNCH-BLOCKING-CHECKLIST.md) first. VibeMiner can read config from the website, [TESTNET.md](TESTNET.md), or a small API so the app stays up to date without code changes.

---

## 4. Onboarding details you can provide

If you have **VibeMiner-specific onboarding** (e.g. app store links, install steps, or a "Add your network" form), we can:

- Link to it from [TESTNET.md](TESTNET.md) and the website "Join Testnet" / "One-click mining" section.
- Describe it in this doc (e.g. "To add Boing to VibeMiner, follow …").

Share the onboarding flow (or a draft) and we'll integrate it into the docs and site.

---

## 5. Summary

| Boing provides | Use in VibeMiner |
|----------------|------------------|
| `boing-node` binary | Run as process; optional bundling or PATH detection. Windows: build with `--no-default-features`. |
| `--validator`, `--rpc-port`, `--data-dir`, `--p2p_listen`, `--bootnodes` | Command line for "Start node" / "Start validator". |
| RPC on port 8545 (default) | Status (`boing_chainHeight`), faucet (`boing_faucetRequest`), block/tx queries. See [RPC-API-SPEC.md](RPC-API-SPEC.md). |
| Public RPC `https://testnet-rpc.boing.network/` | Faucet calls (no local faucet needed); read-only queries. |
| Bootnode list ([TESTNET.md](TESTNET.md) §6, [website](https://boing.network/network/testnet)) | So the node joins the testnet. |
| P2P port 4001, RPC port 8545 | Firewall: open 4001 for P2P; 8545 only if exposing RPC. |

No separate miner binary; no custom daemon protocol—just the node binary and JSON-RPC. For launch dependencies (bootnodes, public RPC), see [LAUNCH-BLOCKING-CHECKLIST.md](LAUNCH-BLOCKING-CHECKLIST.md).

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
