# Boing Network — Decentralization Strategy & WebRTC Signaling

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


> **Purpose:** Deepen decentralization — P2P robustness, peer discovery, randomness, light clients, cross-chain trustlessness, and decentralized WebRTC signaling for browser light clients.  
> **References:** [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md), [BUILD-ROADMAP.md](BUILD-ROADMAP.md), [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md)

---

## Table of Contents

### Part 1: Decentralization Strategy
1. [P2P Network Robustness & Discovery](#1-p2p-network-robustness--discovery)
2. [Advanced Decentralized Peer Discovery Strategies](#2-advanced-decentralized-peer-discovery-strategies)
3. [Randomness & Validator Selection](#3-randomness--validator-selection)
4. [Light Client Accessibility](#4-light-client-accessibility)
5. [Cross-Chain Interoperability Decentralization](#5-cross-chain-interoperability-decentralization)
6. [Network Topology Monitoring](#6-network-topology-monitoring)

### Part 2: Decentralized WebRTC Signaling
7. [WebRTC Overview](#7-webrtc-overview)
8. [Boing Mainnet as Signaling Channel](#8-boing-mainnet-as-signaling-channel)
9. [Preventing Offer/Answer Spam](#9-preventing-offeranswer-spam)
10. [Decentralized Storage for Large Payloads](#10-decentralized-storage-for-large-payloads)
11. [DHT-Enhanced Peer Discovery for WebRTC](#11-dht-enhanced-peer-discovery-for-webrtc)
12. [Incentivized STUN/TURN Servers](#12-incentivized-stunturn-servers)
13. [STUN/TURN Reputation System](#13-stunturn-reputation-system)
14. [End-to-End Signaling Flow](#14-end-to-end-signaling-flow)

---

# Part 1: Decentralization Strategy

## 1. P2P Network Robustness & Discovery

**Goal:** Nodes find each other and maintain connections without relying on any central authority or a small, easily compromised set of initial peers.

### Current State

- `libp2p` provides a strong foundation for P2P networking (TCP, noise, yamux).
- mDNS and bootstrap lists are typical discovery mechanisms.
- **Gossipsub** topics: `boing/blocks` (blocks) and `boing/transactions` (**`SignedTransaction`**, bincode). Peers verify signatures before mempool insert; see [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) §12.3 and [RUNBOOK.md](RUNBOOK.md) §8.1.
- **Mesh:** Default gossipsub mesh targets assume **multiple** peers; two-node LAN tests may not propagate topic traffic reliably without tuning — see `cargo test -p boing-node --test p2p_tx_gossip_rpc` (four-node mesh) and `boing-p2p` gossip config.
- **Per-IP connection cap:** `boing-node` can limit simultaneous connections from the same remote IPv4/IPv6 address (`--max-connections-per-ip`, rate-limit profile defaults) to reduce Sybil-style fan-in.
- **Risk:** Initial peer discovery that depends on a small set of bootnodes creates a single point of failure and censorship vector.

### Target Architecture

**Dynamic, reputation-enhanced Kademlia DHT with a gossip-first overlay** — minimizing reliance on fixed bootnodes while maximizing network resilience.

---

## 2. Advanced Decentralized Peer Discovery Strategies

### 2.1 Enhanced DHT-based Discovery (Kademlia)

| Aspect | Description |
|--------|-------------|
| **Mechanism** | `libp2p` uses a Kademlia-inspired DHT. Each node maintains a routing table; peers query the network to find other peers by content/peer ID. |
| **Bootnode Decentralization** | Instead of a hardcoded list, use **bootnode rotation** driven by on-chain governance or a rotating set of well-established, community-funded nodes. Prevents any single bootnode from becoming a choke point. |
| **Sybil Attack Resistance** | Implement reputation systems or proof-of-stake mechanisms within the DHT layer. Make it harder for attackers to flood the network with malicious nodes and control routing tables. |
| **Eclipse Attack Mitigation** | Nodes actively diversify connections and periodically re-verify peer lists to resist isolation by an attacker. |

### 2.2 Gossip-first / Epidemic Protocols

| Aspect | Description |
|--------|-------------|
| **Mechanism** | Nodes primarily discover new peers through existing connections. When connecting, they exchange known-peer information; this "gossips" epidemically through the network. |
| **Random Peer Selection** | Regularly select a random subset of known peers to exchange peer lists or initiate new connections. Explores topology efficiently and resists partition attempts. |
| **Active Probing** | Periodically "ping" known and recently discovered peers to check liveness and update topology; shed inactive or unresponsive peers. |
| **Peer Scoring/Reputation** | Local scoring: prioritize peers that have historically provided reliable, low-latency connections. Contributes to network health. |

### 2.3 WebRTC / WebSockets for Browser-based Light Clients

| Aspect | Description |
|--------|-------------|
| **Rationale** | Browser-based light clients cannot use raw TCP. WebRTC enables direct P2P between browsers; WebSockets maintain persistent connections with full nodes or relayers. |
| **Decentralized Signaling** | The signaling process (browsers exchanging connection info) must be decentralized. See [Part 2: Decentralized WebRTC Signaling](#part-2-decentralized-webrtc-signaling) below for full design. |
| **NAT Traversal** | ICE (Interactive Connectivity Establishment), STUN/TURN servers. Community members or protocol-incentivized operators run these to allow nodes behind NATs to connect. |

### 2.4 Relayed Connections & Rendezvous Points

| Aspect | Description |
|--------|-------------|
| **Mechanism** | For nodes behind restrictive firewalls or NATs, relayed connections route traffic through an intermediary. Rendezvous points are where nodes announce presence and find others. |
| **Incentivized Relayers** | Reward nodes that act as relayers (similar to Filecoin/Arweave storage providers). Ensures a robust, distributed set of relayers. |
| **DHT for Rendezvous** | Use Kademlia DHT as decentralized rendezvous: nodes announce willingness to connect via public key/ID; others look up this information. |

### Peer Discovery Architecture (Conceptual)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Boing P2P Discovery Stack                         │
├─────────────────────────────────────────────────────────────────────┤
│  Gossip Layer      │ Exchange peer lists via existing connections    │
│  DHT Layer         │ Kademlia: lookup peers by ID; rendezvous        │
│  Reputation Layer  │ Peer scoring; Sybil/eclipse resistance          │
│  Bootnode Layer    │ Governance-rotated; fallback only               │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Randomness & Validator Selection

**Goal:** Unpredictable, unbiased leader selection resistant to manipulation.

| Approach | Description | Status |
|----------|-------------|--------|
| **VDF (Verifiable Delay Functions)** | Verifiable sequencing; contributes to fair ordering and resistance to validator collusion. | Design target |
| **VRF (Verifiable Random Functions)** | Cryptographically secure randomness for leader election. Unpredictable until revealed. | Design target |
| **Current** | Round-robin leader rotation in HotStuff. | Implemented |

**Integration:** Replace or augment round-robin with VDF/VRF-driven selection in Phase 4/5.

---

## 4. Light Client Accessibility

**Goal:** Broad, independent verification with minimal resources.

| Component | Description | Status |
|-----------|-------------|--------|
| **Verkle-based stateless clients** | Lighter nodes; compact proofs. | Verkle/merkle proofs implemented |
| **Browser-based verification** | Wasm light clients in web browsers; trustless dApp interaction without full nodes. | Roadmap item |
| **Mobile-friendly validation** | Compact proof specs; low bandwidth. | Roadmap item |

---

## 5. Cross-Chain Interoperability Decentralization

**Goal:** Trustless bridging and decentralized oracles.

| Component | Description |
|-----------|-------------|
| **Trustless bridges** | ZKP-based or MPC-based relayers with slashing; avoid multi-sig or federated bridges with few signers. |
| **Decentralized oracles** | External data (prices, chain finality) sourced and verified through truly decentralized oracle networks. No single point of failure. |

---

## 6. Network Topology Monitoring

**Goal:** Community visibility into decentralization health.

| Approach | Description |
|----------|-------------|
| **On-chain or off-chain dashboards** | Monitor node distribution (geographical, cloud provider, etc.). Transparency fosters decentralization. |
| **Metrics** | Unique peers, connection diversity, bootnode usage, relay usage. |

---

## Integration with Boing Principles

| Principle | How This Strategy Reinforces It |
|-----------|--------------------------------|
| **Absolute Decentralization** | Minimize reliance on fixed bootnodes and central components; peer discovery resilient to censorship. |
| **Security** | Robust DHT and gossip resist Sybil, eclipse, and partition attacks. |
| **Authenticity & Uniqueness** | Novel combination: reputation-enhanced DHT + gossip-first + incentivized relayers; on-chain signaling for WebRTC. |

---

# Part 2: Decentralized WebRTC Signaling

## 7. WebRTC Overview

WebRTC requires a **signaling** process to exchange network information (IP addresses, ports, SDPs) between peers before a direct P2P connection can be established. Centralized signaling servers are single points of failure and censorship.

**Goal:** Eliminate centralized signaling; leverage Boing mainnet, decentralized storage, DHT, and incentivized NAT traversal.

---

## 8. Boing Mainnet as Signaling Channel

### Offer/Answer Smart Contract

| Component | Description |
|-----------|-------------|
| **Contract** | Dedicated signaling contract on Boing for offer/answer exchange. |
| **Offer Flow** | Peer A encrypts WebRTC offer (SDP) with Peer B's public key; posts to contract or stores CID (see §10). |
| **Answer Flow** | Peer B fetches offer, decrypts, generates answer, encrypts, posts back to contract. |
| **Event Logging** | Contract emits events on offer/answer post; peers listen via Boing SDK. |

### Benefits

- **Native decentralization** — Inherits censorship resistance of Boing mainnet.
- **Verifiable history** — Signaling exchanges recorded on-chain.
- **Considerations** — Minimize tx cost and latency via efficient contract design; use off-chain storage for large SDPs.

---

## 9. Preventing Offer/Answer Spam

| Mechanism | Description |
|-----------|-------------|
| **Transaction Fees (Gas)** | Every contract interaction incurs gas. Adaptive gas model and predictable pricing make high-volume spam prohibitively expensive. |
| **On-Chain Rate Limiting** | Contract tracks `last_sent_timestamp` and `message_count` per address. E.g. max 5 offers per minute per address. |
| **Staking or Deposit** | Initiating an offer requires a small BOING deposit held by the contract. Returned on successful connection or after expiration; forfeited or partially returned if rejected/expired. |
| **Identity and Reputation** | Optional: require minimum reputation score or Soulbound credentials to send offers. |
| **Challenge-Response** | For sensitive offers: recipient can request a lightweight proof before processing. |

---

## 10. Decentralized Storage for Large Payloads

| Aspect | Description |
|--------|-------------|
| **Small Messages** | ICE candidates and small SDPs can go directly on-chain. |
| **Large SDPs** | Store encrypted SDPs on IPFS/Filecoin/Arweave. |
| **On-Chain Pointers** | Post only CID (Content Identifier) or hash + recipient public key to Boing contract. |
| **Retrieval** | Recipient fetches CID from chain, downloads from decentralized storage, decrypts. |

**Benefits:** Reduces on-chain load and costs; leverages specialized decentralized storage infrastructure.

---

## 11. DHT-Enhanced Peer Discovery for WebRTC

| Component | Description |
|-----------|-------------|
| **DHT Announcement** | Peers announce WebRTC capability and public key/Boing address in Kademlia DHT. |
| **Recipient Lookup** | Before initiating signaling, peer uses DHT to confirm recipient is online and fetch metadata. |
| **Integration** | Complements libp2p discovery in Part 1. |

**Benefits:** Resilient, dynamic discovery; no central directory.

---

## 12. Incentivized STUN/TURN Servers

| Aspect | Description |
|--------|-------------|
| **Role** | STUN (Session Traversal Utilities for NAT) and TURN (Traversal Using Relays around NAT) handle NAT traversal. |
| **Decentralization** | Community members run STUN/TURN nodes. |
| **Protocol Incentives** | Reward reliable STUN/TURN providers (similar to validators or storage providers). |
| **Distribution** | Ensures geographically distributed relay capacity. |

---

## 13. STUN/TURN Reputation System

| Component | Description |
|-----------|-------------|
| **On-Chain Registry** | Smart contract for STUN/TURN server registration. Requires minimum BOING stake. Stores network address and metadata. |
| **Performance Metrics** | Nodes/dApps monitor and report: **uptime**, **latency**, **success rate**, **bandwidth/throughput**. Data submitted to registry; optionally aggregated via decentralized oracle. |
| **Reputation Score** | Contract maintains dynamic score per server from aggregated metrics. Score decays over time to prioritize recent performance. |
| **Selection** | SDK allows dApps to query registry and select servers by reputation, geography, or other criteria. |
| **Slashing** | Servers that underperform, act maliciously, or fail minimum uptime face stake slashing. Proportional to severity and duration. |

---

## 14. End-to-End Signaling Flow

1. **Discovery:** Peers use DHT to find each other and confirm WebRTC readiness.
2. **Offer:** Peer A encrypts SDP with Peer B's public key; uploads to IPFS or posts on-chain; posts CID to Boing contract.
3. **Answer:** Peer B retrieves offer, decrypts, posts encrypted answer (or CID) to contract.
4. **ICE Candidates:** Both exchange ICE candidates via contract or low-latency gossip.
5. **Connection:** Direct WebRTC connection; use STUN/TURN for NAT traversal as needed.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
