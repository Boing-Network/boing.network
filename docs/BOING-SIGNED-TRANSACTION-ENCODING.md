# Boing `SignedTransaction` — bincode layout (JS ↔ Rust)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This spec matches `crates/boing-primitives` serialization using **bincode 1.3** with serde’s default derived encoding. The TypeScript implementation lives in `boing-sdk/src/bincode.ts` and is verified against `cargo run -p boing-primitives --example dump_bincode`.

---

## Endianness and integers

| Type | Encoding |
|------|----------|
| `u32` | 4 bytes, little-endian |
| `u64` | 8 bytes, little-endian |
| `u128` | 16 bytes, little-endian |

---

## `AccountId`

32 raw bytes (Ed25519 public key or derived contract id). No length prefix when embedded in structs.

---

## `AccessList`

Two fields in order:

1. `read: Vec<AccountId>` — `u64` length + each `AccountId` as 32 bytes.
2. `write: Vec<AccountId>` — same.

An empty list serializes as `u64` zero (8 bytes of `0x00`).

---

## `TransactionPayload` (enum)

Variant discriminant is **`u32` LE**, in this **exact** declaration order (must match `TransactionPayload` in `types.rs`):

| Index | Variant | Fields (after discriminant) |
|-------|---------|-----------------------------|
| 0 | `Transfer` | `to: AccountId` (32 B), `amount: u128` (16 B LE) |
| 1 | `ContractCall` | `contract` (32 B), `calldata: Vec<u8>` (`u64` len + bytes) |
| 2 | `ContractDeploy` | `bytecode` vec, `create2_salt: Option<[u8;32]>` |
| 3 | `ContractDeployWithPurpose` | bytecode vec, `purpose_category` string, `description_hash: Option<Vec<u8>>`, `create2_salt` |
| 4 | `ContractDeployWithPurposeAndMetadata` | bytecode vec, purpose string, `description_hash` option, `asset_name: Option<String>`, `asset_symbol: Option<String>`, `create2_salt` |
| 5 | `Bond` | `amount: u128` |
| 6 | `Unbond` | `amount: u128` |
| 7 | `ClaimUnbond` | (none) |
| 8 | `QaPoolVote` | `subject` (32 B), `vote` (`u32` LE: 0 allow, 1 reject, 2 abstain) |
| 9 | `Batch` | `calls: Vec<BatchCall>` (`u64` count, then each: `u32` 0 = Transfer `to`+`amount`, 1 = ContractCall `contract`+`calldata` vec). Max 16 calls; see [NATIVE-BATCH-TRANSACTIONS.md](NATIVE-BATCH-TRANSACTIONS.md). From testnet height 1150 also `u32` 2 = ContractCallWithValue `contract`+`calldata` vec+`value: u128` |
| 10 | `ValidatorRegister` | see `types.rs` (open-validator mode) |
| 11 | `ValidatorDeregister` | see `types.rs` |
| 12 | `ValidatorUnjail` | see `types.rs` |
| 13-19 | `NameCommit`, `NameRegister`, `NameRenew`, `NameTransfer`, `NameSetTarget`, `NameSetPrimary`, `NameRelease` | see [RPC-API-SPEC.md](RPC-API-SPEC.md) (Name registry) and `types.rs` |
| 20 | `ContractCallWithValue` | `contract` (32 B), `calldata` vec, `value: u128` (16 B LE, base units). Active from testnet height 1150; see [NATIVE-VALUE-AND-WBOING.md](NATIVE-VALUE-AND-WBOING.md) |
| 21 | `PutBlob` | L1 media blob ([BOING-L1-MEDIA-BLOBS.md](BOING-L1-MEDIA-BLOBS.md)) |
| 22 | `BlobAnnounce` | media blob |
| 23 | `RenewMediaBlobPin` | media blob |
| 24 | `PayBlobSample` | DAS sample market |
| 25 | `RegisterBlobSampleOffer` | DAS sample market |
| 26 | `SettleBlobSampleMatch` | DAS sample market |
| 27 | `CancelBlobSampleOffer` | DAS sample market |
| 28 | `ChallengeBlobSampleServe` | DAS serve challenge |
| 29 | `RespondBlobSampleServe` | DAS serve challenge |
| 30 | `SubmitSlashAppeal` | governance / slashing |
| 31 | `ResolveSlashAppeal` | governance / slashing (unattested, stake-weighted) |
| 32 | `ResolveSlashAppealAttested` | governance / slashing (Ed25519 ballot-attested; committee nets use this) |
| 33 | `ResolveSlashAppealBlsAggregate` | defined but **not consensus-admitted** |
| 34 | `GrantBlobDownload` | blob download ACL ([HANDOFF_BLOB_DOWNLOAD_ACL.md](HANDOFF_BLOB_DOWNLOAD_ACL.md)) |
| 35 | `RevokeBlobDownload` | blob download ACL |
| 36 | `ReportEquivocation` | consensus-ordered equivocation slash (two conflicting `ConsensusVote`s) |

