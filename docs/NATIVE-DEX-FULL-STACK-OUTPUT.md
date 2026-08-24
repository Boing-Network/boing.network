# Native DEX full-stack deploy — JSON output reference

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


`npm run deploy-native-dex-full-stack` (from `examples/native-boing-tutorial`) prints **one JSON object** to stdout. Use it as a deployment record and to populate env vars for dApps / follow-up scripts.

**What the orchestrator runs (when phases are not skipped):** bytecode dump → pool + pair directory (`bootstrap-native-pool-and-dex`) → multihop swap2 + ledger routers v2–v3 (optional ledger v1 if `BOING_FULL_STACK_INCLUDE_LEDGER_V1=1`) → LP vault + LP share → share `set_minter_once` + vault `configure` → **kickstart liquidity** (vault `deposit_add`, or direct pool `add_liquidity` if LP deploy was skipped). Run only against an RPC where blocks are produced; it is **not** invoked by `boing-node` on startup.

**Defaults:** if **`BOING_BOOTSTRAP_REGISTER_PAIR`** is **unset**, the orchestrator passes **`1`** into bootstrap (synthetic demo token ids unless **`BOING_DEX_TOKEN_A_HEX` / `BOING_DEX_TOKEN_B_HEX`** are set). Set **`BOING_BOOTSTRAP_REGISTER_PAIR=0`** to skip `register_pair`. Kickstart amounts: **`BOING_KICKSTART_AMOUNT_A` / `BOING_KICKSTART_AMOUNT_B`** (fallback **`1000000` / `2000000`**). Skip seeding: **`BOING_FULL_STACK_SKIP_SEED=1`**.

