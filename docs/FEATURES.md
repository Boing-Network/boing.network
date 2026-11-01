# Boing Network — Features timeline

> 👋 **Everyday users:** what works on public testnet today, what’s still gated, and what’s deferred.  
> 🛠️ **Developers / operators:** factual “when implemented” markers from **git tags**, **activation heights**, and **docs** — not guessed calendar months.

**Status vocabulary**

| Status | Meaning |
|--------|---------|
| **testnet live** | Usable on public chain **6913** at the snapshot tip (or crossed height). |
| **gated** | Code shipped; waiting on a **height** or **env** activation. |
| **deferred** | Explicitly out of scope / not wired / roadmap only. |

**Do not invent history.** Where evidence is thin, rows say **uncertain** or **approximate**. Prefer heights + release tags over month guesses.

Related: [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) (plain overview), [BOING-NETWORK-TECHNICAL-ESSENTIALS.md](BOING-NETWORK-TECHNICAL-ESSENTIALS.md) (technical companion), [BUILD-ROADMAP.md](BUILD-ROADMAP.md) (plan), [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) (backlog).

---

## Current testnet snapshot

*Probed `https://testnet-rpc.boing.network/` on **2026-11-01** (`boing_health` / `boing_getNetworkInfo`). Tip moves — re-check before claiming later gates crossed.*

| Fact | Value |
|------|--------|
| Public RPC | `https://testnet-rpc.boing.network/` (Cloudflare Worker → Fly **`boing-testnet-1` only**) |
| Chain | **6913** / Boing Testnet |
| Tip (probe) | **1522** |
| Bootnode | `/ip4/169.155.48.188/tcp/4001` ([FLY-IO.md](FLY-IO.md)) |
| `boing-testnet-2` | **Retired 2026-10-24** |
| Recommended SDK | **`@boing-network/boing-sdk@0.5.38`** (`client_hints`; tag `boing-sdk/v0.5.38` **2026-10-25**) |
| Recommended wallet | Boing Express **0.1.24** (`client_hints`; min advisory **0.1.18**) |
| Latest node tag | `testnet-v0.1.24` (**2026-11-01**) |
| DEX discovery | `boing_listDexPools`, `boing_listDexTokens`, `boing_getDexToken` |
| Canonical CP pool | Published in `end_user` (ops freeze **2026-05-21**; Fly advertise verified **2026-11-01**) — prefer live `end_user` |

### Activation heights (testnet 6913)

| Height | Feature | Status at tip 1522 |
|-------:|---------|--------------------|
| 400 | Open validator set (ops env) | testnet live when set |
| 640 | `.boing` name registry (ops env) | testnet live when set |
| 1000 | Height-1000 protocol rules | crossed (`testnet-v0.1.16+`) |
| 1050 | 18-decimal native BOING | crossed (`testnet-v0.1.17`) |
| 1150 | Native value on contract calls + related opcodes | crossed (`testnet-v0.1.18`) |
| 1188 | Micro fees (`execution_gas_fee_bps=100`) | crossed (`testnet-v0.1.19+` preset) |
| 1200 | Media blob Put/Announce admission | live |
| 1306 | PHASE7 **256 MiB** consensus cap | live |
| ~1317 | Phase 34 HTTP ACL private-by-default | **uncertain on probe** — docs cite 1317; confirm `boing_getMediaBlobParams` / ACL fields before assuming active |
| **1500** | Storage-tree v2 + account-tree v3 + VM hardening | **crossed** (2026-10-30; tags `testnet-v0.1.20` / `0.1.21`) |
| **1600** | Equivocation slash + strict JUMPI / NATIVE_TRANSFER reporting | **gated** (tip &lt; 1600) |
| **1700** | Canonical on-chain QA policy (+ public Unsure pool semantics in `0.1.24`) | **gated** |
| unset / never | Blake3 opcode `0x20` + fungible v3; governance consensus txs 37–39; BLS committee flip | **gated off** until env height set |

---

