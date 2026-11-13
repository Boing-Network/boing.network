# Stake-mode liveness slash (H2) — close path shipped

**Status:** **Shipped** as activation-gated consensus-ordered `ReportLiveness` (wire index **40**).  
**Default:** `BOING_LIVENESS_SLASH_ACTIVATION_HEIGHT` unset / `never` / `off` → **never admitted** (safe for single-validator / open-jail soak).  
**Gate:** Mainnet readiness residual **H2** — **Closed** as the real stake-slash path (2026-11-12 Nico direction).

## What is live today

| Mode | Behavior | Stake burn? |
|------|----------|-------------|
| **Single-validator** | `tick_liveness` no-ops (`num_validators <= 1`) | No |
| **Open-validator** | Deterministic jail via `StateStore::apply_open_validator_block_hook` | No (jail only) |
| **Stake / static multi-validator** | `tick_liveness` wall-clock streak → diagnostics; when activation height is live **and** quorum size is 1 (`n ≤ 2`), queues `ReportLiveness` | **Yes**, only via block execution of `ReportLiveness` after activation |

Tokenomics: `LIVENESS_SLASH_BPS` (10%) and `slash_liveness_stake` (covers `stake + pending_unbond`, escrow + appeals). Call site: VM `execute_report_liveness` ← `TransactionPayload::ReportLiveness`.

Regression: `cargo test -p boing-node --test liveness_slash` (leader timeout must not mutate committed stake outside execution) and `cargo test -p boing-node --test liveness_report_slash`.

## Evidence semantics (shipped)

**Kind B-lite — quorum miss attestations** (deterministic crypto; no wall-clock in the state root):

1. Attesters sign `BLAKE3(domain || height_le || offender || validator_count_le)` (`boing.liveness.miss.v1`).
2. Evidence needs `2f+1` distinct attesters with `f = (n−1)/3` (`n = validator_count` in the evidence).
3. Execution verifies signatures, burns via `slash_liveness_stake`, seeds `ConsensusSlashReason::Liveness` (idempotent per `(validator, height)`).
4. Mempool checks activation, evidence verify, offender ∈ set, and `validator_count == consensus.num_validators()`.

`tick_liveness` still uses a local streak ≥ `LIVENESS_MISS_THRESHOLD` before acting. Auto-queue of a report works when quorum size is 1 (`n ≤ 2`). Larger sets need peer attestation collection (follow-up); the execute path already accepts multi-attestation evidence in tests.

**Do not** re-enable `slash_liveness_stake` from `tick_liveness` itself.

## Activation (coordinated)

```bash
# Example only — replace H well above tip; every validator/RPC node must agree
export BOING_LIVENESS_SLASH_ACTIVATION_HEIGHT=H
# or: --liveness-slash-activation-height H
```

Discovery: `boing_health` / `boing_getNetworkInfo` publish `liveness_slash_activation_height` (`null` = never).

Same rollout discipline as [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md).

## Public testnet

Leave the env **unset** until an explicit soak decision. Open-mode jail remains the live liveness control on chain 6913.

## Refs

- Audit H2: Project store `docs/boing-network-protocol-audit.md`
- Code: `crates/boing-primitives/src/liveness.rs`, `crates/boing-execution` `execute_report_liveness`, `Node::tick_liveness` / `queue_liveness_report`
- Equivocation pattern: `crates/boing-node/tests/equivocation_slash.rs`