Frozen **canonical** AccountIds for the default testnet deployer are in [`scripts/canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json) (regenerate with `cargo run -p boing-execution --example print_native_create2_manifest -- <DEPLOYER_HEX>`).

---

## Top level

| Field | Meaning |
|--------|---------|
| **`ok`** | **`true`** if every executed phase exited **0**. |
| **`dotenv`** | Whether **`.env`** was loaded: **`loaded`**, **`path`**, **`appliedPairs`** (keys set from file that were not already in `process.env`). |
| **`phases`** | Per-step payloads (below). Omitted phases are not run (skips). |
| **`warnings`** | Present when the orchestrator detects a **non-canonical** pool/factory topology (see § Warnings). |
| **`summary`** | Short copy of the main AccountIds for wiring and env. |

---

## `phases.preflightFaucet` (when `BOING_AUTO_FAUCET_REQUEST=1`)

| Field | Meaning |
|--------|---------|
| **`ok`** | Faucet RPC succeeded. |
| **`rpc`** | JSON-RPC URL used. |
| **`senderHex`** | Public AccountId derived from **`BOING_SECRET_HEX`**. |
| **`faucet.amount`** | BOING credited (integer, chain units). |
| **`faucet.to`** | Recipient id (may be **without** `0x` prefix — normalize to `0x` + 64 hex when comparing). |
| **`faucet.message`** | Human-readable status from the node. |

---

## `phases.poolFactory` (from `bootstrap-native-pool-and-dex`)

| Field | Meaning |
|--------|---------|
| **`ok`** | Pool + factory steps completed. |
| **`bootstrap.poolRetriedWithNonce`** | **`true`** if pool deploy hit **CREATE2 address occupied** and retried with **`BOING_USE_CREATE2=0`**. |
| **`bootstrap.factoryRetriedWithNonce`** | Same for the pair directory deploy. |
| **`bootstrap.autoNonceOnCollisionDisabled`** | **`true`** if **`BOING_BOOTSTRAP_NO_AUTO_NONCE=1`**. |
| **`pool.senderHex`** / **`dexDirectory.senderHex`** | Deployer AccountId. |
| **`pool.create2`** | **`true`** = CREATE2 pool at predictable id; **`false`** = **nonce-derived** pool (different id every deploy). |
| **`pool.predictedPoolHex`** | Pool contract AccountId to use as **`BOING_POOL_HEX`** / routing. |
| **`pool.tx_hash`** | Submit result from SDK (some stacks return a placeholder like **`ok`** — confirm on-chain if you need the real hash). |
| **`pool.note`** | Explains CREATE2 vs nonce-derived semantics. |
| **`dexDirectory.create2`** | **`true`** = canonical factory slot; **`false`** = nonce-derived directory. |
| **`dexDirectory.predictedFactoryHex`** | Pair directory (DEX factory) AccountId. |
| **`dexDirectory.deploy_tx_hash`** | Factory deploy tx (same caveat as **`pool.tx_hash`**). |
| **`dexDirectory.register_tx_hash`** | Present if **`register_pair`** ran after factory deploy. |
| **`registerPairSubmitted`** | **`true`** iff **`register_tx_hash`** is set. |

---

## `phases.routers` (`deploy-native-dex-aux-contracts`)

| Field | Meaning |
|--------|---------|
| **`ok`** | All non-skipped router steps completed. |
| **`rpc`** | JSON-RPC URL. |
| **`results.swap2MultihopRouter`** | Deploy result for multihop / swap2 router. |
| **`results.ledgerRouterV2`** | Ledger router v2. |
| **`results.ledgerRouterV3`** | Ledger router v3. |
| **`results.*.predictedContractHex`** | Contract AccountId after deploy. |
| **`results.*.create2`** | Usually **`true`** at canonical salts. |
| **`results.*.create2RetriedWithNonce`** | **`true`** if that step fell back to nonce deploy. |
| **`results.*.skipped`** | Step skipped via **`BOING_AUX_SKIP_*`**. |
| **`results.ledgerRouterV1`** | Present only when **`BOING_FULL_STACK_INCLUDE_LEDGER_V1=1`** (or child had **`BOING_AUX_INCLUDE_LEDGER_V1=1`**). |

---

## `phases.lpAux` (`deploy-native-dex-lp-aux-contracts`)

| Field | Meaning |
|--------|---------|
| **`results.ammLpVault`** | LP vault deploy JSON (**`predictedContractHex`** = vault AccountId). |
| **`results.lpShareToken`** | LP share token deploy JSON (**`predictedContractHex`** = share AccountId). |

---

## `phases.lpShareSetMinter` / `phases.lpVaultConfigure`

On-chain wiring after deploys:

| Field | Meaning |
|--------|---------|
| **`action`** | **`set_minter_once`** or **`configure`**. |
| **`senderHex`** | Signer AccountId. |
| **`tx_hash`** | Submit result (same placeholder caveat). |
| **`lastSimulation`** | Final **`boing_simulateTransaction`** result (**`success`**, **`gas_used`**, **`suggested_access_list`**). |
| **`attempts`** | Simulation/submit retry count. |

The vault **`configure`** step binds **`BOING_POOL_HEX`** from **`summary.poolHex`** to the vault + share token you deployed.

---

## `phases.kickstartLiquidity` (vault `deposit_add` or pool `add_liquidity`)

Present when **`BOING_FULL_STACK_SKIP_SEED`** is not set and a **`poolHex`** is known after prior phases (or from **`BOING_WIRE_POOL_HEX`** / **`BOING_POOL_HEX`** when pool factory was skipped).

| Case | Meaning |
|------|---------|
| **Vault path** (default when vault + share hex exist) | JSON from **`native-amm-lp-vault-submit-contract-call`** with **`deposit_add`** — inner **`add_liquidity`** amounts from **`BOING_KICKSTART_AMOUNT_*`** (or **`BOING_AMOUNT_*`**). |
| **Pool-only path** | When LP deploy was skipped: JSON from **`native-amm-submit-contract-call`** with **`add_liquidity`** on **`BOING_POOL_HEX`**. For native AMM **v2** access lists, set **`BOING_TOKEN_A_HEX` / `BOING_TOKEN_B_HEX`** (or **`BOING_DEX_TOKEN_*`** forwarded from env). |
| **`{ "skipped": true }`** | Operator set **`BOING_FULL_STACK_SKIP_SEED=1`**. |

---

## `summary` (convenience)

| Field | Meaning |
|--------|---------|
| **`poolHex`** | Pool passed into vault **`configure`** (from bootstrap). |
| **`vaultHex`** | LP vault AccountId. |
| **`shareHex`** | LP share token AccountId. |
| **`registerPairSubmitted`** | Same as top-level poolFactory flag. |
| **`kickstartLiquidity`** | **`completed`** — seed tx ran; **`skipped`** — **`BOING_FULL_STACK_SKIP_SEED`**, or **`not_run`** — no pool hex / seed phase not applicable. |

---

## Example run (mixed canonical + nonce-derived)

Below is a **realistic shape** after public testnet **CREATE2** slots for **pool** and **factory** are already taken: bootstrap retries with **nonce-derived** pool + directory, while **routers** and **LP** contracts still land on **canonical** CREATE2 ids if those slots are free.

| Component | CREATE2? | Example / canonical id |
|-----------|----------|-------------------------|
| Pool | Often **nonce** (`create2: false`) | e.g. `0x81d357da17a5e1fa1d9527f5f82f25f1d783465f18a7c69e53979f147d93420e` |
| Factory | Often **nonce** | e.g. `0x976ef3e2d1bfc1cdbdad5d7c1344634cc550632b6e41befaa2758621dac9649e` |
| Swap2 / ledger v2 / v3 | **Canonical** if slots free | `0x43a641…`, `0x60a232…`, `0xfb5526…` per manifest |
| LP vault / share | **Canonical** if slots free | `0x2b195b…`, `0x0618b4…` per manifest |

**Nothing is “wrong”** with the script if **`ok: true`**: the vault is configured for **your** nonce-derived pool. **Do** point wallets and routing at **`summary.poolHex`** and **`dexDirectory.predictedFactoryHex`** from **this** JSON — not the manifest pool/factory — unless you intentionally use the global canonical pool that already exists on-chain.

**dApp / RPC hints:** `BOING_CANONICAL_NATIVE_*` env vars on the node describe **published** canonical ids, which may **differ** from your nonce-derived pool/factory. For your own stack, prefer **`fetchNativeDexIntegrationDefaults`** plus overrides from this deployment record.

---

## Warnings array

When **`pool.create2`** or **`dexDirectory.create2`** is **`false`**, the full-stack script adds **`warnings`** to the printed JSON so operators notice **mixed topology** without re-reading nested fields.

---

## Related

- Tutorial README §7c1 — script list and bootstrap field cheat sheet  
- [`DEVNET-OPERATOR-NATIVE-AMM.md`](./DEVNET-OPERATOR-NATIVE-AMM.md) §2 — operator copy/paste for **`deploy-native-dex-full-stack`**  
- [`NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md`](./NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md) — Appendix **C** records a full **`ok: true`** stdout snapshot (mixed CREATE2 / nonce topology, optional v1 / skip flags)  
- [`NATIVE-DEX-FACTORY.md`](./NATIVE-DEX-FACTORY.md), [`NATIVE-AMM-LP-VAULT.md`](./NATIVE-AMM-LP-VAULT.md)  
- [`RUNBOOK.md`](./RUNBOOK.md) — **`BOING_CANONICAL_NATIVE_*`** on `boing-node`