## 1. Protocol / VM

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| Sparse Merkle state; code/storage in account leaf | testnet live | [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md); NEXT-STEPS “Done” | high |
| Storage-tree v2 (256-bit key routing) | testnet live | H=**1500**; [STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md); `testnet-v0.1.21` **2026-10-30** | high |
| Account-tree v3 (256-bit account routing) | testnet live | H=**1500**; [ACCOUNT-TREE-V3-ACTIVATION.md](ACCOUNT-TREE-V3-ACTIVATION.md) | high |
| VM hardening (init-code QA, mem gas, stack 1024) | testnet live | H=**1500**; live `vm_hardening_activation_height` | high |
| Strict JUMPI + oversized NATIVE_TRANSFER reporting | gated | H=**1600**; live `vm_jump_strict_activation_height` | high |
| Native 18-decimal denomination | testnet live | H=**1050**; `testnet-v0.1.17` **2026-10-17**; [NATIVE-DENOMINATION.md](NATIVE-DENOMINATION.md) | high |
| Native value calls + opcodes `0x31`/`0x34`/`0x47`/`0xf2` | testnet live | H=**1150**; `testnet-v0.1.18`; [NATIVE-VALUE-AND-WBOING.md](NATIVE-VALUE-AND-WBOING.md) | high |
| wBOING + custody pools | testnet live | with native value era | high |
| Micro fee market + optional priority tips | testnet live | H=**1188**; [OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md) | high |
| Blake3 VM opcode `0x20` + fungible template v3 | gated | env `BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT` unset = never; PR #157 **2026-10-31** | high |
| Pool v11 first-mint √(A·B) | shipped | new CREATE2 only; no height gate; PR #154 | high |
| L1 media blobs Phases 1–33 | testnet live | admission H=**1200**; [BOING-L1-MEDIA-BLOBS.md](BOING-L1-MEDIA-BLOBS.md) | high |
| PHASE7 256 MiB consensus cap | testnet live | H=**1306** | high |
| Phase 34 Grant/Revoke blob download ACL | code live; private-by-default **uncertain** | SDK `boing-sdk/v0.5.38` **2026-10-25**; [HANDOFF_BLOB_DOWNLOAD_ACL.md](HANDOFF_BLOB_DOWNLOAD_ACL.md); verify ACL activation on RPC | medium |
| Phase 34.1 HTTP capability tickets | deferred | HANDOFF ACL “Phase 34.1” | high |
| ReservedRealV1 DAS verify (IPA/KZG/SNARK) | deferred / NotWired | media Phase 33 notes | high |
| BLS committee flip (tx 32→33 multi-pin) | gated | env unset | high |
| Name registry `.boing` | testnet live | H=**640** when ops set env | high |
| Height-1000 protocol rules | crossed | `testnet-v0.1.16+` | high |

---

## 2. Consensus / governance

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| HotStuff multi-validator MVP | testnet live (ops often single-validator) | NEXT-STEPS; RUNBOOK multi-validator section | high |
| Stake-derived validator epochs + unbonding | opt-in done | RUNBOOK §8.1; NEXT-STEPS | high |
| Open validator registration | testnet live | H=**400** when set; [TESTNET-VALIDATOR-GUIDE.md](TESTNET-VALIDATOR-GUIDE.md) | high |
| VRF leader election (ECVRF + gossip) | opt-in done | NEXT-STEPS | high |
| Equivocation + liveness slashing (+ appeals) | code live; **ReportEquivocation consensus slash gated** | H=**1600** for slash activation; ENVIRONMENT-VARIABLES | high |
| Operator governance RPC (propose / vote / cooling / execute) | testnet live | node-local `governance.json`; RPC-API-SPEC | high |
| Stake-weighted quorum (`strict_quorum`) | shipped | PRs #153 / #149; [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md) | high |
| Governance consensus txs 37–39 + StateStore registry | gated | wire shipped; admission off until height set; PR #156 **2026-10-31** | high |
| AdvanceExecution / Execute as consensus wire; BLS ballots | deferred | GOVERNANCE “Out of scope” | high |
| Mainnet governance activation | deferred | BUILD-ROADMAP | high |

---

