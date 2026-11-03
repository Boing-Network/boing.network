# Boing Network — Technical Essentials

A technical companion to the plain-language [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md). This page is for integrators, operators, and protocol readers who need accurate current facts without opening every deep spec.

**Status:** public **testnet** (chain **6913**) is live. **Mainnet (6914) is not live.** Tip and activation fields move — re-check live RPC before asserting a gate has crossed.

**Deeper sources of truth** (prefer these over copying prose from here):

| Topic | Canonical doc |
|-------|----------------|
| JSON-RPC methods, errors, probes | [RPC-API-SPEC.md](RPC-API-SPEC.md) |
| Crypto, VM ISA, gas, QA rules, crates | [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) |
| Shipped / gated / deferred timeline | [FEATURES.md](FEATURES.md) |
| Signed tx / block bincode | [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md) |
| Env vars | [ENVIRONMENT-VARIABLES.md](ENVIRONMENT-VARIABLES.md) |
| Static identity pointers | [chain-metadata.json](chain-metadata.json) |

---

## 1. Chain identity and architecture

| Fact | Value |
|------|--------|
| Public testnet chain id | **6913** (`0x1b01`) — *Boing Testnet* |
| Planned mainnet chain id | **6914** (`0x1b02`) — not live |
| Public RPC | `https://testnet-rpc.boing.network/` |
| Hosted node | Fly **`boing-testnet-1` only** (`boing-testnet-2` retired **2026-10-24**) |
| Bootnode | `/ip4/169.155.48.188/tcp/4001` — [FLY-IO.md](FLY-IO.md) |
| Language / node | Rust `boing-node` |
| Hashing | **BLAKE3** (32-byte digests) |
| Signatures | **Ed25519** (64-byte sig; 32-byte pubkey = `AccountId`) |
| Consensus | **PoS + HotStuff BFT** (libp2p: TCP, Noise, gossipsub, request-response) |
| Execution | **Boing VM** only — custom stack ISA; not EVM / not another chain’s bytecode |
| State | Sparse Merkle trees (account + per-contract storage); Verkle is a stated target |

Addresses are **32-byte** hex (`0x` + 64 hex). Wallets and dApps must not treat chain **6913** as an EVM chain.

Signing domains (see encoding spec): transaction **`BOING_TX_V2`**; priority tip **`BOING_TX_PRIORITY`**. Always decode with `SignedTransaction::from_bincode` / `Block::from_bincode` — zero tips and empty signature vectors are **omitted** on the wire.

---

## 2. Consensus and activation heights

Consensus is HotStuff-style BFT over a validator set. Public testnet day-to-day traffic often rides a **single hosted validator** path even though multi-validator MVP, stake-derived epochs, VRF leader election, and open registration exist in code/ops. Open validator registration activates when ops set the height (documented **400**). Equivocation reporting and related slash activation are gated (see table).

**Selected activation heights on testnet 6913** (from [FEATURES.md](FEATURES.md); tip was **1522** on a **2026-11-01** probe — re-verify):

| Height | Feature | Notes |
|-------:|---------|--------|
| 400 | Open validator set | When ops env is set |
| 640 | `.boing` name registry | When ops env is set |
| 1000 | Height-1000 protocol rules | Crossed |
| **1050** | 18-decimal native BOING | Crossed — [NATIVE-DENOMINATION.md](NATIVE-DENOMINATION.md) |
| **1150** | Native value on calls + opcodes `0x31` / `0x34` / `0x47` / `0xf2` | Crossed |
| **1188** | Micro fees (`execution_gas_fee_bps=100`) | Crossed |
| **1200** | Media blob Put/Announce admission | Live |
| **1306** | PHASE7 **256 MiB** blob consensus cap | Live |
| ~1317 | Phase 34 private-by-default ACL | Confirm on live `boing_getMediaBlobParams` / ACL fields |
| **1500** | Storage-tree v2 + account-tree v3 + VM hardening | **Crossed** |
| **1600** | Equivocation slash + strict JUMPI / NATIVE_TRANSFER reporting | **Gated** (tip &lt; 1600 at last FEATURES probe) |
| **1700** | Canonical on-chain QA policy + public Unsure pool semantics | **Gated** |
| unset | Blake3 opcode `0x20`; governance consensus txs 37–39; BLS committee flip | Off until env height set |

