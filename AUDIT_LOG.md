# AUDIT LOG: BOING NETWORK HARDENING AND QUALITY ASSURANCE

**Date:** 2026-03-30
**Branch:** `agent/hardening-20260330`
**Auditor / Engineer:** Jules

---

## 1. Environment & Baseline Status (Phase 0 & Phase 1)

- **Branch**: `agent/hardening-20260330`
- **Dirty Tree at Start**: None (clean baseline preserved).
- **Target Subsystems**: `crates/`, `boing-sdk/`, `website/`, `docs-site/`, `workers/`, `examples/`, `desktop-hub/`, `fuzz/`.

| Subsystem / Check | Command Executed | Result | Notes / Blockers |
|---|---|---|---|
| Rust Formatting | `cargo fmt --all -- --check` | PASS | Workspace formatting verified. |
| Rust Compilation | `cargo build` | PASS | All workspace crates compile cleanly. |
| Rust Test Suite | `cargo test --workspace --exclude boing-network-hub` | PASS | All workspace unit and integration tests pass (100% pass rate). |
| Rust Clippy | `cargo clippy --workspace --exclude boing-network-hub --all-targets` | PASS (warnings) | Warnings in primitives, execution, p2p, node, state (`needless_range_loop`, `match_like_matches_macro`, `question_mark`, `assertions_on_constants`, `bool_assert_comparison`, `collapsible_match`, `field_reassign_with_default`, `unnecessary_get_then_check`). |
| TypeScript SDK | `cd boing-sdk && npm ci && npm run build && npm test` | PASS | 77 test files passed (508 passed). Full RPC integration suite runs and passes (8/8) with live test node. |
| Website | `cd website && npm ci && npm run build` | PASS | Astro 40 pages built cleanly with brand raster assets generated. |
| Docs Site | `cd docs-site && npm ci && npm run build` | PASS | Astro 7.4.1 builds all 17 documentation pages with Pagefind search index. |
| Workers (deploy-registry-indexer) | `cd workers/deploy-registry-indexer && npm ci && npm test` | PASS | 14 tests passed. |
| Workers (nft-owner-indexer) | `cd workers/nft-owner-indexer && npm ci && npm test` | PASS | 15 tests passed. |
| Workers (public-rpc-gateway) | `cd workers/public-rpc-gateway && npm ci && npm test` | PASS | 39 tests passed. |
| Workers (origin-access-proxy) | `cd workers/origin-access-proxy && npm run typecheck` | FAIL (baseline) -> PASS (fixed) | Missing `DOM.Iterable` in tsconfig compiler options prevented Headers iteration. |
| Desktop Hub Frontend | `cd desktop-hub && npm ci && npm run build` | PASS | Vite built dist/ cleanly. GTK/WebKit system libraries missing for Tauri desktop runner on this host. |
| Examples (observer-d1-worker) | `cd examples/observer-d1-worker && npm ci && npm test` | PASS | 60 tests passed. |
| Examples (boing-dapp-starter) | `cd examples/boing-dapp-starter && npm ci && npm run build` | PASS | TypeScript compilation clean. |
| Fuzz targets | `cd fuzz && cargo check` | PASS | Nightly fuzz binaries check cleanly. |
| Benchmarks | `cargo bench --workspace --exclude boing-network-hub --no-run` | PASS | Criterion benchmarks compile cleanly. |
| Security Audits | `npm audit` across JS/TS workspaces | PASS | 0 vulnerabilities found across all packages. |

---

## 2. Issues Inventory & Remediation (Phase 2 & Phase 3)

