# Native AMM — manual E2E smoke (Boing Express + boing.finance)

Use this after code changes to the wallet, RPC, or dApp. It complements the node-level test `native_amm_rpc_happy_path` in **boing.network** (no browser extension).

## Preconditions

1. **Pool id:** Non-zero `nativeConstantProductPool` for chain **6913** — set `REACT_APP_BOING_NATIVE_AMM_POOL` for local/production boing.finance builds, or the committed placeholder until ops publishes a long-lived testnet pool (see [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md)).
2. **Boing Express:** Unpacked or store build with a wallet that has testnet BOING (faucet on [boing.network/faucet](https://boing.network/faucet)).
3. **RPC:** `https://testnet-rpc.boing.network` reachable from the browser (CORS allows boing.finance origins).

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

## Automated extension E2E (future)

CI-friendly runs need Chrome with `--load-extension=…` and scripted unlock (password + approvals). Until then, this manual script satisfies checklist **A4.3**. Optional local automation: Playwright with `BOING_EXPRESS_EXTENSION_PATH` and stored session is tracked as a follow-up.

## References

- [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md)
- [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) — Native constant-product swap
- [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) — RPC and CORS