Discovery: `boing_getNetworkInfo` / `boing_health` publish activation fields and `client_hints`. Do not invent heights — read the node.

---

## 3. Account and storage trees

**Account leaf** (global SMT) commits:

```text
BLAKE3(balance_le || nonce_le || stake_le || code_hash || storage_root)
```

- `code_hash` = `BLAKE3(bytecode)` or zero; `storage_root` is the per-contract storage SMT (or zero).
- Parallel scheduling uses signed **access lists** (`read` / `write` `AccountId`s). Disjoint lists may run in the same batch; incomplete lists are unsafe.

**Storage-tree v2** (testnet **H = 1500**): per-contract storage routes on **all 256 key bits** (fixes 64-bit prefix collision). See [STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md).

**Account-tree v3** (same **H = 1500**): account routing uses full 256-bit ids. See [ACCOUNT-TREE-V3-ACTIVATION.md](ACCOUNT-TREE-V3-ACTIVATION.md).

Mainnet defaults remain unset until an announced height. Clients that recompute roots must switch routing when `height >=` the published activation fields.

---

## 4. VM and opcodes (highlights)

Boing VM is a deterministic stack machine (`boing-execution`). Full opcode table, gas, and well-formedness rules: [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) §7–8.

Notable families:

| Area | Examples |
|------|----------|
| Arithmetic / bitwise | `ADD`…`MULMOD`, `LT`/`GT`/`EQ`, shifts, `AND`/`OR`/`XOR` |
| Context | `ADDRESS`, `CALLER`, `BLOCKHEIGHT`, `TIMESTAMP` |
| Memory / storage | `MLOAD`/`MSTORE`, `SLOAD`/`SSTORE` |
| Control | `JUMP`/`JUMPI`, `PUSH*`, `RETURN`/`STOP` |
| Calls / create | `CALL` (`0xf1`), `CREATE2` (`0xf5`); optional deploy init marker `0xFD` |
| Logs | `LOG0`–`LOG4` |
| Native value (from **1150**) | `BALANCE` `0x31`, `CALLVALUE` `0x34`, `SELFBALANCE` `0x47`, `NATIVE_TRANSFER` `0xf2` |
| **Blake3** `0x20` | Memory-range hash; **gated** by `BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT` (unset = never; invalid before activation) |

Hardening at **1500** includes init-code QA, memory gas, stack depth **1024**. Strict JUMPI and oversized `NATIVE_TRANSFER` reporting activate at **1600**.

Bytecode size cap (QA): **32 KiB**. Contract call interpreter budget: **`GAS_PER_CONTRACT_CALL = 40_000_000`** (fee uses actual `gas_used`).

---

## 5. Native denomination and fees

- **1 BOING = 10¹⁸** base units (`u128` on ledger; **decimal strings** on RPC).
- Testnet **6913**: whole-BOING below height **1050**; base units from **1050** ([NATIVE-DENOMINATION.md](NATIVE-DENOMINATION.md)).
- Gas price constant: **10¹²** base units per gas (= **0.000001 BOING/gas**) before micro BPS.
- Fee market v1 ([OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md)): recommended **`execution_gas_fee_bps = 100`** → ≈ **0.00021 BOING** per 21k transfer (active from **1188** on public testnet). Optional `priority_tip` for ordering; tips default to the proposer.
- Blob / Forever pin fees are **independent** of execution-gas BPS.

`boing_getNetworkInfo.native_currency` and fee policy fields are authoritative for live formatting and BPS.

---

## 6. Quality gate and public pool

Protocol QA screens **deploy** bytecode (and related metadata) before it lands on the ledger: **Allow / Reject / Unsure**.

- Preflight: `boing_qaCheck`. Structured rejection codes for wallets/UIs.
- Rule catalog and opcode whitelist: [QA-GATE-RULES.md](QA-GATE-RULES.md); product narrative: [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md).
- Live registry lists on public RPC are often empty / size-capped until policy is applied — query `boing_getQaRegistry`.
- **Canonical on-chain QA policy** and **public Unsure review pool** semantics: gated at height **1700** (`testnet-v0.1.23` / `0.1.24` era). Until that height, do not claim the public pool is live on tip.

