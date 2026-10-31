# Governance consensus transactions — activation (audit #61 item 41)

**Status:** wire indices **37–39** shipped with StateStore-committed registry. Default
admission is **off** forever until a coordinated height is chosen
(`BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT` unset / `never` / `off`).

## Surfaces

| Surface | Behavior |
|---------|----------|
| `TransactionPayload::GovernancePropose` (37) | Allowlisted `qa_registry` / `qa_pool_config` / `network_fee_config`; commits into state-root registry |
| `TransactionPayload::GovernanceVote` (38) | Stake-weighted ballot (`weight` = sender bonded stake at execution) |
| `TransactionPayload::GovernanceAdvanceCooling` (39) | Requires ⌈2/3⌉ approve of cast weight (`strict_quorum` forced on-chain) |
| Operator RPC (`boing_governancePropose` / `Vote` / …) | Node-local `governance.json` convenience (not in state root) |
| Committed registry | `GovernanceRegistry` SMT leaf (`BOING_GOVERNANCE_REGISTRY_V1`); persisted as `governance_registry.bin` |

## Wire indices (append-only)

After `ReportEquivocation` (36):

| Index | Name | Body |
|------:|------|------|
| 37 | `GovernancePropose` | `target_key: String`, `target_value: Vec<u8>` |
| 38 | `GovernanceVote` | `proposal_id: u64`, `approve: bool` |
| 39 | `GovernanceAdvanceCooling` | `proposal_id: u64` |

Helpers: `crates/boing-governance/src/consensus_tx.rs`. **Do not reorder.**

## Activation

1. Every validator sets **`BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT=H`** (or
   `--governance-consensus-tx-activation-height H`) **before** height `H`.
2. At `H`, mempool admits the new payloads; below `H` they fail closed
   (`consensus_tx::not_yet_admitted_message`).
3. Propose / vote / cooling clocks use the **block timestamp** (not wall clock) so
   validators stay deterministic.
4. Operator RPC propose/vote/cooling remain available as a local convenience; prefer
   signed txs 37–39 after activation so ballots land in the state root.

## Out of scope (still deferred)

- Moving cooling timers from wall-clock / block-timestamp to pure block-height clocks
- BLS / attested ballots for governance
- Automatic migration of in-flight `governance.json` proposals into the committed registry
- Wire indices for `AdvanceExecution` / `Execute` (execute remains operator RPC for now)

## Refs

- Audit: https://github.com/Boing-Network/boing.network/issues/61 (item 41)
- Quorum RPC: [#149](https://github.com/Boing-Network/boing.network/pull/149), [#153](https://github.com/Boing-Network/boing.network/pull/153)
- Similar activation docs: [ACCOUNT-TREE-V3-ACTIVATION.md](ACCOUNT-TREE-V3-ACTIVATION.md)
