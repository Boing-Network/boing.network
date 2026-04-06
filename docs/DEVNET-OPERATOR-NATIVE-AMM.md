# Devnet / self-hosted RPC — deploy native AMM pool and seed liquidity

Use this when you run your own **validator + full node** (e.g. **VibeMiner**) with a **public JSON-RPC URL**, you have **devnet BOING** (native balance for fees), and **nothing is deployed yet**. It complements [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) (canonical **public** testnet) and [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md) (browser path).

---

## 1. Align chain ID and RPC everywhere

### How do I know which chain ID my devnet uses?

- **Boing L1 block headers do not contain a chain ID field** — the numbers **`0x1b01` (6913)** testnet and **`0x1b02` (6914)** mainnet are **wallet / dApp conventions** ([THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) §3), not something `boing_getBlockByHeight` exposes today.
- **Default:** If you run **unmodified** `boing-node` from this repo and join the **same network** as public testnet (bootnodes / genesis from [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) / `GET https://boing.network/api/networks`), treat the chain as **6913** unless your operator docs say otherwise.
- **Private / forked genesis:** Whoever published the devnet should state the chain ID they configured in **wallet and dApp builds**. If it is not **6913**, you need **custom** Boing Express + boing.finance (or other dApps) with that ID — hosted apps will not match.
- **Boing Express (web + extension):** The dashboard shows **Chain ID** (hex + decimal) for the selected network — that is what the wallet reports via **`boing_chainId`** to connected sites. RPC override changes the JSON-RPC URL only, not that reported ID.

- **Wallet (Boing Express)** and **boing.finance** must use the **same chain** as your nodes for transactions to be valid. Official public testnet is **`0x1b01` (6913)**. If your genesis uses a **different** chain id, you need **local builds** of the wallet and dApp with that id and your RPC URL — the hosted apps default to public testnet.
- From a shell, confirm your RPC answers:

```bash
export BOING_RPC_URL=https://your-full-node-rpc.example/
cd examples/native-boing-tutorial && npm install && npm run preflight-rpc
```

- **CORS:** Browsers must be allowed to call your RPC from **boing.finance** / **boing.express** origins. See [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) and [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) §2 (same allowlist as public nodes).

---

## 2. What you deploy (pool contract)

The **native constant-product pool** is a normal **Boing VM** contract: bytecode from `boing_execution::native_amm`.

- **v1 (recommended to start):** ledger-only reserves — **`constant_product_pool_bytecode()`**, CREATE2 salt **`NATIVE_CP_POOL_CREATE2_SALT_V1`**. Matches [examples/native-boing-tutorial](../examples/native-boing-tutorial/) **`deploy-native-amm-pool`** defaults.
- **v2 (optional):** adds reference-token **`CALL`** on swap output and remove-liquidity payouts after **`set_tokens`** — different bytecode + **`NATIVE_CP_POOL_CREATE2_SALT_V2`**. See [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md).

**`add_liquidity` does not pull BOING from your reference-token balance** in either version; it only updates **in-pool reserve counters** and your **LP ledger**. Your devnet BOING is still required as **native balance** to pay deploy and call fees.

---

## 3. Operator machine: dump bytecode (one line per file)

From **this repo root** (same commit you trust for production bytecode):

```bash
cargo run -p boing-execution --example dump_native_amm_pool 2>pool-meta.txt 1>pool-lines.hex
```

- **`pool-meta.txt`** — stderr: byte lengths for v1 and v2.
- **`pool-lines.hex`** — stdout: **line 1 = v1**, **line 2 = v2** (each a single `0x…` hex string).

For **v1** deploy, point **`BOING_NATIVE_AMM_BYTECODE_FILE`** at a file that contains **only line 1**, or set **`BOING_NATIVE_AMM_VARIANT=v1`** and pass **`pool-lines.hex`** (the deploy script picks the first line). For **v2**, use **`BOING_NATIVE_AMM_VARIANT=v2`** (second line) or a one-line file.

---

## 4. Deploy the pool (SDK script — needs signing seed)

You need a **funded** Ed25519 account (native BOING) and its **32-byte secret** as hex — **only on your machine**, never in chat or git.

