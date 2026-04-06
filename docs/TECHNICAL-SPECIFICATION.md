# Boing Network — Technical Specification

> **Purpose:** Single source of truth for all technical specifications: software stack, cryptography, data formats, bytecode, gas, RPC API, and protocol rules.  
> **References:** [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md), [RPC-API-SPEC.md](RPC-API-SPEC.md), [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Tech Stack](#2-tech-stack)
3. [Cryptography](#3-cryptography)
4. [Data Structures](#4-data-structures)
5. [Serialization (Bincode)](#5-serialization-bincode)
6. [Transaction Signing](#6-transaction-signing)
7. [VM & Bytecode](#7-vm--bytecode)
8. [Gas Model](#8-gas-model)
9. [Protocol QA Rules](#9-protocol-qa-rules)
10. [Block & Ledger Structure](#10-block--ledger-structure)
11. [JSON-RPC API](#11-json-rpc-api)
12. [Networking](#12-networking)
13. [Implementation Crates](#13-implementation-crates)
14. [Cross-References](#14-cross-references)

---

## 1. Overview

Boing Network is an **L1 blockchain** built from first principles in Rust. It uses a custom stack-based VM, BLAKE3 for hashing, Ed25519 for signatures, PoS + HotStuff BFT for consensus, and protocol-enforced quality assurance for deployments.

---

## 2. Tech Stack

| Layer | Technology |
|-------|------------|
| **Language** | Rust |
| **Hashing** | BLAKE3 |
| **Signatures** | Ed25519 (64-byte) |
| **Consensus** | PoS + HotStuff BFT |
| **State** | Sparse Merkle tree (Verkle target) |
| **Execution** | **Boing VM** (stack-based; ISA defined in §7 — independent of other chain VMs) |
| **Networking** | libp2p (TCP, Noise, gossipsub, request-response) |
| **Governance** | Phased (proposal → cooling → execution); time-locked |

---

## 3. Cryptography

### 3.1 Hashing — BLAKE3

- **Algorithm:** BLAKE3
- **Output:** 32 bytes
- **Usage:** Transaction ID, block hash, signable message, Merkle roots, account proofs

### 3.2 Signatures — Ed25519

- **Algorithm:** Ed25519 (ed25519-dalek)
- **Signature size:** 64 bytes
- **Public key:** 32 bytes (used as `AccountId`)
- **Usage:** Transaction authorization; sender's `AccountId` must equal signer's public key

### 3.3 Address (AccountId)

- **Size:** 32 bytes
- **Display:** 64 hex characters (optional `0x` prefix)
- **Derivation:** Typically from Ed25519 public key; raw 32 bytes otherwise

---

## 4. Data Structures

### 4.1 AccountId

```text
AccountId = [u8; 32]
```

32-byte opaque identifier. Typically derived from Ed25519 public key.

### 4.2 AccessList

```text
AccessList = {
  read:  Vec<AccountId>,
  write: Vec<AccountId>,
}
```

Declares accounts this transaction reads/writes. Enables parallel scheduling (txs with disjoint access lists run in parallel).

**Conflict rule:** Two transactions **conflict** for scheduling if any `AccountId` appears in **either** list (`read` or `write`) of one tx and **either** list of the other (`AccessList::conflicts_with`). Disjoint access lists allow the same batch to run transfers in parallel.

**Soundness (required for correct parallel execution):** Every account whose state this transaction **may read or write** during execution must appear in the access list. Listing too few accounts can allow the scheduler to place conflicting txs in the same parallel batch; the executor then fails with a conflict error or, in worst cases, could compromise correctness if checks were incomplete. Listing redundant accounts is safe but reduces parallelism.

**Recommended sets by payload (minimal patterns used in tests and tooling):**

| Payload | `read` | `write` | Notes |
|---------|--------|---------|--------|
| `Transfer` | `sender`, `to` | `sender`, `to` | Sender: nonce + balance; recipient: balance (or account creation). |
| `Bond` / `Unbond` | `sender` | `sender` | Only sender stake/balance/nonce change. |
| `ContractCall` | `sender`, `contract`, … | `sender`, `contract`, … | Minimum: `sender` (nonce) and `contract` (code + contract storage). The interpreter’s `SLOAD`/`SSTORE` use the callee’s storage only; the VM does **not** infer extra accounts from bytecode—list any others if future host hooks read additional state. |
| `ContractDeploy` (variants) | At least `sender`; if `create2_salt` is set, also the **predicted** CREATE2 contract `AccountId` (`create2_contract_address(sender, salt, bytecode)` in `boing-primitives`) | Same | Nonce and balance update on `sender`; the new contract account is created at a deterministic address (nonce-derived or CREATE2). **Parallel scheduling:** when using CREATE2, the suggested list includes the predicted `AccountId` so another tx cannot be scheduled as if that account were still unused in the same batch. |

**Mempool / signing:** The access list is part of the signed `Transaction` (hashed into `tx.id()`). Wallets should build lists consistent with the table above and any extra accounts the dApp knows the contract will touch.

### 4.3 Transaction

```text
Transaction = {
  nonce:       u64,
  sender:      AccountId,
  payload:     TransactionPayload,
  access_list: AccessList,
}
```

**Transaction ID:** `BLAKE3(bincode(Transaction))` (32 bytes).

### 4.4 TransactionPayload

```text
TransactionPayload = enum {
  Transfer { to: AccountId, amount: u128 },
  Bond { amount: u128 },
  Unbond { amount: u128 },
  ContractCall { contract: AccountId, calldata: Vec<u8> },
  ContractDeploy {
    bytecode:        Vec<u8>,
    create2_salt:    Option<[u8; 32]>,   // None → nonce-derived contract address
  },
  ContractDeployWithPurpose {
    bytecode:           Vec<u8>,
    purpose_category:   String,
    description_hash:   Option<Vec<u8>>,
    create2_salt:       Option<[u8; 32]>,
  },
  ContractDeployWithPurposeAndMetadata {
    bytecode:           Vec<u8>,
    purpose_category:   String,
    description_hash:   Option<Vec<u8>>,
    asset_name:         Option<String>,
    asset_symbol:       Option<String>,
    create2_salt:       Option<[u8; 32]>,
  },
}
```

**Salt-derived deploy (`create2_salt` / `create2_contract_address`):** When `create2_salt` is `Some(salt)`, the new contract’s `AccountId` is `create2_contract_address` in `boing_primitives`: `BLAKE3("boing.create2.v1\0" || deployer || salt || BLAKE3(bytecode))`. When `create2_salt` is `None`, the address is the legacy nonce hash `nonce_derived_contract_address(sender, tx.nonce)` using the **transaction nonce before this deploy is applied**. If the target address already has an account or stored bytecode, execution fails with `DeploymentAddressInUse` and the sender nonce is **not** incremented.

**Deploy init code (optional, `CONTRACT_DEPLOY_INIT_CODE_MARKER` = `0xFD`):** If the first byte of `bytecode` is `0xFD`, the remainder is **init code**: the node runs it once with `CALLER` = deployer and `ADDRESS` = the new contract account (before runtime code is installed). `LOG0`–`LOG4` during that run are included on the deploy transaction’s execution receipt. The slice supplied to `RETURN` becomes the **runtime bytecode** stored for the contract (empty if execution stops via `STOP` without `RETURN`). Gas for the deploy is the fixed deploy base plus metered init gas (same per-op schedule as `ContractCall`, subject to the deploy/init gas limit). Payloads **without** the `0xFD` prefix keep legacy behavior: the full `bytecode` vector is stored as contract code and no VM execution runs at deploy. The marker byte is **not** a valid VM opcode; QA and static walks apply to the bytes **after** the prefix. The **full** payload (including `0xFD` when present) is hashed for salt-derived address derivation and mempool blocklist checks.

**Bincode:** Adding `create2_salt` changes the serialized shape of deploy payloads versus older clients; wallets must match `boing-primitives` serde layout.

### 4.5 AccountState

```text
AccountState = {
  balance: u128,
  nonce:   u64,
  stake:   u128,
}
```

- **balance:** Spendable BOING (smallest units)
- **nonce:** Sequence number for replay protection
- **stake:** Bonded stake (for validation)

### 4.6 Account

```text
Account = {
  id:    AccountId,
  state: AccountState,
}
```

### 4.7 BlockHeader

```text
BlockHeader = {
  parent_hash:    Hash,
  height:         u64,
  timestamp:      u64,
  proposer:       AccountId,
  tx_root:        Hash,
  receipts_root:  Hash,
  state_root:     Hash,
}
```

### 4.8 Block

```text
Block = {
  header:       BlockHeader,
  transactions: Vec<Transaction>,
}
```

**Block hash:** `BLAKE3(bincode(BlockHeader))`.

**Transaction root:** Merkle root of `tx.id()` for all transactions (binary tree).

### 4.9 SignedTransaction

```text
SignedTransaction = {
  tx:        Transaction,
  signature: Signature,
}
```

**Signature:** 64-byte Ed25519 over signable hash (see [§6 Transaction Signing](#6-transaction-signing)).

---

## 5. Serialization (Bincode)

All on-wire and stored structures use **bincode** (little-endian, compact format).

- **Transaction:** `bincode::serialize(&tx)` — used for tx ID and signing
- **SignedTransaction:** `bincode::serialize(&signed_tx)` — submitted to RPC as hex
- **Block/BlockHeader:** bincode for hashing and storage

**RPC submission:** `hex(bincode(SignedTransaction))` — e.g. `0x1234...`

---

## 6. Transaction Signing

### 6.1 Signable Message

```text
signable_message = BLAKE3(
  nonce.to_le_bytes() ||
  sender.0[32] ||
  bincode(payload) ||
  bincode(access_list)
)
```

- **nonce:** 8 bytes, little-endian
- **sender:** 32 bytes (AccountId)
- **payload:** bincode-serialized `TransactionPayload`
- **access_list:** bincode-serialized `AccessList`

### 6.2 Signature

```text
signature = Ed25519_sign(signable_message)
```

64-byte Ed25519 signature. Verifier uses `sender` as public key.

### 6.3 SignedTransaction

```text
SignedTransaction = { tx, signature }
```

Submit: `hex(bincode(SignedTransaction))` to `boing_submitTransaction([hex_signed_tx])`.

---

## 7. VM & Bytecode

### 7.1 VM Model

- **Type:** Stack-based, deterministic
- **ISA:** The **Boing VM** is its own instruction set. Opcode bytes and semantics are **Boing-defined** here and in `boing-execution` (see [BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md)).
- **Implementation:** `boing-execution` crate

### 7.2 Opcodes

| Opcode | Hex | Gas | Description |
|--------|-----|-----|-------------|
| **Stop** | `0x00` | 0 | Halt execution |
| **Add** | `0x01` | 3 | Add top two stack values |
| **Sub** | `0x02` | 3 | Subtract |
| **Mul** | `0x03` | 5 | Pop `b`, `a` (stack top = `b`). Push `(a × b) mod 2^256` using full **256×256** operands (low **256** bits of the product; same width as **MulMod** before reduce). |
| **Div** | `0x04` | 5 | Unsigned division (256-bit); divisor zero → VM fault |
| **Mod** | `0x06` | 5 | Unsigned remainder; divisor zero → VM fault |
| **AddMod** | `0x08` | 8 | Pop `n`, `b`, `a` (stack top = `n`). Push `(a + b) mod n` using full-width addition before reduce. If `n = 0`, push `0` (no fault). |
| **MulMod** | `0x09` | 8 | Pop `n`, `b`, `a`. Push `(a × b) mod n` using full 512-bit product before reduce. If `n = 0`, push `0`. |
| **Lt** | `0x10` | 3 | Less-than (unsigned 256-bit stack words) |
| **Gt** | `0x11` | 3 | Greater-than (unsigned) |
| **Eq** | `0x14` | 3 | Equality |
| **IsZero** | `0x15` | 3 | 1 if top word is zero |
| **And** | `0x16` | 3 | Bitwise AND |
| **Or** | `0x17` | 3 | Bitwise OR |
| **Xor** | `0x18` | 3 | Bitwise XOR |
| **Not** | `0x19` | 3 | Bitwise NOT |
| **Shl** | `0x1b` | 3 | Shift left: pop `shift`, pop `value` (stack top = `shift` word). Effective shift = unsigned `shift` mod 256 (big-endian low byte `shift[31]`). Push `(value << k) mod 2^256`. |
| **Shr** | `0x1c` | 3 | Logical shift right: same stack order and effective `k`; unsigned `value >> k`. |
| **Sar** | `0x1d` | 3 | Arithmetic shift right: same stack order and `k`; signed two’s-complement 256-bit; sign-extending fill. |
| **Address** | `0x30` | 2 | Push this contract’s `AccountId` (32-byte word) |
| **Caller** | `0x33` | 2 | Push the **immediate caller** `AccountId` (32-byte word): at **top-level** execution this is the transaction signer (`tx.sender`); after a nested **`Call`**, it is the **contract that performed `Call`** (the previous frame’s `Address`). |
| **Call** | `0xf1` | 700 + callee gas | Nested contract call. Pops `ret_size`, `ret_offset`, `args_size`, `args_offset`, `target` (stack top = `ret_size`). Copies `memory[args_offset..args_offset+args_size)` as calldata. Callee runs with `caller_id` = current contract and `contract_id` = `target`. Merges callee logs into the current receipt. Writes return data to caller memory (zero-pad to `ret_size`). Pushes `1` on success. **`target` has no code** → success, empty return, `1` pushed. `args_size` / `ret_size` must not exceed **24 KiB** each. Max **64** nested depth. Callee **faults** (`OutOfGas`, `InvalidBytecode`, …) **propagate** and abort the whole transaction (no callee-state rollback). Remaining gas after the **700** base is the callee’s `gas_limit`. |
| **Create2** | `0xf5` | 32,000 + 200×`size` + init gas | In-contract deploy (CREATE2-style). Pops `salt` (word), `size`, `offset` (stack top = `salt`). Reads `memory[offset..offset+size)` as the **full deploy payload** (same rules as `ContractDeploy` bytecode: optional `0xFD` init marker; QA uses category **`dapp`** for this path). Address is `create2_contract_address` with deployer = **current contract** (`ADDRESS`). Inserts the account, runs init with `CALLER` = deployer, merges init logs, stores runtime code. Pushes the new **contract address** (32-byte word). **Does not** increment the factory’s nonce. Init meters gas from the remaining budget after the opcode’s base + linear charge; `size` may not exceed the governance bytecode size cap (**32 KiB** default). |
| **Dup1** | `0x80` | 3 | Duplicate top stack word |
| **Log0** | `0xa0` | dynamic | Pop `offset`, `size`; append log with empty topics and `memory[offset..size)` as data (bounds + per-tx limits apply) |
| **Log1** | `0xa1` | dynamic | Pop `offset`, `size`, one topic (32-byte words); same as Log0 with one indexed topic |
| **Log2** | `0xa2` | dynamic | Two topics + data |
| **Log3** | `0xa3` | dynamic | Three topics + data |
| **Log4** | `0xa4` | dynamic | Four topics + data |
| **MLoad** | `0x51` | 3 | Load from memory at offset |
| **MStore** | `0x52` | 3 | Store to memory |
| **SLoad** | `0x54` | 100 | Load from storage |
| **SStore** | `0x55` | 20,000 | Store to storage |
| **Jump** | `0x56` | 8 | Pop and jump to offset |
| **JumpI** | `0x57` | 10 | Conditional jump |
| **Push1** | `0x60` | 3 | Push 1-byte immediate |
| **Push2** | `0x61` | 3 | Push 2-byte immediate |
| **…** | … | 3 | … |
| **Push32** | `0x7f` | 3 | Push 32-byte immediate |
| **Return** | `0xf3` | 0 | Return memory slice |

**`Call` stack layout:** Stack top is popped first: `ret_size`, `ret_offset`, `args_size`, `args_offset`, then `target` (32-byte word, big-endian `AccountId`).

**`Create2` stack layout:** Stack top is popped first: `salt` (word), `size`, `offset` (push `offset`, then `size`, then `salt` so `salt` is on top before `0xf5`).

**PUSH encoding:** For `0x60`..`0x7f`, immediate length = `byte - 0x5f` (PUSH1 = 1 byte, PUSH32 = 32 bytes).

**LOG stack layout:** For `LOGn`, stack top is `offset`, then `size`, then topic words in order `topic_{n-1}` … `topic_0` below (so push topics first, then `size`, then `offset`). Gas scales with topic count and data length.

**Execution logs (receipts):** Each contract call may emit up to **24** logs; each log has at most **4** topics (32-byte words each) and **1024** bytes of data (`ExecutionReceipt.logs` in `boing-primitives`).

**Reference fungible calldata** (optional convention for wallets): see [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md).

### 7.3 Well-Formedness

- All bytes must decode to a valid opcode or be part of a PUSH immediate.
- PUSH immediates must have correct length (no truncated instructions).
- No jump targets to non-instruction boundaries.
- No trailing bytes at end of bytecode.

### 7.4 Bytecode Limits

- **Maximum size:** 32 KiB (32 × 1024 bytes) — enforced by protocol QA.

---

## 8. Gas Model

### 8.1 Base Costs (per transaction type)

| Tx Type | Base Gas |
|---------|----------|
| Transfer | 21,000 |
| ContractCall | 100,000 |
| ContractDeploy | 200,000 |
| Bond | 21,000 |
| Unbond | 21,000 |

### 8.2 Opcode Gas

See [§7.2 Opcodes](#72-opcodes). Contract execution adds opcode costs to base `CONTRACT_CALL` or `CONTRACT_DEPLOY`.

### 8.3 Adaptive Multiplier

- Base multiplier: 1.0
- Max multiplier: 2.0 (under congestion)
- Configurable via `GasConfig.multiplier_e4` (e.g. 10000 = 1.0, 15000 = 1.5)

---

## 9. Protocol QA Rules

Deployments must pass protocol QA before inclusion. See [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) for full design.

### 9.1 Bytecode Rules (Hard)

| Rule | Limit | rule_id |
|------|-------|---------|
| Bytecode size | ≤ 32 KiB | `MAX_BYTECODE_SIZE` |
| Valid opcodes only | Boing VM set in §7.2 (includes Address, Caller, Dup1, Log0..Log4, arithmetic, compare, bitwise, memory, storage, jump, push, return) | `INVALID_OPCODE` |
| Well-formed | No truncated PUSH, no trailing bytes | `MALFORMED_BYTECODE` |
| Not blocklisted | Bytecode hash not in blocklist | `BLOCKLIST_MATCH` |

### 9.2 Purpose (when declared)

- Valid categories: `dApp`, `token`, `NFT`, `meme`, `community`, `entertainment`, `tooling`, `other`
- Invalid category → `PURPOSE_DECLARATION_INVALID`
- Scam pattern match → `SCAM_PATTERN_MATCH`

### 9.3 Pre-flight Check

Use `boing_qaCheck([hex_bytecode])` or extend with optional `purpose_category`, `description_hash`, `asset_name`, `asset_symbol` (same order as [RPC-API-SPEC.md](RPC-API-SPEC.md) § boing_qaCheck) before submit.

---

## 10. Block & Ledger Structure

- **Chain:** Linear chain of blocks
- **Block hash:** BLAKE3 of bincode-serialized `BlockHeader`
- **Tx root:** Merkle root of transaction IDs (binary tree)
- **State root:** Root of Sparse Merkle tree (or equivalent)

---

## 11. JSON-RPC API

### 11.1 Transport

- **Protocol:** JSON-RPC 2.0
- **Transport:** HTTP POST
- **Base URL:** `http://<host>:<rpc_port>/`
- **Default port:** 8545

### 11.2 Methods Summary

**Source of truth:** method names and handlers in `crates/boing-node/src/rpc.rs` (`BOING_RPC_SUPPORTED_METHODS`). Full params and examples: [RPC-API-SPEC.md](RPC-API-SPEC.md).

| Method | Params | Result (summary) |
|--------|--------|-------------------|
| `boing_chainHeight` | `[]` | `u64` — committed tip height |
| `boing_clientVersion` | `[]` | string (e.g. `boing-node/0.1.0`) |
| `boing_rpcSupportedMethods` | `[]` | string[] — sorted `boing_*` names |
| `boing_health` | `[]` | `{ ok, client_version, chain_id?, chain_name?, head_height, rpc_surface, rpc_metrics }` |
| `boing_getSyncState` | `[]` | `{ head_height, finalized_height, latest_block_hash }` |
| `boing_getNetworkInfo` | `[]` | Chain metadata, tip, `chain_native`, `developer`, `rpc_surface`, `end_user`, `rpc.not_available`, … |
| `boing_getRpcMethodCatalog` | `[]` | Embedded method catalog (JSON Schema–style hints) |
| `boing_getRpcOpenApi` | `[]` | OpenAPI 3.1 root for `POST /`, `GET /ws`, probes |
| `boing_getBalance` | `[hex_account_id]` | `{ balance: string }` |
| `boing_getAccount` | `[hex_account_id]` | `{ balance, nonce, stake }` |
| `boing_getBlockByHeight` | `[height]` or `[height, include_receipts]` | Block or `null` |
| `boing_getBlockByHash` | `[hex_block_hash]` or `[hex_block_hash, include_receipts]` | Block or `null` |
| `boing_getTransactionReceipt` | `[hex_tx_id]` | Receipt or `null` |
| `boing_getLogs` | `[filter]` | Log entry array (bounded span / cap) |
| `boing_getAccountProof` | `[hex_account_id]` | `{ proof, root, value_hash }` |
| `boing_verifyAccountProof` | `[hex_proof, hex_state_root]` | `{ valid: boolean }` |
| `boing_getContractStorage` | `[hex_contract_id, hex_key_32]` | `{ value: string }` — 32-byte word hex |
| `boing_simulateTransaction` | `[hex_signed_tx]` | `{ gas_used, success, return_data, logs?, error?, suggested_access_list, access_list_covers_suggestion }` |
| `boing_submitTransaction` | `[hex_signed_tx]` | `{ tx_hash: string }` — on accept, current node uses literal `"ok"`; see [RPC-API-SPEC.md](RPC-API-SPEC.md) § boing_submitTransaction |
| `boing_registerDappMetrics` | `[hex_contract, hex_owner]` | `{ registered: true, contract, owner }` |
| `boing_submitIntent` | `[hex_signed_intent]` | `{ intent_id: string }` |
| `boing_qaCheck` | `[hex_bytecode]` or `[hex_bytecode, purpose_category?, description_hash?, asset_name?, asset_symbol?]` | `{ result, rule_id?, message?, doc_url? }` |
| `boing_qaPoolList` | `[]` | `{ items: [...] }` |
| `boing_qaPoolConfig` | `[]` | Pool governance + queue snapshot |
| `boing_getQaRegistry` | `[]` | Effective QA rule registry JSON |
| `boing_qaPoolVote` | `[tx_hash_hex, voter_hex, vote]` | Vote outcome object |
| `boing_operatorApplyQaPolicy` | `[qa_registry_json, qa_pool_config_json]` | `{ ok: true }` (operator auth when configured) |
| `boing_faucetRequest` | `[hex_account_id]` | `{ ok: true, amount, to, message }` (only when `--faucet-enable`) |

### 11.3 Error Codes

| Code | Meaning |
|------|---------|
| -32600 | Invalid Request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32000 | Server error |
| -32016 | Rate limit exceeded |
| -32050 | QA: Deployment rejected |
| -32051 | QA: Pending pool |
| -32052 … -32057 | QA pool / operator RPC (see [RPC-API-SPEC.md](RPC-API-SPEC.md) § Error Codes) |

**Reference:** [RPC-API-SPEC.md](RPC-API-SPEC.md) for full method and error details.

---

## 12. Networking

### 12.1 Stack

- **Library:** libp2p
- **Transports:** TCP
- **Security:** Noise
- **Stream multiplexing:** yamux
- **Protocols:** gossipsub, request-response

### 12.2 Discovery

- mDNS (local)
- Bootstrap lists
- **Target:** DHT (Kademlia) + gossip-first overlay; bootnode rotation (see [DECENTRALIZATION-AND-NETWORKING.md](DECENTRALIZATION-AND-NETWORKING.md))

### 12.3 Gossip topics (blocks and transactions)

| Topic | Payload | Notes |
|-------|---------|--------|
| `boing/blocks` | `bincode(Block)` | Block fan-out; peers still sync via request/response (`/boing/block-sync/1`) if gossip is slow. |
| `boing/transactions` | `bincode(SignedTransaction)` | After a successful **`boing_submitTransaction`** / **`boing_faucetRequest`**, the node gossips the **signed** tx; peers verify the Ed25519 signature and run the same mempool + QA admission as RPC. |

**Mesh sizing:** libp2p gossipsub uses default mesh parameters (see upstream `mesh_n` / `mesh_n_low`). Very small graphs (e.g. two TCP peers) may not form a reliable topic mesh; production testnets should run **enough interconnected peers** or tune gossipsub in `boing-p2p`. Regression: `cargo test -p boing-node --test p2p_tx_gossip_rpc` (four-node full mesh).

**P2P → node delivery:** `boing-p2p` forwards gossip to `boing-node` over an **unbounded** async channel so the swarm task never **awaits** on a full bounded queue (which could stall connections and propagation).

**Limits:** Optional cap on simultaneous connections **per remote IP** (`RateLimitConfig.connections_per_ip`, CLI **`--max-connections-per-ip`**) — see [RUNBOOK.md](RUNBOOK.md) §8.1.

---

## 13. Implementation Crates

| Crate | Role |
|-------|------|
| `boing-primitives` | Types, BLAKE3, Ed25519, Transaction, Block, AccountId, SignedTransaction |
| `boing-consensus` | PoS + HotStuff BFT |
| `boing-state` | State store, state root, checkpoints |
| `boing-execution` | VM, bytecode, interpreter, BlockExecutor, TransactionScheduler |
| `boing-tokenomics` | Block emission, dApp incentives |
| `boing-governance` | Time-locked governance, slashing appeal |
| `boing-automation` | Scheduler, triggers, executor incentives, verification |
| `boing-qa` | Protocol QA: Allow/Reject/Unsure checks |
| `boing-cli` | `boing init`, `boing dev`, `boing deploy` |
| `boing-p2p` | libp2p networking |
| `boing-node` | Node binary (RPC, mempool, block producer, chain) |

---

## 14. Cross-References

| Document | Use for |
|----------|---------|
| [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) | Six pillars, design philosophy, quick reference |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Full architecture, innovations, tokenomics |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | Complete RPC method specs and examples |
| [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) | QA design, automation, community pool; Appendix A: deployer checklist |
| [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) | Wallet integration, signing spec |
| [RUNBOOK.md](RUNBOOK.md) | Node operation, P2P gossip / per-IP limits (§8.1), monitoring, incidents |
| [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) | Testnet ops + public RPC + infra routing and env matrix |
| [DECENTRALIZATION-AND-NETWORKING.md](DECENTRALIZATION-AND-NETWORKING.md) | P2P strategy, discovery roadmap, WebRTC signaling |
| [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) | Security requirements and practices |
| [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) | Actionable tasks for VM, receipts, RPC finality, token standards (pillar-aligned) |
| [BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md) | Boing VM only — no foreign chain bytecode runtimes |

---

**Source of truth:** `crates/boing-primitives`, `crates/boing-execution`, `crates/boing-qa`, `crates/boing-node`. This document reflects the implementation; when in doubt, refer to the code.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