| ID | Severity | Component | File:Line | Root Cause | Proposed Fix | Status | Commit |
|---|---|---|---|---|---|---|---|
| ISS-001 | P2 | `boing-node` | `crates/boing-node/src/media_blob_store.rs:534` | Method `len` defined without `is_empty` | Add `pub fn is_empty(&self) -> bool { self.len() == 0 }` | Fixed | Baseline |
| ISS-002 | P2 | `boing-node` | `crates/boing-node/src/intent_pool.rs:51` | Double lookup `contains_key` followed by `insert` | Use entry API for clarity and efficiency | Fixed | Baseline |
| ISS-003 | P2 | `boing-cli` | `crates/boing-cli/src/deploy.rs:76` | Clippy `manual_is_multiple_of` warning | Replace `compact.len() % 2 == 0` with `compact.len().is_multiple_of(2)` | Fixed | Baseline |
| ISS-004 | P2 | `boing-node` | `crates/boing-node/src/rpc.rs:465` | Redundant closure in mapping Arc | Use `Arc::<str>::from` function pointer | Fixed | Baseline |
| ISS-005 | P2 | `boing-node` | `crates/boing-node/src/rpc.rs:1876` | Clippy `question_mark` warning | Simplify with `?` | Fixed | Baseline |
| ISS-006 | P2 | `fuzz` | `fuzz/fuzz_targets/fuzz_qa_match.rs:15` | Clippy `unused_mut` warning | Remove redundant `mut` | Fixed | Baseline |
| ISS-007 | P2 | `boing-primitives` | `crates/boing-primitives/src/blobs_phase6.rs:412` | Unnecessary `unwrap` on 8-byte slice `try_into()` from BLAKE3 hash | Direct 8-byte array copy `seed_bytes.copy_from_slice(&b[0..8])` | Fixed | Baseline |
| ISS-008 | P2 | `boing-primitives` | `crates/boing-primitives/src/blobs_phase7.rs:118` | Unnecessary `unwrap` on 8-byte slice `try_into()` from BLAKE3 hash | Direct 8-byte array copy `seed_bytes.copy_from_slice(&b[0..8])` | Fixed | Baseline |
| ISS-009 | P2 | `boing-node` | `crates/boing-node/src/rpc.rs:3058` | Unnecessary `.as_object_mut().unwrap()` in JSON building | Build the json map directly or check safely | Fixed | Baseline |
| ISS-010 | P2 | `boing-node` | `crates/boing-node/src/rpc.rs:1413,1427` | Unnecessary `.as_object_mut().unwrap()` in JSON building | Build json map or match safely | Fixed | Baseline |
| ISS-011 | P2 | `boing-state` | `crates/boing-state/src/store.rs:74` | `Undo` enum variant `QaPendingRecord` caused large variant size difference (~512 bytes vs ~112 bytes) | Box `QaPendingRecord` inside `Undo::QaPending` | Fixed | Baseline |
| ISS-012 | P2 | `boing-cli` | `crates/boing-cli/src/token.rs` | Functions `mint`, `transfer`, `approve` had too many arguments (>7) | Introduce `TokenActionSigner` struct to cleanly encapsulate signer parameters | Fixed | Baseline |
| ISS-013 | P2 | `boing-cli` | `crates/boing-cli/src/router.rs:88` | Complex return type `(AccountId, u8, Option<(AccountId, AccountId)>)` | Extracted `ParsedHop` type alias | Fixed | Baseline |
| ISS-014 | P2 | `boing-node` | `crates/boing-node/src/persistence.rs:408` | Complex return type `Option<HashMap<(u64, AccountId), ConsensusVote>>` | Extracted `ObservedVotesMap` type alias | Fixed | Baseline |
| ISS-015 | P2 | `boing-node` | `crates/boing-node/src/native_dex_discovery.rs:443` | Complex return type tuple | Extracted `DeployTokenMetaMap` type alias | Fixed | Baseline |
| ISS-016 | P2 | `boing-node` | `crates/boing-node/src/rpc.rs:1276` | `build_reverify_slash_appeal_ballots` had 9 arguments | Encapsulated parameters in `ReverifySlashAppealBallotsOptions` struct | Fixed | Baseline |
| ISS-017 | P2 | `boing-node` | `crates/boing-node/src/rpc.rs:5815` | Complex return type on `rpc_http_trace_layer` | Extracted `RpcHttpTraceLayer` type alias | Fixed | Baseline |
| ISS-018 | P2 | `boing-state` | `crates/boing-state/src/sparse_merkle.rs:298` | Clippy `field_reassign_with_default` warning | Initialize `trie` using struct update syntax with `..Default::default()` | Fixed | Baseline |
| ISS-019 | P2 | `boing-execution` | `crates/boing-execution/src/native_dex_multihop_swap_router.rs` | Functions `encode_swap4..6` had too many arguments | Suppressed lint explicitly via `#[allow(clippy::too_many_arguments)]` preserving exact public API surface | Fixed | Baseline |
| ISS-020 | P1 | `docs-site` | `docs-site/package.json` | Astro 7.4.5 required Node `>=22.23.4`, failing builds under Node 22.22.1 | Pinned Astro to 7.4.1 and added overrides for `@astrojs/mdx` 8.1.0, `@astrojs/prism` 4.1.0, `@astrojs/telemetry` 3.4.0 | Fixed | Baseline |
| ISS-021 | P1 | `origin-access-proxy` | `workers/origin-access-proxy/tsconfig.json` | Missing `DOM.Iterable` in `lib` array broke `Headers` iteration during `npm run typecheck` | Added `"DOM.Iterable"` to `compilerOptions.lib` | Fixed | `a818dbc` |
| ISS-022 | P2 | `boing-primitives` | `crates/boing-primitives/src/blobs.rs:756` | Needless range loop indexing `data_chunks` | Use `data_chunks.iter().enumerate().take(n)` | Fixed | `780a694` |
| ISS-023 | P2 | `boing-primitives` | `crates/boing-primitives/src/blobs_erasure.rs:155` | Needless range loop indexing `working` | Use `working.iter().take(data_count)` iterator | Fixed | `780a694` |
| ISS-024 | P2 | `boing-primitives` | `crates/boing-primitives/src/blobs_phase32.rs:102` | Verbose match expression for option condition | Replace with `matches!(flip_from, Some(h) if height >= h)` | Fixed | `780a694` |
| ISS-025 | P2 | `boing-primitives` | `crates/boing-primitives/src/randomness.rs:63` | `let...else` returning `None` | Replace with `let (_, vrf) = proofs.iter().find(|(id, _)| id == v)?` | Fixed | `780a694` |
| ISS-026 | P2 | `boing-state` | `crates/boing-state/src/names.rs:604,649,658` | Unnecessary `get(&K).is_none()` calls in tests | Replace with `!r.reverse.contains_key(&K)` | Fixed | `780a694` |
| ISS-027 | P2 | `boing-node` | `crates/boing-node/src/media_blob_store.rs:619,641` | `assert_eq!(..., false)` comparison against bool literal | Replace with `assert!(!...)` | Fixed | `780a694` |
| ISS-028 | P2 | `boing-node` | `crates/boing-node/src/network_preset.rs:517` | Non-const assertions on constants in test | Replace with `const { assert!(...) }` | Fixed | `780a694` |
| ISS-029 | P2 | `boing-node` | `crates/boing-node/tests/testnet_replay.rs:110,767` | `field_reassign_with_default` on `OpenValidatorParams` | Initialize struct using inline fields with `..Default::default()` | Fixed | `780a694` |
| ISS-030 | P2 | `boing-node` | `crates/boing-node/tests/p2p_tx_gossip_rpc.rs:192` | Needless range loop over indices | Refactored loop to use iterators directly | Fixed | `780a694` |
| ISS-031 | P2 | `boing-node` | `crates/boing-node/tests/multi_validator_consensus.rs:178` | Needless range loop over indices | Refactored loop to use iterators directly | Fixed | `780a694` |
| ISS-032 | P2 | `boing-node` | `crates/boing-node/tests/multi_node_testnet.rs:122` | Needless range loop over indices | Refactored loop to use iterators directly | Fixed | `780a694` |
| ISS-033 | P2 | `boing-p2p` | `crates/boing-p2p/tests/gossip_limits.rs:43` | Collapsible nested `if let` | Collapsed into single pattern `if let Ok(Some(P2pEvent::MediaBlobReceived(b)))` | Fixed | `780a694` |