## 3. QA

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| Protocol QA Allow / Reject / Unsure + `boing_qaCheck` | testnet live | [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md), [QA-GATE-RULES.md](QA-GATE-RULES.md) | high |
| Live registry lists on public RPC | often empty / size cap only until policy applied | re-query `boing_getQaRegistry`; intended EN blocklist via apply script | medium (drifts) |
| Canonical on-chain QA policy | gated | H=**1700**; tags `testnet-v0.1.23` / `0.1.24` | high |
| Public QA pool for Unsure (review instead of reject) | gated | from **1700** with `testnet-v0.1.24` semantics | high |
| CREATE2 uses registry `max_bytecode_size` from canonical height | gated | with QA canonical gate | high |
| Governance of QA rule lists via time-lock | partial / roadmap | BUILD-ROADMAP Phase QA | medium |

---

## 4. RPC

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| Core JSON-RPC + method catalog (doc-enforced) | testnet live | [RPC-API-SPEC.md](RPC-API-SPEC.md); `cargo test -p boing-node` coverage | high |
| `boing_getNetworkInfo` / `boing_health` activation fields | testnet live | live probe 2026-11-01 | high |
| `client_hints` recommended SDK / wallet | testnet live | live 0.5.38 / 0.1.24 | high |
| DEX discovery triple | testnet live | tag `testnet-v0.1.9` **2026-04-13** | high |
| `boing_getTokenMetadata` / `boing_listTokens` | testnet live | commits ~**2026-10-28** | high |
| Media blob RPC suite + `boing_getBlobAccess` | testnet live (with media activation) | RPC-API-SPEC; Phase 34 | high |
| Operator-token fail-closed for operator methods | shipped | ~2026-10-24+ security/ops commits | high |
| Public gateway 6 MiB JSON caps; slim blocks `include_code: false` | ops live | gateway + explorer practice | high |
| Transaction simulation tip+1 rules | live | `testnet-v0.1.18` era; RPC-API-SPEC | high |
| `/api/networks` public listing | website live | [NETWORKS-API.md](NETWORKS-API.md) | high |

---

## 5. Wallets (Express)

*Express has no git tags in this workspace; versions from `extension/manifest.json` + commits.*

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| Boing Express **0.1.24** (clearer approve/swap intent) | testnet live / recommended | manifest + `client_hints`; docs PR #33 **2026-11-01** | high |
| PutBlob publish sessions (`boing_requestPutBlobSession`) | Express-side UX | not node RPC; Express docs ~0.1.22 | high |
| Chain-bound v2 signing + 18-decimal awareness | from **0.1.18** (min_wallet) | live `client_hints`; Express **2026-10-17** | high |
| Native value call UX | **0.1.19** | Express **2026-10-17** | high |
| Priority tip UX | **0.1.21** | Express **2026-10-21** | high |
| `.boing` names in popup | **0.1.13** | Express **2026-10-16** | high |
| BIP-39 / SLIP-0010 recovery | shipped | pre-0.1.24 history | medium (first version approximate) |
| Incentivized testnet UX | not running | Express 0.1.23 wording; BUILD-ROADMAP | high |

---

## 6. DEX / Finance

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| Native CP AMM (swap / add / remove, 30 bps, Log2) | testnet live | NEXT-STEPS NAMM; [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) | high |
| Adjustable-fee / fee-on-input pool versions | shipped in VM templates | NEXT-STEPS | high |
| Native DEX factory (`register_pair`, Log3) | testnet live (when deployed/published) | [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md); live `end_user` factory | high |
| Ledger / swap2 / multihop routers | in-repo + testnet when published | NATIVE-DEX-* docs; live end_user ids | high |
| L1 discovery RPCs | testnet live | since `testnet-v0.1.9` **2026-04-13** | high |
| Native-dex-indexer Worker + D1 directory | in-repo worker | [HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md](HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md) | high |
| Canonical testnet pool publish | testnet live | ops **2026-05-21**; Fly `end_user` verified **2026-11-01** | high |
| Finance Swap → default CP; Pools + Smart route | product live pattern | finance `native-dex.md` | high |
| SDK `@boing-network/boing-sdk@^0.5.38` | published | tag **2026-10-25**; CHANGELOG | high |
| Planet-scale DEX subgraph / OBS-1 history | deferred / partial | NEXT-STEPS indexer | high |

Capability overview: [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md). Ops publish hub: [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md).

