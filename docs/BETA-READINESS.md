# Boing Network — Beta Readiness

> **Purpose:** Ensure all tools, documentation, and operations are ready for public testing and beta onboarding.  
> **References:** [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) (six pillars and essentials), [RUNBOOK.md](RUNBOOK.md), [BUILD-ROADMAP.md](BUILD-ROADMAP.md), [RPC-API-SPEC.md](RPC-API-SPEC.md), [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md)

---

## 1. Quality Checklist

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
| **boing** | `boing init [name]` | Scaffold dApp project (valid name: alphanumeric, `-`, `_`) |
| | `boing dev [--port 8545]` | Start local chain (uses `boing-node` from same dir or PATH, else `cargo run -p boing-node`) |
| | `boing deploy [path]` | Verify RPC connectivity; deployment is via `boing_submitTransaction` |
| | `boing metrics register --contract <hex> --owner <hex>` | Register dApp for incentive tracking |
| | `boing completions <shell>` | Shell completion (bash, zsh, fish, powershell, elvish) |
| | `boing --version` | Report CLI version for support |

**Quality bar:** No panics on valid input; clear errors on invalid input; `boing init` rejects empty or invalid project names.

### 1.3 Validator & Node Operator Tools

| Tool | Command | Purpose |
|------|---------|---------|
| **boing-node** | `boing-node --help` | Show options |
| | `boing-node --validator --rpc-port 8545 --data-dir ./data` | Run as validator (produces blocks) |
| | `boing-node --rpc-port 8545 --data-dir ./data` | Run as full node (no block production) |
| | `boing-node --p2p_listen /ip4/0.0.0.0/tcp/4001 ...` | Enable P2P for testnet |

**Quality bar:** Node starts and serves RPC; validator produces blocks when `--validator`; data persists in `--data-dir`.

### 1.4 RPC API

All methods documented in [RPC-API-SPEC.md](RPC-API-SPEC.md) are implemented and tested:

- `boing_submitTransaction`, `boing_chainHeight`, `boing_getBlockByHeight`, `boing_getBlockByHash`
- `boing_getAccountProof`, `boing_verifyAccountProof`
- `boing_simulateTransaction`, `boing_registerDappMetrics`, `boing_submitIntent`

Rate limiting and error codes match the spec.

### 1.5 Documentation

| Doc | Purpose |
|-----|---------|
| [RUNBOOK.md](RUNBOOK.md) | Node setup, RPC, CLI, monitoring, incident response |
| [BUILD-ROADMAP.md](BUILD-ROADMAP.md) | Quick start, phases, current status |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | Full RPC reference |
| [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) | Security posture and incident contacts |
| [TESTNET.md](TESTNET.md) | Single vs multi-node, bootnodes, faucet, VibeMiner |
| [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) | One-click mining/validator integration for VibeMiner |
| [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md) | Website spec, site structure, Cloudflare setup and deployment |

---

## 2. Validator Quick Start (Beta)

1. **Build**
   ```bash
   cargo build --release
   ```

2. **Run validator**
   ```bash
   ./target/release/boing-node --validator --rpc-port 8545 --data-dir ./boing-data
   ```

3. **Stake (on-chain)**  
   Submit a `Bond` transaction via `boing_submitTransaction` to add stake; validator set is derived from top stakers (see [boing-state](crates/boing-state) `top_stakers`).

4. **Monitor**
   - Chain height: `curl -s -X POST http://127.0.0.1:8545/ -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]}'`
   - Logs: `RUST_LOG=info ./target/release/boing-node ...`

5. **Testnet (multi-node)**  
   Use `--p2p_listen /ip4/0.0.0.0/tcp/4001` and connect to bootnodes (see RUNBOOK and testnet docs when available).

---

## 3. Developer Quick Start (Beta)

