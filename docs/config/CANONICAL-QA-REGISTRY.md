# Canonical QA configuration (reference)

This folder holds **reference JSON** that matches the **default governance shapes** in the Boing node and `boing-qa` crate. Use it for documentation, diffs, and tooling—not as a substitute for checking what a **live** node actually runs.

## Files

| File | Meaning |
|------|--------|
| [`qa_registry.canonical.json`](qa_registry.canonical.json) | Default **rule registry** (`RuleRegistry::new()`): bytecode size cap (32 KiB), empty blocklists and pattern lists. Live nodes may differ after governance or operator updates. |
| [`qa_pool_config.canonical.json`](qa_pool_config.canonical.json) | Default **production-style pool governance** (`QaPoolGovernanceConfig::production_default()`): bounded queue, admin-only voting when administrators are set, 7-day review window. |

## Live vs canonical

- **Canonical JSON** = baseline shipped in docs for transparency and CI-style comparison.
- **Live policy** = whatever the node loaded from disk or applied via `boing_operatorApplyQaPolicy` / CLI / hub.

Anyone can read the **effective** rule registry from a node with no authentication:

- JSON-RPC method **`boing_getQaRegistry`** (no params) — returns the same JSON shape as `qa_registry.json` on disk.

Pool parameters are already exposed as **`boing_qaPoolConfig`** (summary fields). The full pool governance JSON file shape matches [`qa_pool_config.canonical.json`](qa_pool_config.canonical.json) when defaults apply.

**Smoke (public RPC):** `npm run verify-public-testnet-rpc` and `npm run verify-qa-alignment` from the monorepo root (also in daily CI — `.github/workflows/canonical-pool-public-rpc.yml`).

## Links

- [RPC-API-SPEC.md](../RPC-API-SPEC.md) — `boing_getQaRegistry`, pool methods, error codes.
- [QUALITY-ASSURANCE-NETWORK.md](../QUALITY-ASSURANCE-NETWORK.md) — protocol QA behavior and governance.
- **Boing Observer** — [QA transparency](https://boing.observer/qa) shows live pool status and registry JSON from RPC.

## Raw URLs (stable paths on `main`)

Replace org/repo if you mirror the monorepo:

- `https://raw.githubusercontent.com/boing-network/boing.network/main/docs/config/qa_registry.canonical.json`
- `https://raw.githubusercontent.com/boing-network/boing.network/main/docs/config/qa_pool_config.canonical.json`