Governance of rule lists via time-lock remains partial / roadmap — see FEATURES and BUILD-ROADMAP.

---

## 7. RPC surface (highlights)

Authoritative method list: [RPC-API-SPEC.md](RPC-API-SPEC.md) (unit-tested against `BOING_RPC_SUPPORTED_METHODS`). JSON-RPC 2.0 over HTTP POST (default node port **8545**). Same listener: `GET /live`, `/ready`, `/openapi.json`, `/.well-known/boing-rpc`, `GET /ws` (newHeads).

Integrator-relevant groups:

| Group | Examples |
|-------|----------|
| Health / discovery | `boing_health`, `boing_getNetworkInfo` (`client_hints`, activation heights, `developer.dex_discovery_methods`, `end_user` native DEX ids) |
| Chain reads | `boing_getBlockByHeight` / `ByHash` (prefer `include_code: false` for slim stubs), account/balance, receipts |
| Submit / simulate | `boing_submitTransaction`, `boing_simulateTransaction`, `boing_simulateContractCall` |
| QA | `boing_qaCheck`, registry getters |
| DEX discovery | `boing_listDexPools`, `boing_listDexTokens`, `boing_getDexToken` |
| Tokens | `boing_getTokenMetadata`, `boing_listTokens` |
| Media blobs | Put/announce/ACL suite incl. `boing_getBlobAccess` — see media section |
| Operator | Fail-closed without `BOING_OPERATOR_RPC_TOKEN` (`-32057`) |

Public gateway and explorer proxies cap JSON bodies (≈ **6 MiB**) and reject truncated bodies. Browser apps need correct **CORS** (or same-origin proxy). Node listing for portals: [NETWORKS-API.md](NETWORKS-API.md) (`GET /api/networks`).

---

## 8. L1 media blobs

Phases **1–33** live; Put/Announce admission from height **1200**. **PHASE7 256 MiB** consensus cap active from **1306**. Phase **34** download ACL (Grant/Revoke) is shipped; private-by-default activation around **1317** should be confirmed on live params.

- Separate **blob fee path** — not account `SSTORE`.
- Block JSON: byte fields as **`0x` hex**; large vectors stubbed when `include_code: false`.
- PutBlob **publish sessions** (`boing_requestPutBlobSession`) are **Express-side**, not node RPC.
- Download: wallet proof (publisher / ACL grantee) primary; operator / token override paths documented in [HANDOFF_BLOB_DOWNLOAD_ACL.md](HANDOFF_BLOB_DOWNLOAD_ACL.md). Explorer streams via `/api/blob/download`.
- Deferred / not wired for product claims: Phase **34.1** capability tickets; real ReservedRealV1 DAS verification; practical multi-hundred-MiB non-inline Put plane.

Full protocol: [BOING-L1-MEDIA-BLOBS.md](BOING-L1-MEDIA-BLOBS.md).

---

## 9. Native DEX and CP pool discovery

Boing’s native constant-product AMM, factory, routers, and LP vaults are **Boing VM contracts** (selectors / CREATE2 salts in [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) and related NATIVE-DEX-* docs). Capability overview: [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md).

**Discovery layers** (merge for pickers/labels; prefer live RPC):

1. **L1 RPC** — `boing_listDexPools` / `boing_listDexTokens` / `boing_getDexToken`; method names also under `boing_getNetworkInfo.developer.dex_discovery_methods`.
2. **`end_user`** — published canonical factory / default CP pool ids (ops freeze documented; re-read live `end_user`).
3. **Indexer / directory Worker** — [HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md](HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md) (`workers/native-dex-indexer`, `/v1/directory/*`).
4. **Hydrated pool venue metadata** — Finance / SDK merge for UI labels.

Product orientation: Finance **Swap** targets the network default CP pool; additional factory pairs and multihop sit behind **Pools** / Smart route. Planet-scale subgraph / OBS-1 history remains deferred / partial.

SDK: **`@boing-network/boing-sdk@^0.5.38`** (npm; not unscoped `boing-sdk`). Tutorial bootstrap: `examples/native-boing-tutorial/scripts/` (operators run against live RPC; node does **not** auto-create pools on start).

---

## 10. Product surfaces (SDK / Express / Finance / Observer)

