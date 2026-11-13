# Second validator / path toward HA (H10)

**Status:** Public testnet chain **6913** runs **`BOING_VALIDATOR_SET=open`** with a single live anchor proposer historically; H10 close = stand up **`boing-testnet-2`**, sync, open-register so live `validator_count` ≥ 2.  
**Do not claim HA or multi-validator BFT** until a live tip reports `validator_count >= 4` with matching sets and quorum commits under load.  
**Claim after close:** “second validator live; single-machine residual closed” — **not** BFT HA.  
**Cost:** Match `boing-testnet-1` class (`shared-cpu-2x` / `2048mb`, under ~$1/day). Do **not** enable Workers Paid.

Related: [FLY-IO.md](FLY-IO.md), [RUNBOOK.md](RUNBOOK.md) §8.1, [TESTNET.md](TESTNET.md), [TESTNET-VALIDATOR-GUIDE.md](TESTNET-VALIDATOR-GUIDE.md), [MAINNET-READINESS.md](MAINNET-READINESS.md) (H10).

## Quorum honesty (read first)

HotStuff-style quorum here is **`2f+1`** with `f = (n − 1) / 3` (integer division):

| Validators `n` | `f` | Quorum | What you get |
|----------------|-----|--------|--------------|
| **1** | 0 | 1 | Single-validator soak — **not HA** |
| **2** | 0 | 1 | Dual proposers / ops practice; **either vote alone commits** — **not BFT HA** |
| **3** | 0 | 1 | Still `f = 0` — **not BFT HA** |
| **4** | 1 | 3 | First classic Byzantine threshold (`f = 1`) |

## Path Open — second Fly validator on live testnet 6913 (preferred)

Live tip already uses open registration (`boing_getValidators.mode == "open"`). Do **not** flip to a static two-key `BOING_VALIDATORS` set on the mid-flight ledger without a dedicated cutover.

1. Confirm cost: `fly.testnet-2.toml` uses **shared-cpu-2x / 2048mb** (same class as testnet-1, ≈ under $1/day). Stop if spend would clearly exceed that class.
2. Generate an Ed25519 node key; set Fly secret `BOING_VALIDATOR_KEY` on `boing-testnet-2` (never commit).
3. Deploy companion:

   ```bash
   FLY_APP_2=boing-testnet-2 FLY_CONFIG_2=fly.testnet-2.toml \
     ./scripts/fly-deploy-testnet.sh
   ```

4. Align non-secret env with testnet-1 (open activation **400**, trees, QA, micro-fees, etc. — already in the toml / preset).
5. Wait until the node syncs near tip (large PutBlob blocks need 2 GiB RAM; watch OOM).
6. Fund the owner (≥ **100 BOING** bond + fees) via faucet / operator; then:

   ```bash
   # From a machine with boing CLI; keys via env/files — never paste into chat logs casually
   export BOING_VALIDATOR_KEY=<node-secret-hex>
   # owner may equal node key:
   boing validator register --rpc https://testnet-rpc.boing.network/
   ```

7. Wait for the next open-validator epoch boundary (`boing_getValidators.open.next_epoch_height`). Confirm `validator_count >= 2` and the new id `in_set: true`.
8. Messaging: **“second validator live; dual-proposer / open-set soak”** — not “HA”.

Public gateway may keep pointing at **testnet-1 only** (single-hop Free Worker).

## Path A — Companion full node only

Same Fly app with `BOING_VALIDATOR=0` — sync / RPC failover only; does **not** raise `validator_count`.

## Path B — Dual-validator static set (new nets / coordinated cutover)

For **new** private / mainnet nets (or a planned restart): shared `BOING_VALIDATORS` of length 2 + per-node `BOING_VALIDATOR_KEY`. Still **not** BFT HA.

## Path C — Four validators (minimum honest BFT claim)

`n ≥ 4` before any “multi-validator BFT” language.

## Checklist

- [ ] Live `boing_getNetworkInfo.consensus.validator_count` / `boing_getValidators.validator_count` ≥ 2
- [ ] Every node shares identical activation heights and chain id
- [ ] P2P peers connected; tip advances
- [ ] Copy reviewed: no “HA” / “BFT” wording unless `n ≥ 4`
- [ ] Gateway / origin gate still single-hop Free Worker (no Workers Paid)
