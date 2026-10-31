# Governance consensus transactions — activation plan (audit #61 item 41)

**Status:** library quorum + operator RPC votes shipped; **wire `TransactionPayload` variants not
yet appended**. Default admission is **off** forever until a coordinated height is chosen.

## What already works (node-local)

| Surface | Behavior |
|---------|----------|
| `boing_governancePropose` | Allowlisted `qa_registry` / `qa_pool_config` / `network_fee_config` |
| `boing_governanceVote` | Stake-weighted ballot (`weight` = bonded stake, or override) |
| `boing_governanceAdvanceCooling` | Requires ⌈2/3⌉ approve of cast weight when `strict_quorum` (default on new nodes) |
| Persistence | `governance.json` (not in the account-tree state root) |

New [`Governance`](../crates/boing-governance) engines set `strict_quorum = true`. Legacy
`governance.json` files without the field deserialize with `strict_quorum = false` so existing
operator-attested deployments keep working until operators flip the flag.

## Why consensus txs are still gated

Votes and proposals that live only in `governance.json` are **not** shared by HotStuff commit.
Two honest validators can disagree about ballots and reject each other’s cooling advances. The fix
is the same pattern as slash appeals: append consensus payloads, apply them in block execution,
and commit a registry into the state root.

## Planned wire indices (append-only)

After `ReportEquivocation` (36):

| Index | Name | Body (planned) |
|------:|------|----------------|
| 37 | `GovernancePropose` | `target_key: String`, `target_value: Vec<u8>` |
| 38 | `GovernanceVote` | `proposal_id: u64`, `approve: bool` (weight = sender bonded stake at cast) |
| 39 | `GovernanceAdvanceCooling` | `proposal_id: u64` (succeeds only if on-chain quorum met) |

Shapes and activation helpers live in
`crates/boing-governance/src/consensus_tx.rs`. **Do not reorder** once shipped.

## Activation

1. Ship a release that appends indices 37–39, applies them in `BlockExecutor` / `Vm` into a
   StateStore-committed governance registry, and mirrors apply into fee/QA setters on execute.
2. Every validator sets **`BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT=H`** (or a network
   preset) **before** height `H`. Unset / `never` / `off` keeps txs rejected.
3. At `H`, mempool admits the new payloads; below `H` they fail closed with a clear error
   (`consensus_tx::not_yet_admitted_message`).
4. After activation, operator RPC propose/vote/cooling remain as **operator convenience** or are
   deprecated in favor of signed txs — product choice at go-live. `strict_quorum` must be `true`
   network-wide.

## Out of scope until wire landing

- Moving cooling timers from wall-clock `SystemTime` to block-height clocks
- BLS / attested ballots for governance (slash-appeal Phase 18 pattern can be reused later)
- Automatic migration of in-flight `governance.json` proposals into the committed registry

## Refs

- Audit: https://github.com/Boing-Network/boing.network/issues/61 (item 41)
- Quorum RPC wiring: prior [#149](https://github.com/Boing-Network/boing.network/pull/149); this
  batch adds vote + strict quorum
- Similar activation docs: [ACCOUNT-TREE-V3-ACTIVATION.md](ACCOUNT-TREE-V3-ACTIVATION.md)