---

## 7. Observer

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| Explorer on public testnet RPC | live | default `https://testnet-rpc.boing.network/` | high |
| Same-origin `/api/rpc` proxy + 6 MiB caps | live | observer ops docs | high |
| Phase 34 blob download via `/api/blob/download` | shipped | wallet proof primary; HANDOFF ACL | high |
| Token naming from node + LP vault decode | shipped | observer commits **2026-10-28** | high |
| Readable tx summaries (approve/swap/transfer/LP) | shipped | **2026-10-28** | high |
| Storage-tree v2 status vs head on home | shipped | **2026-10-26** | high |
| Hosted durable observer (OBS-1) at product scale | deferred / partial | [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md); NEXT-STEPS | high |
| Universal contract deploy indexer worker | in-repo stepping stone | HANDOFF_Universal_Contract_Deploy_Indexer | medium |

---

## 8. Ops / infra

| Feature | Status | When / evidence | Confidence |
|---------|--------|-----------------|------------|
| Fly `boing-testnet-1` hosted node | live | [FLY-IO.md](FLY-IO.md) | high |
| `boing-testnet-2` retirement | done | **2026-10-24** | high |
| Public RPC gateway Worker | live | FLY-IO; edge policy enforce since **2026-10-28** | high |
| Fresh-node genesis→tip sync presets | shipped | `testnet-v0.1.19` **2026-10-28** (micro 1188, blobs 1200, PHASE7 1306, …) | high |
| P2P signed-tx gossip + per-IP caps | done | NEXT-STEPS; RUNBOOK | high |
| Desktop Network Hub | latest tag `desktop-hub/v0.1.27` **2026-04-07** | tags; Authenticode still docs/readiness | high |
| DHT / WebRTC / incentivized relays | deferred | BUILD-ROADMAP unchecked | high |
| Light clients / bridges / cross-chain | deferred | BUILD-ROADMAP | high |
| Mainnet launch | deferred | BUILD-ROADMAP | high |
| Incentivized testnet program | not currently running | TESTNET Part 3 planning only | high |

Operator hub: [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md). Day-to-day: [RUNBOOK.md](RUNBOOK.md).

---

## Release tags (selected)

| Date | Tag | Notable (from tag message / docs) |
|------|-----|-----------------------------------|
| 2026-11-01 | `testnet-v0.1.24` | Public QA pool semantics from block **1700** |
| 2026-10-31 | `testnet-v0.1.23` | Canonical on-chain QA policy from **1700** |
| 2026-10-31 | `testnet-v0.1.22` | Jump-strict / equivocation gate era (**1600**) |
| 2026-10-30 | `testnet-v0.1.21` / `0.1.20` | Storage/account/VM hardening at **1500** |
| 2026-10-28 | `testnet-v0.1.19` | Fresh-node sync; micro **1188**, blobs **1200**, PHASE7 **1306** |
| 2026-10-25 | `boing-sdk/v0.5.38` | Phase 34 blob ACL |
| 2026-10-17 | `testnet-v0.1.18` … `0.1.16` | Native value **1150**, 18-dec **1050**, height-1000 rules |
| 2026-04-13 | `testnet-v0.1.9` | DEX discovery RPC |
| 2026-03-26 | `testnet-v0.1.0` | Early public testnet node line |

Full tag list: `git tag --list 'testnet-*' 'boing-sdk/*' 'desktop-hub/*'`.

---

## Explicitly not yet / deferred

- Height **1600** / **1700** gates (tip 1522 on 2026-11-01 probe).
- Env-gated forever until set: Blake3 opcode, governance consensus txs 37–39, BLS committee flip, mainnet tree activations.
- Phase 34.1 capability tickets; real ReservedRealV1 DAS verification; practical non-inline multi-hundred-MiB Put plane.
- OBS-1 durable explorer at planet scale; DHT/WebRTC; bridges; light clients; mainnet.
- Incentivized testnet rewards program (not running).
- Many vision items in [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) / BUILD-ROADMAP unchecked rows — **not** claimed shipped here.

---

*Boing Network — Quality, enforced by the protocol.*
*Last factual refresh: 2026-11-01.*