```bash
cd boing-sdk && npm install && npm run build
cd ../examples/native-boing-tutorial && npm install

export BOING_RPC_URL=https://your-full-node-rpc.example/
export BOING_SECRET_HEX=0x<64_hex_chars>
export BOING_NATIVE_AMM_BYTECODE_FILE=/path/to/pool-v1-only.hex
# or: export BOING_NATIVE_AMM_VARIANT=v1
#      export BOING_NATIVE_AMM_BYTECODE_FILE=/path/to/pool-lines.hex

npm run deploy-native-amm-pool
```

JSON stdout includes **`predictedPoolHex`** (CREATE2) and **`tx_hash`**. After the tx is included, treat **`predictedPoolHex`** as the pool **`AccountId`** (verify with **`boing_getContractStorage`** on reserve A key — [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) / **`NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX`** in `boing-sdk`).

Optional sanity check:

```bash
export BOING_POOL_HEX=0x<from_predictedPoolHex>
npm run fetch-native-amm-reserves
```

---

## 5. Seed liquidity (CLI or browser)

**First mint:** call **`add_liquidity`** with positive **`amount_a`** and **`amount_b`** (stay within **u64** range — see [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md)).

**CLI (no browser):** from `examples/native-boing-tutorial`:

```bash
export BOING_RPC_URL=https://your-full-node-rpc.example/
export BOING_SECRET_HEX=0x<same_or_other_funded_signer>
export BOING_POOL_HEX=0x<pool>
export BOING_NATIVE_AMM_ACTION=add
export BOING_AMOUNT_A=1000000
export BOING_AMOUNT_B=2000000
npm run native-amm-submit-contract-call
```

**Browser:** configure **boing.finance** with your pool id and RPC (below), then use **Add liquidity** in the native AMM panel ([NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md)).

Then **`fetch-native-amm-reserves`** again — reserves should match your seed (plus any later trades).

---

## 5b. LP vault + LP share (optional)

If you deploy [NATIVE-AMM-LP-VAULT.md](NATIVE-AMM-LP-VAULT.md) and [NATIVE-LP-SHARE-TOKEN.md](NATIVE-LP-SHARE-TOKEN.md) on the same devnet, use the same **`examples/native-boing-tutorial`** package:

| Goal | Command | Key env vars |
|------|---------|----------------|
| **`set_minter_once`** (vault → minter) | `npm run native-lp-share-submit-contract-call` | `BOING_LP_SHARE_ACTION=set_minter_once`, `BOING_MINTER_HEX`, `BOING_LP_SHARE_HEX` |
| **Vault `configure`** | `npm run native-amm-lp-vault-submit-contract-call` | `BOING_LP_VAULT_ACTION=configure`, `BOING_VAULT_HEX`, `BOING_POOL_HEX`, `BOING_SHARE_HEX` |
| **Vault `deposit_add`** | `npm run native-amm-lp-vault-submit-contract-call` | `BOING_LP_VAULT_ACTION=deposit`, `BOING_AMOUNT_A` / `BOING_AMOUNT_B`, … |

Full env tables: [examples/native-boing-tutorial/README.md](../examples/native-boing-tutorial/README.md) §7f–§7i.

---

## 6. Point Boing Express + boing.finance at your RPC and pool

**On your side (not in this repo):**

| Piece | What to set |
|--------|-------------|
| **Boing Express** | Testnet RPC env to your public URL (e.g. **`VITE_BOING_TESTNET_RPC`** in a local wallet build — [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) §2). |
| **boing.finance** | Same RPC for chain **6913** (or your chain), plus **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** / **`REACT_APP_BOING_NATIVE_AMM_POOL`** / **`nativeConstantProductPool`** = deployed **`0x` + 64 hex** ([TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) §2, [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) §3 pattern, but with **your** pool). |

Until those are set, the hosted **boing.finance** site will keep talking to **default** public RPC and will **not** know your pool address.

---

## 7. Optional checks

- **`BOING_INTEGRATION_RPC_URL=<your RPC> npm run verify`** in **`boing-sdk`** — runs live RPC integration tests ([PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) §6).
- **`BOING_POOL_HEX=... npm run fetch-native-amm-logs`** — **`Log2`** after successful swap / add / remove.

---

## References

| Doc | Role |
|-----|------|
| [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) | Selectors, storage keys, CREATE2 salts |
| [examples/native-boing-tutorial/README.md](../examples/native-boing-tutorial/README.md) | All **`npm run`** env tables |
| [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) | dApp + Express **`contract_call`** shape |
