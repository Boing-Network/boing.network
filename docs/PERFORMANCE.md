# Performance notes and benchmarks

Measured on a Windows 11 developer desktop (release profile, single thread, `cargo bench`), October 2026. Absolute numbers vary by machine; compare the *ratios* and how each cost scales with state size. Re-run before and after touching the hot paths below.

## Commands

```bash
# State root: cold rebuild, snapshot, and a block cycle that touches N accounts
cargo bench -p boing-state --bench state_root
# Mempool admission: fast precheck vs. old dry-run vs. full submit_transaction
cargo bench -p boing-node --bench admission
# Both accept BOING_BENCH_ACCOUNTS=1000,10000 BOING_BENCH_ITERS=200 BOING_BENCH_TOUCH=200
```

## Hot paths and what changed

### 1. State root (`boing-state`)

`StateStore::state_root` used to rebuild the sparse Merkle tree from every account on every block, and `snapshot()` deep-cloned the whole account map. The tree is now an arena SMT with per-node hash caching: a block only re-hashes the paths of touched accounts, and snapshots share the arena. The root is **bit-identical** to the old definition (depth 64 on the first 64 key bits, single-entry subtree = `H(k||v)`, empty = zero hash); differential tests compare against `state_root_full_rebuild`.

| accounts | block cycle before (us) | after (us) | speed-up | snapshot before (us) | after (us) | speed-up |
|---------:|------------------------:|-----------:|---------:|---------------------:|-----------:|---------:|
| 1,000 | 1,449 | 339 | 4.3x | 368 | 220 | 1.7x |
| 10,000 | 11,133 | 1,183 | 9.4x | 1,986 | 950 | 2.1x |
| 50,000 | 57,776 | 4,404 | 13.1x | 10,522 | 3,835 | 2.7x |

*Block cycle* = apply 200 touched accounts then compute the root; before it was O(total accounts), now O(touched x depth).

Trade-off: a **cold** root from scratch (node start / full replay) is slightly slower because it also builds the cache: 1.2 -> 1.1 ms (1k), 9.0 -> 9.6 ms (10k), 50.8 -> 58.8 ms (50k). This happens once per process, not per block.

### 2. Mempool admission (`boing-node`)

`submit_transaction` used to snapshot all state and run the VM to check that the sender can afford the fee, so cost grew with total accounts and an attacker could burn CPU with cheap invalid submits. Admission is now: signature check -> duplicate check -> O(1) account gates (sender exists, nonce not stale, balance covers the failed-tx fee) -> activation checks -> fee affordability check without a full state snapshot.

| accounts | fast account gates (us) | old snapshot + VM dry run (us) | full `submit_transaction` (us) |
|---------:|------------------------:|-------------------------------:|-------------------------------:|
| 1,000 | 0.1 | 157.5 | 66.5 |
| 10,000 | 0.2 | 1,031.8 | 66.1 |
| 50,000 | 0.5 | 4,781.3 | 65.7 |

A transfer submit is now flat at about 66 us regardless of state size (dominated by Ed25519 verification), versus 4.8 ms at 50k accounts before. Stale-nonce / unknown-sender / unfunded submits are rejected by the O(1) gates without a state snapshot or VM run (after the signature check).

### 3. Block building and gossip

- `Mempool::drain_for_block` is a heap-based, nonce-aware selection: highest tip first among each sender's next-ready transaction, never skipping a nonce. (Previously a high-tip tx with a future nonce could be included before its predecessor and fail.)
- The transaction gossip handler takes a read lock and verifies the signature once.

## Known remaining hot spots (not changed)

- `node.receipts` grows without bound and native DEX discovery scans it whole.
- `top_stakers` sorts all accounts; `prove_storage` / `storage_root_of` scan all contract storage; `prove` re-sorts all leaves.
- `boing_getBlobDasSample` / `boing_getBlobDasLightClientBatch` re-read and re-chunk the whole blob per call.
- Gossipsub uses the default 64 KiB transmit limit; large blocks may not propagate (needs a measured change with a testnet soak).
