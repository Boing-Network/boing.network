# Native AMM — manual E2E smoke (Boing Express + boing.finance)

Use this after code changes to the wallet, RPC, or dApp. It complements the node-level test `native_amm_rpc_happy_path` in **boing.network** (no browser extension).

## Preconditions

1. **Pool id:** Non-zero `nativeConstantProductPool` for chain **6913** — default public testnet pool **`0x7247ddc3180fdc4d3fd1e716229bfa16bad334a07d28aa9fda9ad1bfa7bdacc3`** ([RPC-API-SPEC.md](RPC-API-SPEC.md) § Native constant-product AMM, [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published). Set **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** in **`frontend/src/config/boingCanonicalTestnetPool.js`** and/or **`REACT_APP_BOING_NATIVE_AMM_POOL`**, or `import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from 'boing-sdk'`. **Your own validator/full node + public RPC:** deploy a pool first, then point the dApp + wallet at your RPC and pool id ([DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md)).
2. **Boing Express:** Unpacked or store build with a wallet that has testnet BOING (faucet on [boing.network/faucet](https://boing.network/faucet)), or devnet BOING on your chain.
3. **RPC:** Default smoke assumes `https://testnet-rpc.boing.network` is reachable from the browser (CORS allows boing.finance origins). For a **private** RPC, use local builds of Express + boing.finance with your URL ([THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) §2).

## Happy path — one swap

1. Load **boing.finance** `/swap` in Chrome with Boing Express enabled.
2. Connect **Boing Express** on **Boing testnet (6913)**. Approve **connection** and **message signature** if prompted.
3. Confirm the **Native constant-product pool (Boing VM)** panel is visible (`data-testid="native-amm-panel"` in devtools).
4. Click **Refresh reserves**; confirm Reserve A / B load or show a clear RPC error (fix RPC/pool id if not).
5. Enter a small **integer** amount in (within reserves and u64-safe range).
6. Click **Swap via Boing Express**. Approve **transaction signing** (and a second sign if simulation widens the access list).
7. Confirm toast shows a submitted tx hash; optional: look up the account on [boing.observer](https://boing.observer).

## Optional — add liquidity

1. Expand **Add liquidity (reserve A + B)**.
2. Enter two positive integers; submit **Add liquidity via Boing Express** and complete signing.

## Automated extension E2E (optional)

In-repo Playwright harness (headed **Chromium**, loads unpacked extension): [examples/native-boing-playwright/README.md](../examples/native-boing-playwright/README.md). Set **`BOING_EXPRESS_EXTENSION_PATH`**, run **`npm install`** and **`npx playwright install chromium`** in that folder, then **`npm run test:e2e`**. After the window opens, **unlock Boing Express** and **connect** on testnet **6913** within the panel timeout (default **120s**). Without the env var, the suite **skips** (exit **0**) so CI can still validate the harness.

Full unattended CI still needs a safe unlock strategy (not committed here); this satisfies checklist **A4.3** as **optional local / release regression** automation alongside the manual steps above.

## References

- [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md)
- [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) — Native constant-product swap
- [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) — RPC and CORS
