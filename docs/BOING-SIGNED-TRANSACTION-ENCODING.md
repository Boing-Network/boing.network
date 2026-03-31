# Boing `SignedTransaction` — bincode layout (JS ↔ Rust)

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

---

## Golden vectors

Regenerate hex fixtures:

```bash
cargo run -p boing-primitives --example dump_bincode
```

The example uses a **fixed** `SigningKey::from_bytes` test seed so `SignableHash` and full `SignedTransaction` hex stay stable for CI. Update `boing-sdk/tests/bincode.test.ts` if the seed or payload fixtures change.

---

## References

- `crates/boing-primitives/src/types.rs` — struct/enum definitions  
- `crates/boing-primitives/src/signature.rs` — `signable_transaction_hash`, `SignedTransaction`  
- [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) — `boing_signTransaction` / `boing_sendTransaction`  
- [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) — Phase 1 track P  
