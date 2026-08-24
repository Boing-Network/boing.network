# Native DEX limits vs EVM — why they exist and what extension costs

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This complements [BOING-NATIVE-DEX-CAPABILITY.md](./BOING-NATIVE-DEX-CAPABILITY.md) and [NATIVE-DEX-FACTORY.md](./NATIVE-DEX-FACTORY.md). It answers **why** several EVM DEX patterns are absent or different, and what it would take to add them.

---

## 1. “No factory deploys pool in one contract call” (Uniswap-style)

**What people want:** One `call` into a factory; a new pair contract appears on-chain in the same execution.

**What Boing ships today:** Each **native CP pool** is deployed with a **`ContractDeploy` / `ContractDeployWithPurpose` transaction** (optional CREATE2 salt). The **pair directory** contract only **`register_pair`**’s existing pool ids.

**Reasons (not arbitrary):**

| Factor | Detail |
|--------|--------|
| **Admission / QA** | New bytecode is reviewed through the **deploy transaction** path (mempool QA, purpose category, allowlists). That boundary is intentional for testnet safety and operator workflows. |
| **Predictability** | Pool templates and CREATE2 salts are **explicit** in tooling (`dump_*`, tutorial scripts, manifest). A generic in-contract deploy loop pushes more policy into runtime bytecode. |
| **VM capability nuance** | The interpreter **does** implement **`CREATE2` (`0xf5`)** for `StateStore` execution (`crates/boing-execution/src/interpreter.rs`, `apply_in_tx_create2`). There is **no separate EVM `CREATE`** opcode; saltless in-contract deploy is not mirrored. A **new** factory program *could* use `CREATE2` to deploy children **if** each child template passes QA — that is **new audited bytecode + governance**, not flipping a flag on the current directory. |

**Benefit of changing:** UX closer to EVM (“one factory method”). **Cost:** New factory VM program, QA rules for child bytecode, migration story, possibly new CREATE2 salts / canonical addresses.

---

## 2. No canonical concentrated-liquidity (Uniswap v3–style) curve

**Reason:** The shipped stack is **constant-product** pools (`constant_product_pool_bytecode` v1–v5). A different curve is a **different VM program** (new math, new storage layout, new QA surface, new SDK encoders, new routing).

**Benefit:** Better capital efficiency for professional LPs, familiar v3 UX. **Cost:** Large protocol + security + tooling program (often comparable to shipping a second DEX).

---

## 3. Multihop router capped at **6** pools per transaction

**Reason:** The multihop router is **fixed bytecode** with explicit selectors for 2- through 6-hop paths (`native_dex_multihop_swap_router.rs`). Layouts, gas, and interpreter **`Call`** depth are bounded up front. See [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](./NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md).

**Benefit of 7+ hops:** Longer paths in one tx (rare in practice; often split across txs or use different topology). **Cost:** **New router bytecode revision** (new selectors / buffer sizes), `boing-sdk` encoders, dump examples, QA re-acceptance, and usually a **new CREATE2 salt + canonical address** (or nonce deploy) — coordinated with operators and `BOING_CANONICAL_NATIVE_*` hints.

---

## 4. Pair directory: no on-chain O(1) `(tokenA, tokenB) → pool`

**Reason:** The factory bytecode avoids a heavy mapping model; discovery is **`Log3`** + **`get_pair_at` scan** (SDK helpers). True O(1) mapping in bytecode typically wants **keccak-style** word hashing or large storage patterns that were not required for v1.

**Benefit:** Cheaper reads in-contract for routers. **Cost:** New storage layout + migration, or precompile / opcode support if you want EVM-style hashing without VM extensions.

---

## 5. TWAP / price oracles

**Reason:** Treated as **application / indexer** concerns unless you add dedicated oracle contracts ([BOING-PATTERN-ORACLE-PRICE-FEEDS.md](./BOING-PATTERN-ORACLE-PRICE-FEEDS.md)).

---

## Practical “what should we build next?”

| Extension | Rough effort / risk | Notes |
|-----------|---------------------|--------|
| **7+ hop router** | Medium — new artifact + SDK + ops | Bounded; breaks “canonical router” unless versioned (v2 router salt). |
| **CREATE2-based pair factory (new contract)** | High — audit + QA policy for children | VM opcode exists; policy and bytecode design dominate. |
| **Concentrated liquidity pool** | Very high | New pool type end-to-end. |
| **O(1) on-chain pair map** | Medium–high | VM/storage design or new opcodes. |

If you want implementation work in-repo, the most **incremental** high-value item is usually **router versioning** (e.g. multihop v2 with more hops) with a clear **migration** doc for operators. Broader EVM parity items belong on the **protocol roadmap** with security review, not as drive-by patches.

---

## Related

- [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](./BOING-VM-CAPABILITY-PARITY-ROADMAP.md) — VM-level parity tracking  
- [NATIVE-DEX-FULL-STACK-OUTPUT.md](./NATIVE-DEX-FULL-STACK-OUTPUT.md) — operator deploy JSON  
- [HANDOFF-DEPENDENT-PROJECTS.md](./HANDOFF-DEPENDENT-PROJECTS.md) — boing.finance / Express alignment
