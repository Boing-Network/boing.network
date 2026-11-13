# Blake3 + governance consensus-tx activation (M6) — Nico checklist

**Status:** Both features are **implemented and gated off forever** until heights are set.  
**Do not set heights on public testnet or mainnet without an explicit Nico decision.**  
This doc makes acceptance a **one-step env/flag flip** once the height `H` is chosen.

Companion detail for governance wire indices: [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md).  
Env catalog: [ENVIRONMENT-VARIABLES.md](ENVIRONMENT-VARIABLES.md).

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

Verify before and after any flip:

```bash
curl -fsS -A boing-sdk/json-rpc -X POST "$BOING_RPC_URL" \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_getNetworkInfo","params":[]}' \
  | jq '{blake3_opcode_activation_height, governance_consensus_tx_activation_height, head_height, chain_id}'
```

Expect **`null` / `null`** on public testnet until Nico decides otherwise.

## One-step decision (when Nico says go)

Pick **`H` well above the live tip** (same rule as account-tree / storage-tree activations). Then on **every** validator and RPC node of that chain, **before** height `H`:

```bash
# Example only — replace H; never commit secrets
export BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT=H
export BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT=H   # optional: same or later H
# Fly: fly secrets set / [env] in fly.*.toml + rolling restart
```

You may activate Blake3 and governance **independently** (different heights). Recommended: activate Blake3 first on testnet soak, then governance consensus txs.

After tip ≥ `H`:

1. Re-query network info — both fields show the configured integers.
2. Blake3: deploy / call a fixture that uses opcode `0x20` (see execution tests / reference token v3).
3. Governance: submit signed txs 37→38→39 per [GOVERNANCE-CONSENSUS-TX-ACTIVATION.md](GOVERNANCE-CONSENSUS-TX-ACTIVATION.md); confirm registry SMT updates.
4. Never unset or lower `H` on that chain after crossing.

## Mainnet / product residual

Until Nico picks heights (or writes “defer past genesis”):

> M6 remains **gated off**. Mainnet may ship with Blake3 opcode and governance consensus txs **inactive** (`null`). Enabling later is a coordinated activation, not a hard fork of wire layout.

BN-155 economics are **orthogonal** — do not invent fee / emission numbers when flipping these gates.

## Code hooks (already present)

- Resolve: `crates/boing-node/src/media_blob_config.rs` (`resolve_blake3_opcode_activation`, `resolve_governance_consensus_tx_activation`)
- Enable: `BoingNode::enable_blake3_opcode`, `enable_governance_consensus_txs`
- RPC publish: `m6_activation_gates_json` in `crates/boing-node/src/rpc.rs`
- Regression: `cargo test -p boing-node m6_activation_gates_default_null`

## Out of scope (still deferred)

- AdvanceExecution / Execute as consensus wire
- BLS / attested governance ballots
- Auto-migration of `governance.json` into the committed registry
