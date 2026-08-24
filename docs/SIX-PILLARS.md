# The Six Pillars of Boing Network

> *Authentic. Decentralized. Optimal. Quality-Assured.*

This is the written form of the six pillars. Sites such as [boing.observer/about](https://boing.observer/about) and [boing.network/about](https://boing.network/about) publish this document as a **PDF** rather than duplicating the prose as HTML. For the broader essentials (tech stack, crates, key docs), see [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md).

When trade-offs arise, the network applies this order:

**Security** → **Scalability** → **Decentralization** → **Authenticity** → **Transparency** → **True quality assurance**

---

## 1. Security

Safety and correctness over speed.

Boing uses HotStuff BFT consensus, Ed25519 signatures, and BLAKE3 hashing. Public RPC is rate-limited. Equivocation is detected and slashable. Security advisories and incident response follow [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md).

---

## 2. Scalability

High throughput without compromising the other pillars.

Transfers that do not conflict can execute in parallel. Access-list batching feeds that scheduler. Gas is metered per opcode and transaction type. Target block time is about two seconds.

---

## 3. Decentralization

Permissionless participation. No validator whitelist: anyone with stake can validate.

Networking is libp2p. Hosted testnet bootnodes:

- `/ip4/169.155.48.188/tcp/4001`
- `/ip4/109.105.220.118/tcp/4001`

There is no central gatekeeper for consensus, governance, or protocol QA. Unsure deployments go to the community QA pool, not a single operator.

---

## 4. Authenticity

Unique architecture and identity.

Boing is an independent L1: a custom stack-based VM with Boing opcodes, HotStuff BFT, BLAKE3 + Ed25519. It is not a fork or a framework wrapped around another chain.

---

## 5. Transparency

100% openness.

The protocol is open source, with public specs and account proof APIs. Signing payloads are human-readable. QA rejections include a `rule_id` and `message` so deployers can act on them. The explorer publishes a live [QA transparency](https://boing.observer/qa) dashboard (pool queue and governance parameters from public RPC).

---

## 6. True quality assurance

Protocol-enforced QA: only assets that meet the rules and security bar are allowed on-chain.

Every deployment is classified as **allow**, **reject**, or **unsure**. Unsure cases go to the **community QA pool** for review. Checks include the opcode whitelist, well-formedness, bytecode blocklist, deploy metadata content policy, scam patterns, and purpose declaration.

Meme, community, and entertainment purposes are valid. Malice is not. See [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) for policy and the canonical malice definition, live pool status on [QA transparency](https://boing.observer/qa), and machine-readable methods in [RPC-API-SPEC.md](RPC-API-SPEC.md).

---

## PDF on the websites

Regenerate site PDFs from the repo:

```bash
cd website && npm run build:pdfs
```

That writes `website/public/pdfs/SIX-PILLARS.pdf` (and the other docs listed in `website/scripts/generate-pdfs.js`). Explorers and the marketing site should **embed or link that PDF** for this write-up instead of maintaining a second HTML copy.
