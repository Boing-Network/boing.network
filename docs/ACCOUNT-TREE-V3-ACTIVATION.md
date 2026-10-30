# Account-tree v3 (full-key routing of the account tree) — activation notes

**Status:** implemented. Default remains **OFF** (mainnet unset). **Testnet 6913 crossed at H = 1500**
(preset from testnet-v0.1.21+; public tip past 1506 on 2026-10-30 with
`account_tree_v3_activation_height: 1500`). Audit #61 item 30.

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
- **Testnet 6913: H = 1500, baked into the preset** (testnet-v0.1.21+), the same block as storage-tree v2 and
  VM hardening; one full state-root rebuild covers both trees. The env var / flag still overrides the preset.
  Crossing is tested in `replay_live_chain_then_cross_tree_and_vm_gates_locally` and (with a full dump)
  `replay_full_live_chain_then_cross_preset_gates`.

### Testnet crossing record (chain 6913)

Observed on the public RPC (`https://testnet-rpc.boing.network/`) on 2026-10-30:

| Height | `state_root` |
|---|---|
| 1499 (legacy) | `0x03e1e55794f8dbfdbca67061a668c12ae3c2ef97ff94b68684fb1aec3c0b93d7` |
| 1500 (v2+v3) | `0xca7dd351852e0d74b657212d4cbf52a9dd916e171c41a6b1055b379df38c8131` |
| 1506 | `0x88d18b7ec7de3170095e9974092b8fdfbf42068cdb4196bde74e4041077a7b71` |

`boing_getNetworkInfo` reported `storage_tree_v2_activation_height: 1500` and
`account_tree_v3_activation_height: 1500`. The root change at 1500 is expected (one-time rebuild at full
depth). Do not unset or lower `H` on any node that follows this chain.

## Activation (new networks / mainnet)

1. Release a build with this change and pick `H` well above the current tip.
2. Every validator and RPC node sets `BOING_ACCOUNT_TREE_V3_ACTIVATION_HEIGHT=H` before the chain reaches
   `H` (or bake it into the network preset, as testnet did). A node without it computes a different state
   root at the first block from `H` on and stops syncing.
3. Once reached, `H` is consensus history: never change or unset it on that chain.
4. Mainnet stays unset until the soak gate in [STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md#mainnet-gate)
   is met (same clock as storage-tree v2 when they share a height).
