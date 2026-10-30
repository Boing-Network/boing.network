# Account-tree v3 (full-key routing of the account tree) — activation notes

**Status:** implemented, **OFF by default**. Nothing changes on any chain until every node is started
with the same `BOING_ACCOUNT_TREE_V3_ACTIVATION_HEIGHT` (or `--account-tree-v3-activation-height`).
Audit #61 item 30.

## The problem

The global state tree (accounts, QA pending records, registry commitments) routes on only the **first 64
bits** of the 32-byte key, like the legacy storage trees ([STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md)).
Two account ids that share their first 8 bytes land in one bucket and **only the smallest id is
committed**: changes to the other account do not move the state root and its account proof never
verifies. Account ids are public keys or hashes, so a collision is not accidental, but an attacker can
grind a key whose first 64 bits match a target account (about 2^64 work for a chosen target, 2^32 for any
pair), which hides that account's state from light clients and proofs.

## The fix

From the activation height `H` the account tree routes on **all 256 key bits** (`DEEP_MAX_DEPTH`).
Hashing rules are unchanged (empty subtree `Hash::ZERO`, single entry `BLAKE3(key ‖ value_hash)`, inner
node `BLAKE3(left ‖ right)`), and proofs keep the same format: `MerkleProof` carries one step per level,
so existing verifiers work at any depth.

- Below `H` every root is bit-for-bit the legacy root (`crates/boing-state/tests/account_tree_v3.rs`,
  `crates/boing-node/tests/testnet_replay.rs`).
- At `H` the node rebuilds the account tree once at the new depth (O(state)); after that updates stay
  incremental. A rewind below `H` switches back.
- The gate is independent of storage-tree v2; they can activate at the same height.

## Activation

1. Release a build with this change and pick `H` well above the current tip.
2. Every validator and RPC node sets `BOING_ACCOUNT_TREE_V3_ACTIVATION_HEIGHT=H` before the chain reaches
   `H`. A node without it computes a different state root at the first block from `H` on and stops
   syncing.
3. Once reached, `H` is consensus history: never change or unset it on that chain.
