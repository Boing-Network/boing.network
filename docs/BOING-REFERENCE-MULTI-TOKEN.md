# Boing reference multi-token (shared item types, ERC-1155-style)

**Status:** reference contract template **v1** (artifact id `boing.reference_multi_token.v1`). Not consensus-enforced: it is ordinary Boing VM bytecode that runs on today's VM with **no** protocol change, no new opcode and no hashing opcode (the gated `BLAKE3` opcode stays off).

**Code:** [`crates/boing-execution/src/reference_multi_token.rs`](../crates/boing-execution/src/reference_multi_token.rs) · **SDK:** [`boing-sdk/src/referenceMultiToken.ts`](../boing-sdk/src/referenceMultiToken.ts) · **CLI:** `boing items …` · **Pinned hex:** [`artifacts/reference-multi-token-v1-deploy.hex`](artifacts/reference-multi-token-v1-deploy.hex) / [`artifacts/reference-multi-token-v1-runtime.hex`](artifacts/reference-multi-token-v1-runtime.hex) (+ `.sha256`).

## When to use it

| You want | Use |
|----------|-----|
| Every copy is unique (pets, art, tickets with seat numbers) | [Reference NFT](BOING-REFERENCE-NFT.md) — one owner per `token_id` |
| One fungible currency | [Reference fungible](BOING-REFERENCE-TOKEN.md) |
| **Many interchangeable item types in one contract** (game resources, materials, consumables, badges with a count) | **This template** — `balance[item_id][account]` |

Every player who mines "iron" holds the *same* item `iron`; the contract stores one balance per (item, holder) and **one** metadata commitment per item type, not one per copy. Minting 1,000 iron costs the same as minting 1 iron (one balance write + one supply write).

## Deploy

Deploy payload = **`0xFD` || init**. The init frame stores the deployer (`CALLER`) as **admin** and `RETURN`s the runtime, so there is **no lazy-admin race** (unlike the NFT template, nobody can become admin by calling first). Deploy with purpose category **`nft`** (existing QA category; no QA change was needed). `boing_getContractCode` afterwards returns the **runtime**; wallets and explorers identify the template by exact runtime bytes (or the deploy payload when reading history).

```bash
export BOING_SECRET_FILE=~/.boing/testnet-deployer.hex   # or --secret-hex / BOING_SECRET_HEX
boing --rpc-url https://testnet-rpc.boing.network/ items deploy --name "Pasture Resources" --symbol RES
boing items mint --contract 0x<id> --to 0x<player> --item 1 --amount 25
boing items mint-batch --contract 0x<id> --to 0x<player> --entries 1:25,2:3
boing items transfer --contract 0x<id> --to 0x<friend> --item 1 --amount 5
boing items balance --contract 0x<id> --holder 0x<player> --item 1
boing items info --contract 0x<id> --item 1
```

SDK: `buildReferenceMultiTokenDeployMetaTx({ name, symbol })` returns the Express `contract_deploy_meta` object.

## Storage layout

All words are 32 bytes, big-endian. `item_id` is a **u64** (high 24 bytes of its word must be zero); amounts and balances are **u128** (high 16 bytes zero).

| Slot | Key | Value |
|------|-----|-------|
| admin | `0x00…00e0` | admin `AccountId` |
| balance | `(account[0..24] ‖ item_id_be8) ^ REF_MT_BALANCE_XOR` | u128 |
| total supply | `item_id_word ^ REF_MT_SUPPLY_XOR` | u128 |
| item metadata | `item_id_word ^ REF_MT_METADATA_XOR` | 32-byte commitment (e.g. hash of a JSON / media blob) |

Tags (ASCII, zero-padded to 32 bytes): `REF_MT_BALANCE_XOR = "BOING_REFMT_BAL_V1"`, `REF_MT_SUPPLY_XOR = "BOING_REFMT_SUPPLY_V1"`, `REF_MT_METADATA_XOR = "BOING_REFMT_META_V1"`.

**Collision safety without hashing.** A 2-D key must fit in 32 bytes, so the balance key keeps the first **24 bytes (192 bits)** of the account plus the full 8-byte item id. Two holders collide only if they share a 192-bit account prefix (≈2⁹⁶ work for any pair, ≈2¹⁹² to target a given victim). A balance key equals a supply / metadata / admin key only if an account's first 24 bytes equal a fixed tag difference (≈2¹⁹² preimage). This is stronger than the reference fungible v2 allowance key (128 + 128 bits). Golden vectors (account `0xa1…a1`, item `7`):

```
balance  e3eee8efe6fef3e4e7ecf5fee3e0edfef790a1a1a1a1a1a10000000000000007
supply   424f494e475f5245464d545f535550504c595f56310000000000000000000007
metadata 424f494e475f5245464d545f4d4554415f563100000000000000000000000007
```

