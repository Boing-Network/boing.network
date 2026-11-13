# Blake3 + governance consensus-tx activation (M6) — deferred past genesis

**Status:** Both features are **implemented and gated off forever** until heights are set.  
**Disposition (2026-11-12):** **Closed as deferred past genesis / null activation** — owner **Nico**; revisit before enabling. Do **not** invent activation heights.  
**RPC:** Keep publishing `null` / `null` until Nico names an explicit height.

Companion detail for governance wire indices: [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md).  
Env catalog: [ENVIRONMENT-VARIABLES.md](ENVIRONMENT-VARIABLES.md).

## Accepted close (gate-1 intentionality)

> Blake3 opcode `0x20` / fungible template v3 and governance consensus txs **37–39** remain **inactive** (`blake3_opcode_activation_height` / `governance_consensus_tx_activation_height` = **`null`**) through genesis and beyond until Nico sets an explicit coordinated height. Enabling later is a one-step env flip on every node — not a hard fork of wire layout. Owner: **Nico**. Revisit before any enablement.

BN-155 economics are **orthogonal** — do not invent fee / emission numbers when flipping these gates.

## What stays off by default

| Gate | Env / CLI | Effect when unset / `never` / `off` |
|------|-----------|--------------------------------------|
| Blake3 opcode `0x20` + fungible template **v3** allowance keys | `BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT` / `--blake3-opcode-activation-height` | Opcode absent; v3 deploys invalid |
| Governance consensus txs **37–39** (Propose / Vote / AdvanceCooling) | `BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT` / `--governance-consensus-tx-activation-height` | Mempool + execution reject payloads |

Operator RPC governance (`boing_governancePropose` / …) remains a **node-local** convenience and is **not** in the state root until 37–39 activate.

## Discovery (already wired)

`boing_health` and `boing_getNetworkInfo` publish:

- `blake3_opcode_activation_height` — JSON integer or **`null`** (never)
- `governance_consensus_tx_activation_height` — JSON integer or **`null`** (never)

Verify:

```bash
curl -fsS -A boing-sdk/json-rpc -X POST "$BOING_RPC_URL" \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_getNetworkInfo","params":[]}' \
  | jq '{blake3_opcode_activation_height, governance_consensus_tx_activation_height, head_height, chain_id}'
```

Expect **`null` / `null`** on public testnet and at genesis under this deferral.

## One-step decision (only when Nico says go)

Pick **`H` well above the live tip**. Then on **every** validator and RPC node of that chain, **before** height `H`:

```bash
# Example only — replace H; never commit secrets; do not invent H in this doc
export BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT=H
export BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT=H   # optional: same or later H
```

You may activate Blake3 and governance **independently**. Recommended: Blake3 first on testnet soak, then governance consensus txs.

After tip ≥ `H`:

1. Re-query network info — both fields show the configured integers.
2. Blake3: deploy / call a fixture that uses opcode `0x20`.
3. Governance: submit signed txs 37→38→39 per [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md).
4. Never unset or lower `H` on that chain after crossing.

## Code hooks (already present)

- Resolve: `crates/boing-node/src/media_blob_config.rs` (`resolve_blake3_opcode_activation`, `resolve_governance_consensus_tx_activation`)
- Enable: `BoingNode::enable_blake3_opcode`, `enable_governance_consensus_txs`
- RPC publish: `m6_activation_gates_json` in `crates/boing-node/src/rpc.rs`
- Regression: `cargo test -p boing-node m6_activation_gates_default_null`

## Out of scope (still deferred)

- AdvanceExecution / Execute as consensus wire
- BLS / attested governance ballots
- Auto-migration of `governance.json` into the committed registry
