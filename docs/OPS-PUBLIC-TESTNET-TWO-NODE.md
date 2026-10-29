# Public testnet on Windows — validator + full node

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.

**Goal:** One **validator** and one **full node** on separate Windows PCs, both joined to the public testnet.

| Need | How |
|------|-----|
| Correct **`boing-node` build** | Zips from the latest [GitHub release](https://github.com/Boing-Network/boing.network/releases) or **`GET https://boing.network/api/networks`** ([NETWORKS-API.md](NETWORKS-API.md)) |
| **Bootnodes + P2P** | Pass **`--bootnodes`** and **`--p2p-listen`** ([TESTNET.md](TESTNET.md) §6) |
| **Validator** | One machine with **`--validator`** and a staked account |
| **Full node** | Second machine **without** **`--validator`** — local RPC for tools |

**Automated preflight (from a `boing.network` clone):**

```bash
npm run public-testnet-node-preflight
# lighter: npm run compare-local-public-tip
```

| Variable | Default | Purpose |
|----------|---------|---------|
| **`BOING_LOCAL_RPC_URL`** | `http://127.0.0.1:8545` | Local node JSON-RPC |
| **`BOING_PUBLIC_RPC_URL`** | `https://testnet-rpc.boing.network` | Reference tip + `chain_id` |
| **`BOING_SYNC_MAX_LAG`** | `256` | Max blocks local may trail public |

**Manual checklist:** outbound/inbound TCP **4001**; **one** validator process per staked identity; same network settings on both PCs; public tunnel only if **you** publish RPC to others.

See also [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md), [NODE-OPERATOR-COMMANDS.md](NODE-OPERATOR-COMMANDS.md).

---

*Boing Network — Quality, enforced by the protocol.*