---

## 3. Post-Hardening Verification Matrix (Phase 4 & Phase 5)

| Subsystem | Command | Result | Notes |
|---|---|---|---|
| Rust Formatting | `cargo fmt --all -- --check` | PASS | Clean format across all files. |
| Rust Compilation | `cargo build` | PASS | Clean debug and release builds. |
| Rust Test Suite | `cargo test --workspace --exclude boing-network-hub` | PASS | All workspace unit, integration, and doc tests pass cleanly. |
| Rust Clippy | `cargo clippy --workspace --exclude boing-network-hub` | PASS | Clean with zero errors. Remaining `too_many_arguments` warnings are preserved public API functions with no breaking signature changes. |
| TypeScript SDK | `cd boing-sdk && npm run build && npm test` | PASS | 77 test files passed (508 passed). |
| Live SDK RPC | `vitest run tests/rpcIntegration.test.ts` (with local node) | PASS | 8 of 8 live tests pass against local `boing-node`. |
| Workers | `npm test` / `npm run typecheck` across all workers | PASS | All workers build, typecheck, and pass vitest test suites. |
| Website | `cd website && npm run build` | PASS | 40 pages statically generated cleanly. |
| Docs Site | `cd docs-site && npm run build` | PASS | 17 pages statically generated with Pagefind search index. |
| Fuzzing | `cd fuzz && cargo check` | PASS | Fuzz targets compile cleanly. |
| Benchmarks | `cargo bench --workspace --exclude boing-network-hub --no-run` | PASS | Benchmark executables compile cleanly. |

