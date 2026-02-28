# Six Pillars — Readiness Assessment

> **Purpose:** Ensure all six pillars are optimally implemented **before** running the dual-node testnet infrastructure (primary + secondary computers).  
> **References:** [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md), [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md), [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md).

---

## Pillar 1: Security

**Definition:** Safety and correctness over speed.

| Area | Status | Implementation | Gap / Next Step |
|------|--------|----------------|-----------------|
| Consensus (BFT) | ✓ | HotStuff, 2f+1 quorum | — |
| Cryptography | ✓ | Ed25519, BLAKE3 | — |
| RPC rate limiting | ✓ | `RateLimitConfig`, 100 req/s mainnet | — |
| Disk persistence | ✓ | `--data-dir` | — |
| Equivocation detection | ✓ | Consensus slashing | — |
| Incident response | ✓ | RUNBOOK §6 | — |
| connections_per_ip | ☐ | Defined in config; not enforced | Add per-IP connection limit middleware |
| pending_txs_per_sender | ☐ | Defined in config; not enforced | Enforce in mempool insert |
| Bug bounty / audit contacts | ☐ | SECURITY-STANDARDS references | Add concrete security@, GitHub Advisories link |

**Pre-infra optimizations:** Enforce `connections_per_ip` (if RPC exposes client IP), `pending_txs_per_sender` in mempool, and document security contacts.

---

## Pillar 2: Scalability

**Definition:** High throughput without compromising other pillars.

| Area | Status | Implementation | Gap / Next Step |
|------|--------|----------------|-----------------|
| Parallel transfers | ✓ | Access-list batching, rayon | — |
| Transaction scheduler | ✓ | Conflict-free batches | — |
| Gas metering | ✓ | Per-tx-type costs | — |
| State store | ✓ | HashMap + Merkle | — |
| Dynamic gas | ☐ | Roadmap | Post-launch |
| State rent | ☐ | Roadmap | Post-launch |
| Throughput documentation | ☐ | — | Document expected TPS, block time |

**Pre-infra optimizations:** Add a short "Scalability characteristics" section to RUNBOOK or BUILD-ROADMAP (block time, typical TPS, batching behavior).

---

## Pillar 3: Decentralization

**Definition:** Permissionless participation at every layer.

| Area | Status | Implementation | Gap / Next Step |
|------|--------|----------------|-----------------|
| Permissionless node | ✓ | No whitelist; anyone can run | — |
| P2P (libp2p) | ✓ | TCP, Noise, gossipsub | — |
| Bootnode discovery | ✓ | `--bootnodes` | — |
| DHT discovery | ☐ | Roadmap | Requires live net |
| Peer scoring | ☐ | Roadmap | Requires live net |
| Bootnode rotation | ☐ | Roadmap | Governance / ops |

**Pre-infra optimizations:** Document decentralization design in RUNBOOK (who can validate, no central gatekeeper). No code changes needed before dual-node setup.

---

## Pillar 4: Authenticity

**Definition:** Unique architecture and identity (not a fork or framework).

| Area | Status | Implementation | Gap / Next Step |
|------|--------|----------------|-----------------|
| Custom VM | ✓ | Stack-based, Boing opcodes | — |
| Custom consensus | ✓ | HotStuff BFT | — |
| Own primitives | ✓ | BLAKE3, Ed25519, AccountId | — |
| No chain dependency | ✓ | Independent L1 | — |
| Docs / branding | ✓ | BOING-NETWORK-ESSENTIALS, design system | — |

**Pre-infra optimizations:** None required. Authenticity is established by architecture and docs.

---

## Pillar 5: Transparency

**Definition:** 100% openness in design, governance, and operations.

| Area | Status | Implementation | Gap / Next Step |
|------|--------|----------------|-----------------|
| Open source | ✓ | Public repo | — |
| Public specs | ✓ | RPC-API-SPEC, RUNBOOK, etc. | — |
| Account proof APIs | ✓ | `boing_getAccountProof`, `boing_verifyAccountProof` | — |
| Human-readable signing | ✓ | `display_for_signing` | — |
| QA rejection details | ✓ | `rule_id`, `message` in RPC | — |
| Public QA metrics | ☐ | QUALITY-ASSURANCE-NETWORK §16.2 | Add `boing_qaMetrics` RPC (optional) |
| Canonical malice definition | ☐ | QUALITY-ASSURANCE-NETWORK §16.3 | Add doc for pool/community |
| Deployer checklist | ☐ | QUALITY-ASSURANCE-NETWORK §16.1 | Add "How to pass QA" one-pager |

**Pre-infra optimizations:** Add canonical malice definition doc, deployer checklist (QA-PASS-GUIDE.md), optional boing_qaMetrics.

---

## Pillar 6: True Quality Assurance

**Definition:** Protocol-enforced QA; only assets meeting rules and security bar allowed; meme leniency; no malice; pool for genuine edge cases.

| Area | Status | Implementation | Gap / Next Step |
|------|--------|----------------|-----------------|
| Opcode whitelist | ✓ | boing-qa | — |
| Well-formedness | ✓ | boing-qa | — |
| Blocklist | ✓ | RuleRegistry | — |
| Scam patterns | ✓ | RuleRegistry | — |
| Purpose declaration | ✓ | ContractDeployWithPurpose | — |
| Always-review categories | ✓ | RuleRegistry | — |
| Soft rules | ✓ | check_soft_rules | — |
| Community pool | ✓ | boing_qa::pool | On-chain integration pending |
| Execution defense in depth | ✓ | vm.rs | — |
| boing_qaCheck RPC | ✓ | Full check | — |
| Doc links in rejection | ☐ | QUALITY-ASSURANCE-NETWORK §16.1 | Add optional doc URL to QaReject |

**Pre-infra optimizations:** Deployer checklist, canonical malice doc, optional doc-link in reject payload.

---

## Summary: Pre-Infrastructure Checklist

Complete these **before** running bootnodes on primary + secondary:

| # | Item | Pillar | Status |
|---|------|--------|--------|
| 1 | Enforce `pending_txs_per_sender` in mempool | Security | ✓ Done |
| 2 | Document security contacts (GitHub Advisories) in SECURITY-STANDARDS | Security | ✓ Done |
| 3 | Add "Scalability characteristics" (block time, TPS) to RUNBOOK | Scalability | ✓ Done |
| 4 | Add "Decentralization design" note to RUNBOOK | Decentralization | ✓ Done |
| 5 | Create [QA-PASS-GUIDE.md](QA-PASS-GUIDE.md) (deployer checklist) | QA, Transparency | ✓ Done |
| 6 | Create [CANONICAL-MALICE-DEFINITION.md](CANONICAL-MALICE-DEFINITION.md) | QA, Transparency | ✓ Done |
| 7 | Add optional doc URL to QaReject for structured feedback | QA | ✓ Done |
| 8 | (Optional) Add boing_qaMetrics RPC for public QA stats | Transparency, QA | Deferred |

**Defer to post-infrastructure (requires live net):**
- connections_per_ip enforcement (needs client IP from transport)
- DHT discovery, peer scoring
- Actual QA metrics accumulation (needs live deploys)
- Governance of QA rules (on-chain)

---

## Execution Order

1. **Security:** mempool cap, security contacts
2. **Docs:** RUNBOOK scalability + decentralization notes
3. **QA / Transparency:** deployer guide, malice definition, optional QaReject doc link
4. **Optional:** boing_qaMetrics stub (returns zeros until metrics exist)

After these are complete, proceed with [docs/INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) for the dual-node testnet.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
