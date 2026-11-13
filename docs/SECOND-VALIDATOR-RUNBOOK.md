# Second validator / path toward HA (H10)

**Status:** Public testnet is a **single hosted validator** (`boing-testnet-1`, `validator_count: 1`).  
**Do not claim HA or multi-validator BFT** until a live tip reports `validator_count >= 4` with matching static sets and quorum commits under load.  
**Cost:** This runbook is optional. Standing up Fly apps spends money — prefer the cheapest VM that can sync; do **not** enable Workers Paid.

Related: [FLY-IO.md](FLY-IO.md), [RUNBOOK.md](RUNBOOK.md) §8.1, [TESTNET.md](TESTNET.md), [MAINNET-READINESS.md](MAINNET-READINESS.md) (H10).

## Quorum honesty (read first)

HotStuff-style quorum here is **`2f+1`** with `f = (n − 1) / 3` (integer division):

| Validators `n` | `f` | Quorum | What you get |
|----------------|-----|--------|--------------|
| **1** (today) | 0 | 1 | Single-validator soak — **not HA** |
| **2** | 0 | 1 | Dual proposers / ops practice; **either vote alone commits** — **not BFT HA** |
| **3** | 0 | 1 | Still `f = 0` — **not BFT HA** |
| **4** | 1 | 3 | First classic Byzantine threshold (`f = 1`) |

A “second validator” is a **step toward HA**, not HA itself. Announcements must say **single-validator** or **dual-proposer soak** until `n ≥ 4`.

## Paths

### Path A — Companion full node (cheapest; sync / RPC failover only)

Restore or use repo-root [`fly.testnet-2.toml`](../fly.testnet-2.toml) with **`BOING_VALIDATOR=0`**. Peers with testnet-1 over 6PN / bootnodes. Improves **read replica / sync soak**, **not** consensus redundancy.

```bash
# From repo root (costs ~shared-cpu-1x VM + 10 GB volume)
FLY_APP_2=boing-testnet-2 FLY_CONFIG_2=fly.testnet-2.toml \
  ./scripts/fly-deploy-testnet.sh
# Or: fly deploy --config fly.testnet-2.toml --remote-only --ha=false
```

Align env with testnet-1 for every **consensus** activation height (trees, QA, equivocation, micro-fees, name registry, etc.). Keep **`BOING_RATE_PROFILE=mainnet`** if the origin is reachable; set **`BOING_ORIGIN_GATE_TOKEN`** if you expose `*.fly.dev`.

Public gateway may keep pointing at **testnet-1 only** (current design). Companion RPC is operator-only unless you change the Worker upstream (out of scope here; Free Worker single-hop preference).

### Path B — Dual-validator static set (ops practice; still not BFT)

1. Generate two Ed25519 validator keys; derive AccountIds (32-byte pubkey hex).
2. On **both** machines set the **same** ordered set:

   ```bash
   export BOING_VALIDATORS=<ID1_HEX>,<ID2_HEX>
   export BOING_VALIDATOR_KEY=<THIS_NODE_SECRET_HEX>
   export BOING_VALIDATOR=1
   ```

3. Restart / redeploy so both run `--validator` with those env vars (Fly secrets for keys — never commit).
4. Verify: `boing_getNetworkInfo.consensus.validator_count == 2`, both peers connected (`boing_health.p2p`), tip advances when either leads.
5. Messaging: **“dual-proposer testnet soak”** — not “HA”.

**Genesis caveat:** Changing `BOING_VALIDATORS` on a live chain that already committed under a different set requires a **coordinated restart / new network**. Public testnet 6913 is already mid-flight as single-validator; promoting to dual validators on the **same** ledger needs a planned cutover (new genesis or carefully shared bootstrap — treat as a dedicated ops change, not a casual env flip).

For **new** private / mainnet nets: bake `n ≥ 4` into genesis validators from day one when claiming BFT.

### Path C — Four validators (minimum honest BFT claim)

Repeat Path B with **four** keys and `BOING_VALIDATORS` of length 4. Prefer at least two availability zones / machines. Only then may ops language say **multi-validator BFT (f=1)**. Cost ≈ 4× VM; keep companions small (`shared-cpu-1x` / `1024mb` if tip sync allows; bump memory on OOM / PHASE7 blobs).

## Local multi-validator without Fly

```bash
cargo test -p boing-node --test multi_validator_consensus
# Manual four-node: RUNBOOK §8.1 — --validators + --validator-key on each process
```

## Single-validator genesis residual (copy/paste)

When Nico accepts H10 without standing up more nodes:

> Public / day-0 chain may launch with **one hosted validator**. This is **not** high availability and **not** Byzantine fault tolerant. Multi-validator BFT (`n ≥ 4`) remains a post-genesis ops goal. Client and marketing copy must not claim HA while `validator_count` is 1.

## Checklist before claiming anything beyond single-validator

- [ ] Live `boing_getNetworkInfo.consensus.validator_count` matches the announced set
- [ ] Every node shares identical activation heights and chain id
- [ ] P2P: bootnodes connected; tip advances under proposal timeout / pacemaker
- [ ] Copy reviewed: no “HA” / “BFT” wording unless `n ≥ 4`
- [ ] Gateway / origin gate still single-hop Free Worker (no Workers Paid from this work)