| Surface | Role | Technical notes |
|---------|------|-----------------|
| **@boing-network/boing-sdk** | TypeScript client | Recommended **0.5.38**; signing, RPC helpers, blob ACL APIs; rebuild `dist/` after local SDK edits |
| **Boing Express** | Wallet | Recommended **0.1.24** (`client_hints`; min advisory **0.1.18**); `window.boing`; PutBlob publish sessions; v2 signing + tips |
| **Boing Finance** | Swaps / pools / tokens / NFTs | Native VM `AccountId`s in `contracts.js` / env — not EVM addresses on 6913 |
| **Boing Observer** | Explorer | Public testnet RPC; same-origin `/api/rpc`; slim blocks; QA / activation visibility |
| **Website / faucet** | Docs, join, faucet | [boing.network](https://boing.network) |

Integration entry: [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md). Wallet product detail: [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md). Explorer + wallet split: [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md).

---

## 11. Operations (Fly and public RPC)

| Item | Detail |
|------|--------|
| Hosted validator/RPC | Fly app **`boing-testnet-1`** — [FLY-IO.md](FLY-IO.md) |
| Public hostname | Cloudflare Worker `workers/public-rpc-gateway` → Fly; **do not** point `testnet-rpc.boing.network` at a home tunnel |
| Chain env on public node | `BOING_CHAIN_ID=6913`, `BOING_CHAIN_NAME=Boing Testnet` — templates under `tools/boing-node-public-testnet.*` |
| Operator auth | Required for operator methods; **mainnet 6914** refuses start without token and refuses `--faucet-enable` |
| Day-2 ops | [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md), [RUNBOOK.md](RUNBOOK.md), [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) |
| WAF / edge | [PUBLIC-RPC-WAF-GUIDANCE.md](PUBLIC-RPC-WAF-GUIDANCE.md) |

Upgrading a personal laptop node does **not** change public explorer or gateway behavior.

---

## 12. Governance and deferred items

**Shipped / available (with caveats):** operator governance RPC (propose / vote / cooling / execute) against node-local state; stake-weighted quorum (`strict_quorum`); open validator registration when height env is set.

**Gated / deferred (do not claim live on tip without checking):**

- Heights **1600** / **1700** features (see §2).
- Env-gated until set: Blake3 opcode, governance consensus txs **37–39**, BLS media committee flip, mainnet tree activations.
- AdvanceExecution / Execute as consensus wire; BLS ballots — out of scope per governance activation docs.
- DHT / WebRTC / incentivized relays; light clients; bridges; mainnet launch.
- Incentivized testnet rewards program (not currently running). Framing: [BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md](BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md); draft tables: [BOING-INITIAL-TOKENOMICS.md](BOING-INITIAL-TOKENOMICS.md). Supply policy (**BN-155**): **1B BOING genesis**, **uncapped** afterward — not a mainnet claim from testnet activity.
- Durable planet-scale Observer (OBS-1) and full DEX subgraph history.

Plans and backlog: [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md), [BUILD-ROADMAP.md](BUILD-ROADMAP.md), [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md), [FEATURES.md](FEATURES.md) “Explicitly not yet”.

---

## Quick links

| I want… | Go here |
|---------|---------|
| Plain-language overview | [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) · [PDF](../website/public/pdfs/BOING-NETWORK-ESSENTIALS.pdf) |
| Whitepaper | [BOING-NETWORK-WHITEPAPER.md](BOING-NETWORK-WHITEPAPER.md) · [PDF](../website/public/pdfs/BOING-NETWORK-WHITEPAPER.pdf) |
| Full RPC catalog | [RPC-API-SPEC.md](RPC-API-SPEC.md) · [PDF](../website/public/pdfs/RPC-API-SPEC.pdf) |
| VM / crypto / gas | [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) |
| What shipped when | [FEATURES.md](FEATURES.md) · [PDF](../website/public/pdfs/FEATURES.pdf) |
| Tx encoding | [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md) |
| Run / upgrade a node | [RUNBOOK.md](RUNBOOK.md) · [FLY-IO.md](FLY-IO.md) |
| Docs map | [README.md](README.md) |

---

*Boing Network — Quality, enforced by the protocol.*  
*Aligned with FEATURES / live RPC probe facts as of 2026-11-01; re-check tip and activation fields before asserting later gates.*