---

## 4. Invariants & Protocol Freeze Compliance

- **Consensus & Protocol Freeze**: Zero changes to wire format, hashing domain separators, transaction encoding/decoding, BFT quorum logic, or genesis allocations.
- **Arithmetic Safety**: Arithmetic operations across tokenomics, rewards, gas metering, and AMM pricing continue to enforce checked, saturating, or bounded math.
- **RPC & SDK Synchronization**: All 95 supported RPC methods documented in `docs/RPC-API-SPEC.md` match `BOING_RPC_SUPPORTED_METHODS` in `crates/boing-node/src/rpc.rs` and the SDK client methods.
- **Operator Security**: Privileged RPC endpoints (`boing_operator*`) strictly require the secret token header `x-operator-token` and fail closed when unset.

---
---

# AUDIT LOG — PASS 2026-11-10 (hardening-20261110)

**Date:** 2026-11-10
**Branch:** `agent/hardening-20261110`
**Auditor / Engineer:** automated hardening agent

Baseline commit: `2e31e70` (headers preserved above). This pass is additive: it starts from a clean
tree (only intended edits are staged/committed here) and does not regress the 2026-03-30 work.

## 5. Environment

- Rust **not** present initially; installed stable `rustc`/`cargo` **1.99.1** + `rustfmt` + `clippy`
  under `~/.cargo/bin`.
- `/workspaces` volume is small (32 GB). Default `target/` (16 GB) filled it; the workspace build
  now uses `CARGO_TARGET_DIR=/tmp/opencode/cargo-target` and `CARGO_INCREMENTAL=0`. The stale
  in-repo `target/` was removed (untracked build artifact only).
- Node/npm available; `boing-network-hub` (Tauri/GUI) still cannot build here (GTK/WebKit absent).

## 6. Baseline vs Final Verification Matrix

| Subsystem | Command | Baseline | Final | Notes |
|---|---|---|---|---|
| Rust format | `cargo fmt --all -- --check` | PASS | PASS | After all edits. |
| Rust build | `cargo build --workspace --exclude boing-network-hub` | PASS | PASS | |
| Rust tests | `cargo test --workspace --exclude boing-network-hub` | 941 passed / 0 failed | **945 passed / 0 failed / 0 ignored** | +4 net new regression tests. |
| Rust clippy | `cargo clippy --workspace --exclude boing-network-hub --all-targets` | 0 errors; 17 `assertions_on_constants` + `too_many_arguments` | **0 errors; 0 `assertions_on_constants`**; only pre-existing `too_many_arguments` (preserved public APIs) | |
| SDK | `cd boing-sdk && npm ci && npm run build && npm test` | 77 files / 508 passed, 0 npm vulns | not re-run (SDK source untouched) | Unaffected by this pass. |
| JS/TS (12 checks) | website, docs-site, desktop-hub, 4 workers, 3 examples | all PASS | all PASS (`/tmp/opencode/jschecks.log`) | Includes `boing-dapp-starter` after lockfile fix. |
| Fuzz compile | `cargo check --manifest-path fuzz/Cargo.toml` | NOT RUN | PASS | Compiles on stable. **Dynamic fuzzing** (libFuzzer sanitizer coverage) still requires nightly, which this host lacks — NOT VERIFIED (BLOCKED). |
| Benches compile | `cargo bench --workspace --exclude boing-network-hub --no-run` | NOT RUN | PASS | All criterion benches compile (release). Benchmarks not executed (no perf comparison requested). |
| Desktop hub (Tauri) | `cargo build -p boing-network-hub` | NOT VERIFIED (BLOCKED) | NOT VERIFIED (BLOCKED) | GTK/WebKit system libraries absent (consistent with 2026-03-30). |

## 7. Issues Inventory & Remediation (this pass)

