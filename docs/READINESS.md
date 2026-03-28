# Boing Network — Readiness

> **Purpose:** Single checklist for beta, six-pillar readiness, and launch-blocking items.  
> **References:** [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md), [RUNBOOK.md](RUNBOOK.md), [TESTNET.md](TESTNET.md), [BUILD-ROADMAP.md](BUILD-ROADMAP.md), [RPC-API-SPEC.md](RPC-API-SPEC.md), [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md).  
> **Incentivized testnet:** See [TESTNET.md](TESTNET.md) Part 3 for incentive design, promotion, and mainnet migration.

---

## 1. Beta Quality Checklist

### 1.1 Build & Test

| Item | Command | Status |
|------|---------|--------|
| Workspace builds | `cargo build --release` | ✓ |
| All tests pass | `cargo test` | ✓ |
| CLI binary | `target/release/boing` | ✓ |
| Node binary | `target/release/boing-node` | ✓ |

### 1.2 Developer Tools (CLI)

| Tool | Command | Purpose |
|------|---------|---------|
| **boing** | `boing init [name]` | Scaffold dApp project |
| | `boing dev [--port 8545]` | Start local chain |
| | `boing deploy [path]` | Verify RPC; deploy via `boing_submitTransaction` |
| | `boing metrics register` | Register dApp for incentive tracking |
| | `boing completions <shell>` | Shell completion |
| | `boing --version` | Report CLI version |

### 1.3 Validator & Node Operator Tools

| Tool | Command | Purpose |
|------|---------|---------|
| **boing-node** | `boing-node --help` | Show options |
| | `boing-node --validator --rpc-port 8545 --data-dir ./data` | Run as validator |
| | `boing-node --rpc-port 8545 --data-dir ./data` | Run as full node |
| | `boing-node --p2p_listen /ip4/0.0.0.0/tcp/4001 ...` | Enable P2P for testnet |

### 1.4 Documentation

| Doc | Purpose |
|-----|---------|
| [RUNBOOK.md](RUNBOOK.md) | Node setup, RPC, CLI, monitoring |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | Full RPC reference |
| [TESTNET.md](TESTNET.md) | Single vs multi-node, bootnodes, faucet |
| [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) | One-click mining integration |
| [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) | Bootnodes, Cloudflare tunnel |

---

## 2. Six Pillars — Pre-Infrastructure Checklist

Complete these **before** running bootnodes. See [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) for pillar definitions.

| # | Item | Pillar |
|---|------|--------|
| 1 | Enforce `pending_txs_per_sender` in mempool | Security |
| 2 | Document security contacts in SECURITY-STANDARDS | Security |
| 3 | Add "Scalability characteristics" (block time, TPS) to RUNBOOK | Scalability |
| 4 | Add "Decentralization design" note to RUNBOOK | Decentralization |
| 5 | [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) Appendix A (deployer checklist) | QA, Transparency |
| 6 | [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) Appendix B (canonical malice definition) | QA, Transparency |

---

## 3. Launch-Blocking Checklist (Critical Path)

**Why VibeMiner shows "no nodes":** Bootnodes and public RPC must be running. Until then, VibeMiner, terminal validators, and boing.observer cannot use the testnet.

### 3.1 Run Bootnodes

| Step | Action | Done |
|------|--------|------|
| 1.1 | Run at least **2** `boing-node` with stable public IPs, P2P enabled | ☐ |
| 1.2 | Open TCP 4001 (P2P) and optionally 8545 (RPC) | ☐ |
| 1.3 | Record multiaddrs (e.g. `/ip4/1.2.3.4/tcp/4001`) | ☐ |

**Scripts:** [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md). Use `scripts/start-bootnode-1.bat` / `start-bootnode-2.bat` (or `.sh`).

### 3.2 Run Faucet + Public RPC

| Step | Action | Done |
|------|--------|------|
| 2.1 | Run node with `--faucet-enable` and `--bootnodes` | ☐ |
| 2.2 | Expose at `https://testnet-rpc.boing.network/` (Cloudflare tunnel) | ☐ |
| 2.3 | Confirm `boing_faucetRequest` works | ☐ |

### 3.3 Update Config and Docs

| Step | Action | Done |
|------|--------|------|
| 3.1 | Set `PUBLIC_BOOTNODES` in deploy env or `website/.env` | ☐ |
| 3.2 | Set `PUBLIC_TESTNET_RPC_URL` to public faucet RPC | ☐ |
| 3.3 | Update [TESTNET.md](TESTNET.md) §6 bootnode table | ☐ |
| 3.4 | Rebuild and deploy website | ☐ |

### 3.4 Verification

After steps 1–3:

- **VibeMiner** — Should connect to bootnodes and sync
- **Faucet** — [boing.network/faucet](https://boing.network/faucet) succeeds
- **boing.observer** — With testnet RPC configured, shows blocks
- **Terminal** — `boing-node --bootnodes <LIST> --validator` syncs

---

## 4. Pre-Beta Verification Commands

```bash
cargo build --release
cargo test
./target/release/boing --version
./target/release/boing init test-dapp-beta --output /tmp/boing-beta-test
./target/release/boing-node --help
```

---

## 5. Next Steps

- **Beta:** Iterate on RUNBOOK and docs from common questions.
- **Incentivized testnet:** Use [TESTNET.md](TESTNET.md) Part 3 for full launch checklist, incentive design, and promotion.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