Indices 21-36 follow the `TransactionPayload` declaration order; field layouts for them are in `types.rs` (derive order, same primitive rules as below). **Never reorder or insert variants** - discriminants are consensus-critical. Append-only at the end is how new payloads ship.

### `Vec<u8>`

`u64` LE length + raw bytes.

### `String`

`u64` LE UTF-8 byte length + UTF-8 bytes (no NUL terminator).

### `Option<T>` (serde bincode)

- **None:** single byte `0x00`.
- **Some(x):** single byte `0x01` followed by the encoding of `x`.

For `Option<[u8;32]>`, **Some** is `0x01` + 32 bytes (no inner length).

---

## `Transaction`

Field order:

1. `nonce: u64`
2. `sender: AccountId` (32 bytes)
3. `payload` — as above
4. `access_list` — as above

---

## `Signature` (inside `SignedTransaction`)

Serde uses `serialize_bytes` for the inner 64-byte array. In bincode this becomes:

- `u64` value **64** (little-endian)
- followed by **64** raw signature bytes

---

## `SignedTransaction`

1. Full `Transaction` encoding  
2. `Signature` encoding (length-prefixed 64 bytes)
3. `priority_tip: u128` (16 bytes LE) - **present only when non-zero** (`skip_serializing_if`). An untipped tx therefore has the historical byte layout (1 + 2 only), and a tipped tx has exactly 16 extra trailing bytes.

Because the trailing field is conditional, **always decode untrusted bytes with `SignedTransaction::from_bincode`** (Rust), which accepts both layouts. Raw `bincode::deserialize::<SignedTransaction>` rejects the untipped layout. JSON uses the field name `priority_tip` (decimal), omitted when `0`.

---

## Signable hash (what Ed25519 signs)

Not the bincode of the whole `SignedTransaction`. The node and wallet sign:

```text
BLAKE3( nonce_le_8_bytes
     || sender_32_bytes
     || bincode(payload)
     || bincode(access_list) )
```

32-byte BLAKE3 output is the message passed to Ed25519. Rust: `signable_transaction_hash` in `boing-primitives::signature`. TS: `signableTransactionHash()` in `boing-sdk`.

### v2: chain-bound signatures (required after activation)

The legacy hash above has no chain id, so a tx signed for one Boing chain is valid on every other
chain with the same account and nonce (security review 2026-10-16, #2). The v2 hash adds a domain tag
and the chain id:

```text
BLAKE3( "BOING_TX_V2"            (11 ASCII bytes)
     || chain_id_le_8_bytes      (testnet 6913, mainnet 6914)
     || nonce_le_8_bytes
     || sender_32_bytes
     || bincode(payload)
     || bincode(access_list) )
```

The `SignedTransaction` bytes and the tx id (`Transaction::id()`) are unchanged; only what the
signature covers changes.

| Network | v2 accepted | Legacy rejected from |
|---|---|---|
| Testnet (6913) | now | block height **900** (`BOING_TESTNET_SECURITY_V2_FROM`) |
| Mainnet (6914) / new chains with `BOING_CHAIN_ID` | genesis | genesis |
| Local dev without `BOING_CHAIN_ID` | no (unknown chain) | never |

Before activation a node accepts either format, so upgraded wallets sign v2 right away. The rule is
checked at admission (`boing_submitTransaction`, P2P gossip, QA votes). Override the height with
`BOING_TX_V2_FROM`. Nodes advertise the rule in `boing_health.tx_signing`:
`{ v2_supported, domain, chain_id, v2_from_height, v2_required, failed_tx_fee_from_height, block_signatures_from_height, registration_proof_domain, registration_proof_v2_from_height }`.

- Rust: `signable_transaction_hash_v2`, `SignedTransaction::new_for_chain`; `SignedTransaction::new`
  signs v2 when `set_default_signing_chain_id` is set (the CLI sets it from the node; `--chain-id`,
  `--legacy-signing`).
- TS (`boing-sdk`): `signableTransactionHashV2(tx, chainId)`, `signTransactionInput(tx, key, { chainId })`,
  `setDefaultSigningChainId`, `configureSigningFromNode(client)`. Helpers that take a client adopt the
  node's chain id automatically.
- Golden vector: seed `[7; 32]`, nonce 3, transfer 5 to `[9; 32]`, empty access list, chain 6913 →
  `d88a23702aa0c376d9e0f3b6a2c2948058fecda14b95d26843f5f902ece87899`.

### Priority tips (`BOING_TX_PRIORITY`)

When `priority_tip > 0` the signature must commit to the tip, so a relay cannot add or change it:

```text
BLAKE3( "BOING_TX_PRIORITY"      (17 ASCII bytes)
     || chain_id_le_8_bytes
     || nonce_le_8_bytes
     || sender_32_bytes
     || bincode(payload)
     || bincode(access_list)
     || priority_tip_le_16_bytes )
```

With `priority_tip == 0` the v2 hash above is used unchanged, so existing wallets keep working. Tips are **not** supported on the legacy (domain-less) path. Rust: `signable_transaction_hash_with_tip`, `SignedTransaction::new_for_chain_with_tip`. Economics: [OPTIONAL-PRIORITY-FEES.md](OPTIONAL-PRIORITY-FEES.md).

### Signatures in blocks (security review #16)

`Block` is `{ header, transactions, signatures }`. `signatures[i]` is the sender's `Signature` over
`transactions[i]`. From the network's block-signature activation height (testnet: **920**; new chains:
genesis; `boing_health.tx_signing.block_signatures_from_height`) every block must carry exactly one
signature per transaction, and **every importing node verifies them** with the format required at that
height (v2 for the chain after the v2 activation). A block with a missing, forged, legacy-format or
other-chain signature is rejected. Signatures that appear below the activation height are verified too.