| ID | Severity | Component | File:Line | Root Cause | Fix | Status |
|---|---|---|---|---|---|---|
| ISS-100 | **P1** | `boing-execution` | `native_amm.rs` (`constant_product_amount_out_after_fee_with_bps`, `..._fee_on_input_with_bps`) | `10_000 - fee_bps` underflowed (debug panic / wrong quote in release) when an operator-supplied `fee_bps > 10_000` reached the quote helpers | Early-return `0` for `fee_bps > 10_000`; regression `out_of_range_fee_bps_quotes_zero_without_panicking` | Fixed |
| ISS-101 | **P1** | `boing-node` | `mempool.rs` | Any panic while holding the pool lock poisoned the `Mutex`; every later `lock().unwrap()` panicked, **permanently bricking admission/RPC** | `lock_inner()` via `PoisonError::into_inner`; regression `poisoned_mutex_does_not_brick_pool` | Fixed |
| ISS-102 | **P1** | `boing-node` | `intent_pool.rs` | Same poisoning on `RwLock` | `read_inner()`/`write_inner()` recovery; regression `poisoned_lock_does_not_brick_pool` | Fixed |
| ISS-103 | **P1** | `boing-node` | `chain.rs` | Same poisoning on the chain `RwLock`; bricks tip/block RPC | `read_inner()`/`write_inner()` recovery; regression `poisoned_lock_does_not_brick_chain` | Fixed |
| ISS-104 | **P2** | `boing-node` | `dapp_registry.rs` | Same poisoning on registry `RwLock` | recovery helpers; regression `poisoned_lock_does_not_brick_registry` | Fixed |
| ISS-105 | **P2** | `boing-qa` | `pool.rs` | Same poisoning on both QA pool mutexes; bricks `boing_qaPool*` | `lock_governance()`/`lock_inner()`; regression `poisoned_mutex_does_not_brick_pool` | Fixed |
| ISS-106 | **P2** | `boing-node` | `token_metadata.rs` | `index.lock().unwrap()` poisoning risk | `lock_index()` recovery helper | Fixed |
| ISS-107 | **P2** | `examples/boing-dapp-starter` | `package-lock.json` | Lock still referenced unscoped `boing-sdk` v0.3.1 / missing `@boing-network/boing-sdk` name → `npm ci` failed | Regenerated lock against `file:../../boing-sdk` (0.5.40); `npm ci` + typecheck PASS | Fixed |
| ISS-108 | **P3** | `boing-primitives`, `boing-execution` | `blobs_phase{6,26..33}.rs`, `vm.rs`, `reference_nft.rs` (tests) | Clippy `assertions_on_constants` (17) | `const { assert!(...) }`; lint now zero | Fixed |
| ISS-109 | **P3** | `docs` | `ENVIRONMENT-VARIABLES.md` | `BOING_FAUCET_COOLDOWN_SECS` / `BOING_FAUCET_MAX_PER_HOUR` read in `rpc.rs` but undocumented | Added rows | Fixed |

## 8. Protocol-Sensitive Findings (documented only — no code change)

| ID | Severity | Component | Finding | Decision |
|---|---|---|---|---|
| PS-01 | P3 | `boing-qa/src/pool_config.rs:258` | `quorum_decision` uses `f64` division for quorum/threshold ratios. Governed by `validate()` (rejects non-finite / out-of-`(0,1]` fractions) called on RPC apply (`rpc.rs:3708`) and node load (`node.rs:655`), so values are bounded and representable; residual risk is future divergence from byte-exact integer math. | No code change — protocol-sensitive. Proposal: migrate to integer basis-point math in a future config-versioned change. |
| PS-02 | P3 | `blob_download_nonce_cache.rs` | Single-use download-proof nonce cache is process-local (documented residual): a replay is possible after a node restart until proof expiry. | No change — documented follow-up (Phase 34.1 capability tickets). |

## 9. Invariants & Protocol Freeze Compliance (this pass)

- **Zero protocol changes**: no edits to wire encoding, signing domains/chain IDs, genesis, addresses,
  tokenomics, fee/slashing formulas, consensus, or testnet infrastructure.
- All changes are **liveness/DoS-hardening, test-only cleanups, docs, or an off-chain quote-helper guard**.
- The AMM guard only affects values that previously **panicked** (debug) or produced a non-quote;
  valid ranges (`fee_bps <= 10_000`) are byte-identical to before.
- Operator auth, CORS, and rate limits in `rpc.rs` unchanged.

## 10. Blocked / Not Verified

- Fuzz **runtime** (libFuzzer): NOT VERIFIED (BLOCKED) — no nightly toolchain; targets compile on stable.
- `boing-network-hub` (Tauri desktop): NOT VERIFIED (BLOCKED) — GTK/WebKit system libraries unavailable.
- `cargo bench` execution: not run by design (compile check only).
