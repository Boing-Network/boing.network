# Stake-mode liveness slash (H2) — residual + smallest patch path

**Status:** **Not shipped** as consensus-ordered slash. Diagnostics-only in stake mode.  
**Gate:** Mainnet readiness residual **H2** ([MAINNET-READINESS.md](MAINNET-READINESS.md)).  
**Do not flip on without a product decision** on evidence semantics (and, for mainnet, whether day-0 accepts open-jail / single-validator instead).

## What is live today

| Mode | Behavior | Stake burn? |
|------|----------|-------------|
| **Single-validator** | `tick_liveness` no-ops (`num_validators <= 1`) | No |
| **Open-validator** | Deterministic jail via `StateStore::apply_open_validator_block_hook` (miss window / threshold on produced blocks) | No (jail only) |
| **Stake / static multi-validator** | `Node::tick_liveness` wall-clock leader-timeout streak → telemetry only | **No** — deliberately |

Tokenomics already defines `LIVENESS_SLASH_BPS` (10%) and `slash_liveness_stake` (covers `stake + pending_unbond`, escrow + appeals). **There is no call site** from block execution. Equivocation is the live consensus slash path (`ReportEquivocation`, activation-gated).

Regression that must keep passing: `cargo test -p boing-node --test liveness_slash` (leader timeout must not mutate committed stake or the slash book).

## Why wall-clock slash was removed

Node-local leader timeouts previously burned stake outside block execution. Peers that disagreed on “missed” vs “slow” diverged on **state roots**. That is unsafe for any multi-validator network. Re-enabling `slash_liveness_stake` from `tick_liveness` is **out of scope and must not ship**.

## Exact blocker (product / protocol)

Closing H2 requires a **consensus-ordered** liveness evidence path that every honest node verifies identically, then applies `slash_liveness_stake` during tx execution (mirror `ReportEquivocation`).

Unresolved design choices (Nico / protocol):

1. **Evidence kind** — What proves a miss without wall clock?
   - **A. Produced-block miss counters** (extend open-mode style into stake mode): leader scheduled for height `h` but another proposer’s block at `h` (or pacemaker skip) records a miss in the state root. Deterministic; needs clear rules for view-change / abandoned proposals.
   - **B. Quorum attestations** — `2f+1` signed “no proposal seen by deadline” messages. Stronger; heavier gossip + new evidence type; still needs a shared deadline definition.
2. **Wire index** — Append-only `TransactionPayload` after governance **39** (do not reorder). Likely `ReportLiveness { evidence }` calling `slash_liveness_stake`.
3. **Activation** — Env gate (pattern: `BOING_LIVENESS_SLASH_ACTIVATION_HEIGHT`, unset = never), same coordinated rollout as [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md).
4. **Day-0 residual accept** — For **single-validator genesis** or **open-validator jail-only**, Nico may accept H2 as residual in writing without shipping stake-mode burn. That closes the *gate* without the tx.

## Smallest safe patch path (when approved)

Order of work once evidence kind is chosen (estimate by invasiveness, not calendar):

1. Spec evidence bytes + verification in `boing-primitives` (golden vectors).
2. Append `ReportLiveness` payload index; execute → `slash_liveness_stake` + slash-appeal seed (reuse FailedServe / equivocation book).
3. Mempool + VM admission gate (`BOING_LIVENESS_SLASH_ACTIVATION_HEIGHT`); publish height on `boing_health` / `boing_getNetworkInfo` like M6 gates.
4. Node: on deterministic miss, queue evidence tx (do **not** slash in `tick_liveness`).
5. Tests: multi-validator apply identical roots; single-validator still no-ops; activation-below-H inert.
6. Docs: FEATURES → gated; NEXT-STEPS; this file → activated; MAINNET-READINESS checkbox.

**Not in scope here:** inventing slash BPS / economics (already in tokenomics); Workers Paid; mainnet genesis.

## Day-0 residual language (copy/paste for Nico)

> Stake-mode liveness slash remains diagnostics-only. Day-0 mainnet (or continued testnet soak) accepts **open-validator jail** and/or **single hosted validator** without consensus-ordered liveness burn. Equivocation slash remains the economic slash path when activated. Full `ReportLiveness` deferred until evidence semantics are chosen.

## Refs

- Audit H2: Project store `docs/boing-network-protocol-audit.md`
- Code: `crates/boing-node/src/node.rs` (`tick_liveness`), `crates/boing-tokenomics` (`slash_liveness_stake`)
- Open jail: `crates/boing-state/src/open_validators.rs`
- Equivocation pattern: `crates/boing-node/tests/equivocation_slash.rs`