1. **CLI**
   ```bash
   cargo build --release
   ./target/release/boing init my-dapp
   cd my-dapp && ../target/release/boing dev
   ```
   In another terminal:
   ```bash
   ./target/release/boing deploy . --rpc-url http://127.0.0.1:8545
   ```

2. **RPC base URL**  
   `http://127.0.0.1:8545/` (or `--rpc-url` for CLI).

3. **First transaction**  
   Sign a transaction (see [boing-primitives](crates/boing-primitives)), serialize with bincode, hex-encode, and call `boing_submitTransaction` with `[hex_signed_tx]`.

4. **Simulate first**  
   Use `boing_simulateTransaction` to dry-run before submitting.

---

## 4. Known Limitations (Beta)

- **Single-validator default:** Without P2P, node runs as single validator; multi-validator testnet requires `--p2p_listen` and peer config.
- **No faucet UI yet:** Testnet faucet and bootnode list to be provided via website/network docs.
- **Deploy command:** `boing deploy` only checks RPC connectivity; actual deployment is via RPC `boing_submitTransaction` (ContractDeploy payload).
- **Native AA:** Account abstraction is on roadmap, not yet in beta.

---

## 5. Pre–Beta Verification Commands

Run these before opening beta:

```bash
# Build and test
cargo build --release
cargo test

# CLI smoke test
./target/release/boing --version
./target/release/boing --help
./target/release/boing init test-dapp-beta --output /tmp/boing-beta-test
./target/release/boing completions bash > /dev/null

# Node help
./target/release/boing-node --help
```

After beta launch, collect feedback on: CLI UX, runbook clarity, RPC errors, and validator onboarding friction.

---

## 6. Next Steps After Verification (Pre-Beta ✓)

Once the pre-beta commands succeed (build, test, `boing --version`, `boing-node --help`), proceed as follows.

### 6.1 Optional: Quick smoke test

In one terminal:

```bash
./target/release/boing-node --validator --rpc-port 8545
```

In another:

```bash
curl -s -X POST http://127.0.0.1:8545/ -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]}'
# Expect: {"jsonrpc":"2.0","id":1,"result":0} or higher
```

Stop the node with Ctrl+C. This confirms the node serves RPC and produces blocks.

### 6.2 Prepare for public beta

| Step | Action |
|------|--------|
| **1. Testnet info** | Decide testnet name, genesis time (if applicable), and document bootnodes/faucet or “single-node first” in [RUNBOOK.md](RUNBOOK.md) or `/network/testnet` on the website. |
| **2. Website** | Ensure [boing.network](https://boing.network) (or staging) has “Join Testnet” / “Get Started” pointing to [BETA-READINESS.md](BETA-READINESS.md) or the docs quick start. |
| **3. Binaries (optional)** | For users without Rust: build release artifacts and publish (e.g. GitHub Releases: `boing`, `boing-node` for Windows, Linux, macOS). |
| **4. Feedback channel** | Set up Discord, GitHub Discussions, or a form so beta testers can report issues and UX feedback. |
| **5. Announce** | Announce beta (blog, Twitter, Discord) with: repo link, “Quick Start” (build + run node / run CLI), link to docs and feedback channel. |

### 6.3 Incentivized testnet launch

When moving from beta to an **incentivized testnet** (rewarding validators, developers, and users for a fixed period, e.g. 2–4 weeks), use the dedicated checklist and design in **[INCENTIVIZED-TESTNET-READINESS.md](INCENTIVIZED-TESTNET-READINESS.md)**. It covers: technical readiness (bootnodes, public RPC, faucet), incentive design (validators, developers, users), duration recommendation, pre-launch checklist, launch day, and success metrics. Complete the items there before announcing the incentivized testnet start date.

### 6.4 After launch

- Monitor the feedback channel and GitHub issues.
- Prioritize: crashes, RPC errors, and “I couldn’t get started” reports.
- Iterate on [RUNBOOK.md](RUNBOOK.md) and [BETA-READINESS.md](BETA-READINESS.md) from common questions.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
