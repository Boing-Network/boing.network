# VibeMiner Network Listing — Boing Network (fill-in values)

Use these values to list **Boing Network** in the VibeMiner request listing form. Replace placeholders (e.g. release URL) with your actual URLs when you have them.

---

## Request listing form

| Field | Value |
|-------|--------|
| **Network name** | Boing Network |
| **Symbol** | BOING |
| **Environment** | Devnet (free, for testing) |
| **Algorithm** | Other (custom) |
| **Algorithm description** | PoS + HotStuff BFT for consensus; Ed25519 for signing; BLAKE3 for hashing. Validators stake BOING and produce blocks; no traditional mining. |
| **Mining pool** | *Leave empty / omit* — Boing is PoS/node-only; no pool required. |
| **Website (optional)** | https://boing.network |
| **Reward rate (optional)** | Variable (block rewards to validators; testnet faucet for users) |
| **Min. payout (optional)** | N/A (PoS; no pool payouts) |

### Node support (optional but recommended)

| Field | Value |
|-------|--------|
| **Node download URL (HTTPS)** | https://github.com/chiku524/boing.network/releases (or direct asset URL when you publish a release, e.g. `https://github.com/chiku524/boing.network/releases/download/v0.1.0/boing-node-...`) |
| **Command template** | `boing-node --p2p_listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --validator --rpc-port 8545 --data-dir {dataDir}` |
| **Disk (GB)** | 10 |
| **RAM (MB)** | 2048 |
| **Binary SHA256 (optional)** | *(Leave blank or fill per release for integrity)* |

**Notes for command template:**

- Use `{dataDir}` exactly as your form expects (some systems use `{dataDir}`, others `{data_dir}`).
- Bootnodes: current testnet bootnode is `/ip4/73.84.106.121/tcp/4001`. Canonical list is at [boing.network/network/testnet](https://boing.network/network/testnet) and in [TESTNET.md](TESTNET.md) §6. If VibeMiner supports configurable bootnodes (e.g. from a URL or env), you can document that so operators get the latest list.
- Omit `--validator` for a full-node-only run if the app offers that option.
- **Windows:** Build with `--no-default-features` to disable mDNS (see [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md)).

### Description (required)

**Suggested copy:**

Boing Network is an L1 blockchain with protocol-enforced quality assurance (only deployments meeting defined rules are accepted on-chain). Consensus is PoS + HotStuff BFT; signing and hashing use Ed25519 and BLAKE3. Run a **validator** by starting the node with `--validator` and staking BOING; testnet BOING is available via the faucet at https://boing.network/network/faucet or the RPC method `boing_faucetRequest`. No traditional mining; validators produce blocks. Public testnet RPC: https://testnet-rpc.boing.network/ — docs and bootnodes: https://boing.network/network/testnet.

---

## What to require for network listings (recommendations)

When **registering a network** in your application, these requirements can improve quality and safety:

1. **Environment + algorithm**
   - **Require** environment (e.g. Devnet / Testnet / Mainnet) and a clear algorithm field.
   - For **Other (custom)**, require a short description (e.g. “PoS + HotStuff BFT”) so users can tell PoW vs PoS vs other.

2. **Mining pool**
   - **Require** only for chains you classify as “mineable” (PoW). For PoS or node-only networks, do **not** require pool URL/port; the form already allows omitting.

3. **Node support (if offered)**
   - **Node download URL:** Require HTTPS and restrict to allowed hosts (e.g. GitHub releases, official domains) to avoid malicious binaries.
   - **Command template:** Require use of a single data-path placeholder (e.g. `{dataDir}`) so your app can inject the path safely. Validate that the template does not allow shell injection (no `;`, `|`, `$()`, etc.) or restrict to a safe subset.
   - **Disk / RAM:** Optional but helpful; recommend defaults per environment (e.g. Testnet vs Mainnet) if the submitter leaves them blank.
   - **Binary SHA256:** Strongly recommend when available; require for mainnet or when “verify integrity” is enabled.

4. **Description**
   - Keep **required**; short (1–2 paragraphs) so users know what the network is and how to get tokens (faucet, staking, etc.).

5. **Validation**
   - **Chain validation:** If you support it, optionally require a working RPC or “chain ID” and validate (e.g. call `boing_chainHeight` or equivalent) so only reachable networks are listed.
   - **Bootnodes / discovery:** For multi-node networks, consider requiring at least one bootnode or discovery URL so nodes can join; document the expected format (e.g. multiaddr list, or URL that returns JSON).

6. **Listing type**
   - Clearly separate “mineable (PoW)” vs “validator / full node (PoS or other)” in the UI so users don’t expect pool payouts for PoS chains.

---

*Source: [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md), [TESTNET.md](TESTNET.md), [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md).*
