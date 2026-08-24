# Boing Network — Cost Estimate for Running

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


> **Short answer:** Yes, running a blockchain network costs money. Costs scale with decentralization (more validators/nodes = more infrastructure).

---

## Cost Categories

### 1. Infrastructure (Servers / Cloud)

| Component | Role | Typical Specs | Est. Monthly Cost |
|-----------|------|---------------|-------------------|
| **Validator node** | Produces blocks, runs consensus | 8+ vCPU, 32GB RAM, 1TB SSD | $150–400 |
| **Full node** | Stores chain, serves RPC | 4 vCPU, 16GB RAM, 500GB–2TB SSD | $50–150 |
| **RPC node** (optional) | Public API for dApps | 4–8 vCPU, 16–32GB RAM | $80–200 |
| **Seed / bootstrap** | Peer discovery | 2 vCPU, 8GB RAM | $30–60 |

**Per-validator estimate:** ~$150–400/month  
**Bootstrap + 1 validator:** ~$200–450/month  
**Small testnet (4 validators + 2 full nodes):** ~$800–1,800/month

---

### 2. Bandwidth

- Validators and full nodes exchange blocks and transactions.
- Typical usage: 1–10 TB/month per validator for a moderate chain.
- Many cloud plans include 1–5 TB; extra bandwidth can add $50–200/month.

---

### 3. Development & Operations

| Item | Notes |
|------|-------|
| **Security audits** | Recommended before mainnet; $50k–200k+ |
| **Monitoring** | Prometheus, Grafana, alerts — $20–100/month or self-hosted |
| **Backups** | Snapshot storage — $20–100/month |
| **Team** | Dev/ops time (internal or contracted) |

---

### 4. Token Launch (If Applicable)

- **Fair launch:** No direct cost, but time and coordination.
- **Token deployment:** Gas fees on an existing chain if you bootstrap there.
- **Liquidity:** If you list BOING on DEXs, liquidity provisioning has capital and opportunity cost.

---

## Phased Cost Overview

| Phase | What You Run | Est. Monthly Cost |
|-------|--------------|-------------------|
| **Local dev** | 1–4 nodes on your machine | $0 (electricity only) |
| **Private testnet** | 2–4 VMs (e.g. AWS/GCP) | $200–600 |
| **Public testnet** | 4–10 validators + RPC | $800–2,500 |
| **Mainnet (minimal)** | 7+ validators, geo-distributed | $1,500–5,000+ |
| **Mainnet (decentralized)** | 50+ validators, many full nodes | Community-funded (validators pay their own infra) |

---

## How Decentralization Changes the Model

**Early stage (you run everything):**  
- You pay for all validators and infra.
- Centralized but controllable.

**Mature stage (permissionless validators):**  
- Validators run and pay for their own nodes.
- You mainly run foundation/ecosystem nodes (RPC, explorers, etc.).
- Your direct infra cost drops to $200–1,000/month for ecosystem services.

---

## Ways to Reduce Costs

1. **Start local** — Run testnet on your laptop or homelab.
2. **Spot / preemptible instances** — 50–70% cheaper for non-critical testnets.
3. **Smaller regions** — Cheaper than US-East/West in many clouds.
4. **Self-hosted hardware** — One-time purchase, ongoing power and internet.
5. **Community validators** — As you decentralize, others share infra cost.

---

## Rough Total Ranges

| Scenario | Monthly | One-time |
|----------|---------|----------|
| Development only | $0–50 | $0 |
| Small testnet (you operate) | $200–600 | $0 |
| Public testnet | $800–2,500 | $0–10k (audits, tooling) |
| Mainnet launch (minimal) | $1,500–5,000 | $50k–200k+ (audits) |
| Mainnet (community validators) | $200–1,000 (ecosystem only) | — |

---

## Economic Parameters (Governance-Adjustable)

| Parameter | Description | Example |
|-----------|-------------|---------|
| **Value cap per dApp** | Max incentive per dApp owner per epoch/month | 10M BOING/month |
| **Success metrics** | Tx count, fees, volume, unique users, TVL | `f(metrics)` formula |
| **Developer treasury** | Allocation for grants, audits, ecosystem | Governance-defined % |

See [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) for detailed dApp incentive design.

---

*Estimates are illustrative and depend on provider, region, and usage. Infrastructure and audit costs vary widely.*