Signatures are not part of the header or the block hash, so `tx_root` and block hashes are unchanged.
When `signatures` is empty the field is omitted, so historical blocks keep their exact bincode, JSON
and CBOR encodings. Decoders accept both the historical `(header, transactions)` bincode layout and
the new one (`Block::from_bincode`).

Blocks may also carry `priority_tips: Vec<u128>` (same order as `transactions`; omitted when empty, otherwise its length must equal `transactions.len()`). It is appended after `signatures` and is not part of the header or block hash. Because both trailing fields are conditional, **always decode blocks with `Block::from_bincode`**, never a plain bincode deserialize. JSON (`boing_getBlockByHeight`) shows `signatures` as an array
of 64-number arrays.

### Validator registration proof v2 (security review #14)

The node-key proof in `ValidatorRegister` is
`BLAKE3("BOING_OPEN_VALIDATOR_REGISTER_V2" || chain_id_le8 || owner || node_id)` from the activation
height (testnet: **920**; `registration_proof_v2_from_height`). Before it, the v1 proof
`BLAKE3("BOING_OPEN_VALIDATOR_REGISTER_V1" || owner || node_id)` (or v2) is accepted. Golden vector:
node seed `[7; 32]`, owner `[9; 32]`, chain 6913 →
`2d9b960878d673d6e84ded770590a0242fdc66ac4f34ca9d158078c7bf4b59d9`. Rust:
`sign_validator_registration_v2`; TS: `signValidatorRegistration(owner, nodeKey, { chainId })`; the CLI
and Boing Express use v2 when the node advertises a chain id.

---

## Golden vectors

Regenerate hex fixtures:

```bash
cargo run -p boing-primitives --example dump_bincode
```

The example uses a **fixed** `SigningKey::from_bytes` test seed so `SignableHash` and full `SignedTransaction` hex stay stable for CI. Update `boing-sdk/tests/bincode.test.ts` if the seed or payload fixtures change.

### Tipped-digest golden vectors (`docs/fixtures/tx-signing-golden-vectors.json`)

A machine-readable fixture covers the v2 digest (tip 0), the tipped `BOING_TX_PRIORITY` digest (tip 1, tip 1e12, tip `u128::MAX`), chain-id separation (6913 vs 6914) and a non-empty access list. Each vector lists the full `preimage_hex`, `signing_digest_hex`, the deterministic Ed25519 `signature_hex` (seed `signer_seed_hex`) and the complete `signed_tx_bincode_hex`. `u64`/`u128` values are decimal strings.

- Generated and enforced by `crates/boing-primitives/tests/tx_signing_golden.rs` (fails if the fixture is stale; regenerate intentionally with `BOING_UPDATE_GOLDEN=1 cargo test -p boing-primitives --test tx_signing_golden`).
- Consumed by `boing-sdk/tests/txSigningGolden.test.ts`.
- **Wallets (boing.express) and other signers**: reproduce every field from the fixture in your own tests. Raw URL: `https://raw.githubusercontent.com/Boing-Network/boing.network/main/docs/fixtures/tx-signing-golden-vectors.json`.

---

## References

- `crates/boing-primitives/src/types.rs` — struct/enum definitions  
- `crates/boing-primitives/src/signature.rs` — `signable_transaction_hash`, `SignedTransaction`  
- [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) — `boing_signTransaction` / `boing_sendTransaction`  
- [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) — Phase 1 track P  