## Selectors and calldata

Selector = low byte of word 0 (same convention as every Boing reference template). Range `0x20`–`0x29` does not overlap the token (`0x01`–`0x0a`) or NFT (`0x03`–`0x06`) selectors, so wallets can tell the calls apart.

| Sel | Call | Calldata (bytes) | Who |
|-----|------|------------------|-----|
| `0x20` | `balance_of(account, item_id)` → word | 96 | anyone |
| `0x21` | `transfer(to, item_id, amount)` | 128 | holder (moves the caller's own balance) |
| `0x22` | `mint(to, item_id, amount)` | 128 | admin |
| `0x23` | `set_item_metadata(item_id, hash)` | 96 | admin |
| `0x24` | `mint_batch(to, n, item_ids[n], amounts[n])` | 96 + 64n, 1 ≤ n ≤ 16 | admin |
| `0x25` | `transfer_batch(to, n, item_ids[n], amounts[n])` | 96 + 64n, 1 ≤ n ≤ 16 | holder |
| `0x26` | `total_supply(item_id)` → word | 64 | anyone |
| `0x27` | `item_metadata(item_id)` → word | 64 | anyone |
| `0x28` | `admin()` → word | 32 | anyone |
| `0x29` | `transfer_admin(new_admin)` | 64 | admin |

Batch layout: word0 selector, word1 `to`, word2 `n` (u64 in low 8 bytes), then `n` item-id words, then `n` amount words. A non-zero word right after the arrays (length mismatch) faults. Duplicate item ids in one batch are fine (they accumulate). `MAX_REFERENCE_MULTI_TOKEN_BATCH = 16` because every entry emits one log and the VM allows 24 logs per transaction.

**Failure semantics.** Every rejected call (non-admin mint, insufficient balance, `amount == 0`, `to == 0`, item id ≥ 2⁶⁴, amount or resulting balance/supply ≥ 2¹²⁸, bad batch length, unknown selector) `JUMP`s out of range → `VmError::InvalidJump`; the block executor reverts the whole transaction, so batches are all-or-nothing. Self-transfer is a no-op on the balance (debit is written before the credit is read).

## Logs (for indexers)

| Event | Opcode | Topics | Data |
|-------|--------|--------|------|
| Transfer / mint | `LOG3` | `[REF_MT_TOPIC_TRANSFER, from, to]` (`from = 0` for mints) | `item_id_word ‖ amount_word` (64 bytes) |
| Item metadata set | `LOG2` | `[REF_MT_TOPIC_METADATA, item_id_word]` | `hash` (32 bytes) |
| Admin transferred | `LOG3` | `[REF_MT_TOPIC_ADMIN, old_admin, new_admin]` | — |

Topic tags: `"BOING_REFMT_TRANSFER_V1"`, `"BOING_REFMT_METADATA_V1"`, `"BOING_REFMT_ADMIN_V1"` (ASCII, zero-padded). Nodes do not enumerate storage, so "which item ids does this account hold?" is answered by scanning these logs (or calldata) and then reading the exact balance slot with `boing_getContractStorage`. SDK: `decodeMultiTokenTransferLog`, `decodeMultiTokenCalldata`, `fetchMultiTokenBalance(s)`, `fetchMultiTokenItemInfo`.

## Gas (measured, `cargo test -p boing-execution reference_multi_token -- --nocapture`)

| Operation | Gas |
|-----------|-----|
| `mint` (new holder, new item) | ≈ 42.5k (2 `SSTORE` + log) |
| `transfer` | ≈ 42.4k (2 `SSTORE` + log) |
| `mint_batch` n = 16 | ≈ 678k |
| `transfer_batch` n = 16 | ≈ 678k |
| `balance_of` (call) | ≈ 230 — or free via `boing_getContractStorage` |

All far below `GAS_PER_CONTRACT_CALL = 40_000_000`. Runtime is ≈ 2.1 KB, deploy payload ≈ 2.3 KB (uses minimal-width `PUSHn`).

## Security notes

- Admin can mint any amount of any item id and set metadata; hand admin to a game server key with `transfer_admin` and keep the deployer key offline if you like. There is no burn and no operator-approval (`setApprovalForAll`) in v1; players move only their own balances.
- Metadata is a commitment (hash), not a URI. Publish the JSON / image off-chain or as an L1 media blob and store its hash.
- Compressed NFTs (only a Merkle root on chain) are **not** part of v1: they need an on-chain hash opcode (`BLAKE3`, currently gated off) and only pay off at very large scale.

## Testnet

See the "Testnet soak" section below for the reference deployment used to verify v1 on chain 6913.
