# Boing Network — Launch-Blocking Checklist

> **Purpose:** Track the critical path to a successful incentivized testnet launch so that **VibeMiner** (one-click mining), **terminal validators**, and **boing.observer** (blockchain explorer) can use real data.  
> **References:** [INCENTIVIZED-TESTNET-READINESS.md](INCENTIVIZED-TESTNET-READINESS.md), [TESTNET.md](TESTNET.md), [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md), [BOING-OBSERVER-EXPLORER-PROMPT.md](BOING-OBSERVER-EXPLORER-PROMPT.md).

---

## Why VibeMiner Shows "No Nodes"

VibeMiner reports "currently no nodes on Boing Network" because:

1. **No bootnodes** — The config (`website/src/config/testnet.ts` and `PUBLIC_BOOTNODES`) has no bootnode multiaddrs. VibeMiner uses these to connect `boing-node` to the testnet.
2. **No public RPC** — The default RPC URL is `http://127.0.0.1:8545`. There is no live `https://testnet-rpc.boing.network/` (or equivalent) for faucet and health checks.
3. **No live infrastructure** — Bootnodes and a faucet node must be running and reachable before anyone can join the testnet.

Until these exist, VibeMiner, terminal validators, and boing.observer cannot operate against a shared public chain.

---

## Critical Path (in order)

Complete these in sequence. Each depends on the previous.

### 1. Run bootnodes (blocking)

| Step | Action | Owner | Done |
|------|--------|-------|------|
| 1.1 | Run at least **2** `boing-node` instances with stable public IPs, P2P enabled | Ops | ☐ |
| 1.2 | Open TCP **4001** (P2P) and optionally **8545** (RPC) on each host | Ops | ☐ |
| 1.3 | Record multiaddrs, e.g. `/ip4/1.2.3.4/tcp/4001` | — | ☐ |

**Scripts:** Use [docs/INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md). Primary: `scripts/start-bootnode-1.bat` (or `.sh`). Secondary: `scripts/start-bootnode-2.bat` (set `BOOTNODE_1_IP`).

**Commands** (from [RUNBOOK.md](RUNBOOK.md) §8.1):

```bash
./target/release/boing-node \
  --p2p_listen /ip4/0.0.0.0/tcp/4001 \
  --validator \
  --rpc-port 8545 \
  --data-dir ./bootnode-data
```

---

### 2. Run faucet + public RPC (blocking)

| Step | Action | Owner | Done |
|------|--------|-------|------|
| 2.1 | Run a `boing-node` with `--faucet-enable` and `--bootnodes` (from step 1) | Ops | ☐ |
| 2.2 | Expose it at a public URL (e.g. `https://testnet-rpc.boing.network/`) | Ops | ☐ |
| 2.3 | Confirm faucet works: `boing_faucetRequest` returns success | Ops | ☐ |

**Commands** (from [RUNBOOK.md](RUNBOOK.md) §8.2):

```bash
./target/release/boing-node \
  --p2p_listen /ip4/0.0.0.0/tcp/4001 \
  --bootnodes /ip4/<BOOTNODE_1>/tcp/4001,/ip4/<BOOTNODE_2>/tcp/4001 \
  --validator --faucet-enable --rpc-port 8545 --data-dir ./faucet-data
```

---

### 3. Update config and docs (blocking)

| Step | Action | Owner | Done |
|------|--------|-------|------|
| 3.1 | Set `PUBLIC_BOOTNODES` (comma-separated multiaddrs) in deploy env or `website/.env` | Ops/Dev | ☐ |
| 3.2 | Set `PUBLIC_TESTNET_RPC_URL` to the public faucet RPC | Ops/Dev | ☐ |
| 3.3 | Update [TESTNET.md](TESTNET.md) §6 bootnode table | Dev | ☐ |
| 3.4 | Rebuild and deploy website so testnet/faucet pages show live URLs | Dev | ☐ |

---

### 4. VibeMiner and boing.observer (unblocks)

| Consumer | Needs | After |
|----------|-------|-------|
| **VibeMiner** | Bootnodes from website/TESTNET.md + RPC for faucet/health | Steps 1–3 complete |
| **Terminal validators** | Same bootnodes + RPC; docs in TESTNET.md | Steps 1–3 complete |
| **boing.observer** | Public RPC URL only (`NEXT_PUBLIC_TESTNET_RPC` or similar) | Step 2.2 complete |

Once step 3 is done, VibeMiner can read bootnodes and RPC from the website or a config source. boing.observer needs the same public RPC URL to call `boing_chainHeight`, `boing_getBlockByHeight`, etc.

---

## boing.observer Requirements

The explorer ([BOING-OBSERVER-EXPLORER-PROMPT.md](BOING-OBSERVER-EXPLORER-PROMPT.md)) pulls data via JSON-RPC. It needs:

- **Testnet RPC URL** — Same as faucet/public RPC (e.g. `https://testnet-rpc.boing.network/`)
- **Mainnet RPC URL** — Can be "coming soon" until mainnet launch

Configure via env, e.g. `NEXT_PUBLIC_TESTNET_RPC`. No bootnodes required for the explorer (it only does HTTP RPC, not P2P).

---

## Verification

After steps 1–3:

1. **VibeMiner** — Start node via VibeMiner; it should connect to bootnodes and sync. Status (e.g. chain height) should come from the public RPC or local node.
2. **Faucet** — Visit [boing.network/network/faucet](https://boing.network/network/faucet); request succeeds against the public RPC.
3. **boing.observer** — With testnet RPC configured, home page shows latest block height and recent blocks.
4. **Terminal** — `boing-node --p2p_listen ... --bootnodes <LIST> --validator` syncs and produces blocks.

---

## Current Config (reference)

| Config | Location | Current value |
|--------|----------|---------------|
| `TESTNET_RPC_URL` | `website/src/config/testnet.ts` | `https://testnet-rpc.boing.network/` |
| `BOOTNODES` | `website/src/config/testnet.ts` | `/ip4/73.84.106.121/tcp/4001` (primary + secondary) |
| `isTestnetLive` | Derived | `true` when bootnodes + non-local RPC are set |

Override at build: `PUBLIC_TESTNET_RPC_URL`, `PUBLIC_BOOTNODES` (see `website/.env.example`).

---

## Summary

| Blocker | Resolution |
|---------|------------|
| VibeMiner "no nodes" | Run bootnodes, publish multiaddrs, set `PUBLIC_BOOTNODES` |
| Faucet unusable remotely | Run faucet node, expose as public RPC, set `PUBLIC_TESTNET_RPC_URL` |
| boing.observer no data | Use same public RPC URL in explorer config |
| Terminal validators can't join | Bootnodes in TESTNET.md and website |

Complete steps 1 → 2 → 3 in order. Once done, all consumers (VibeMiner, terminal, explorer) can use the live testnet. For the full incentive program and launch process, see [INCENTIVIZED-TESTNET-READINESS.md](INCENTIVIZED-TESTNET-READINESS.md).

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
