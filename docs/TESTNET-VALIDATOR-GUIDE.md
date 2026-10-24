# Run a Boing testnet validator

Public testnet: chain **6913** (`0x1b01`), RPC `https://testnet-rpc.boing.network/`, explorer <https://boing.observer>.
Website version: <https://boing.network/testnet/operators/validator>.

> **How to become a validator today: open registration.** Anyone can join: run a synced node, then
> **register it with a 100 BOING bond** (Boing Express **Validate** tab, or `boing validator register`). You join
> the set at the next epoch (every 100 blocks) and must **stay online**: missing more than 10% of your scheduled
> proposals jails you (bond kept; unjail after a cooldown). No operator approval and no 10,000 BOING stake are
> needed. See [Open validator registration](#open-validator-registration).

## 1. Hardware and network

| | Minimum | Recommended |
|---|---|---|
| CPU | 2 vCPU (x86_64 or arm64) | 4 vCPU |
| RAM | 2 GB | 4 GB |
| Disk | 10 GB SSD | 50 GB SSD (chain data grows) |
| Network | Public IPv4, inbound **TCP 4001** (P2P) open | Static IP, stable uptime |

RPC port **8545** only needs to be public if you want to serve RPC to others.

## 2. Get the node

**Release binaries (recommended):** download `testnet-v0.1.18` (or newer) from
<https://github.com/Boing-Network/boing.network/releases>. The zip contains `boing-node-<platform>` and
`boing-<platform>`, so rename them after unzipping (there is no `--version` flag; use `./boing-node --help`):

```bash
curl -sSLO https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.18/release-linux-x86_64.zip
unzip release-linux-x86_64.zip && mv boing-node-linux-x86_64 boing-node && mv boing-linux-x86_64 boing && chmod +x boing-node boing
./boing validator --help
```

Other platforms: `release-macos-aarch64.zip`, `release-windows-x86_64.zip`.

> **Upgrade to `testnet-v0.1.18` before height 1150.** From block **1150** contract calls can carry
> native BOING (a new `ContractCallWithValue` transaction variant and the `CALLVALUE`, `SELFBALANCE`,
> `BALANCE` and `NATIVE_TRANSFER` opcodes), which is what wBOING and native BOING swaps use. Older releases
> reject the first such transaction and stop following the chain. Nothing to configure: replace the binary
> and restart; your data directory is kept. Details: [NATIVE-VALUE-AND-WBOING.md](NATIVE-VALUE-AND-WBOING.md).

**`testnet-v0.1.17` or newer was required before height 1050.** At block **1050** native BOING switches to
**18 decimals** (1 BOING = 10^18 base units): every balance, stake, pending unbond and validator bond is
multiplied by 10^18 once, fees become `gas_used × 10^12` base units (a transfer costs 0.021 BOING) and the
bond, name rent and faucet amounts are charged in base units. Older releases compute a different state
root at 1050 and stop following the chain. Nothing to configure: replace the binary and restart; your data
directory is kept. Details: [NATIVE-DENOMINATION.md](NATIVE-DENOMINATION.md).

**`testnet-v0.1.16` or newer was required before height 1000.** Older releases cannot follow the chain: from height 900 transactions must be signed
with the chain id (v2) and failed transactions pay a fee; from height 920 blocks carry a signature per
transaction that every node verifies, and the validator registration proof is bound to the chain id
(security fixes #2, #3, #14, #16); from height **1000** name commitments are stored per sender, parallel
transfer batches that touch a fee account run sequentially, the open-validator registry is capped and a
jailed validator cannot deregister before its jail ends (fixes #6, #9, #10, #11). A node on an older
release computes a different state root from block 1000 and stops following the chain. Upgrade by
replacing the binary and restarting; your data directory is kept.

**Build from source (alternative):**

```bash
# Ubuntu / WSL: sudo apt install -y build-essential pkg-config libssl-dev clang git; install Rust via https://rustup.rs
git clone https://github.com/Boing-Network/boing.network && cd boing.network
cargo build --release -p boing-node -p boing-cli
cp target/release/boing-node target/release/boing .
```

**Docker (build from source):**

```bash
git clone https://github.com/Boing-Network/boing.network && cd boing.network
docker build -t boing-node .
```

The image entrypoint reads `BOING_DATA_DIR`, `BOING_P2P_LISTEN`, `BOING_RPC_PORT`, `BOING_VALIDATOR`, `BOING_BOOTNODES`;
the node itself reads `BOING_CHAIN_ID`, `BOING_VALIDATOR_KEY`, `BOING_VALIDATORS` and the other `BOING_*` settings.

## 3. Keys

Your validator identity is an Ed25519 key; its public key is your **AccountId** (64 hex).

1. Open [Boing Express](https://boing.express) and **Create wallet** (or import an existing key) on Boing Testnet.
2. Copy the **AccountId** (your validator address).
3. Use **Reveal private key** to get the 32-byte secret for the node (`BOING_VALIDATOR_KEY`).

Store the secret in a file only you can read (`chmod 600`), never in a shell history, repo or chat.
Use a dedicated key for the validator, not your everyday wallet.

## 4. Fund the bond

Registering locks a **100 BOING** bond; keep a few BOING extra for fees (about 1 BOING per transaction).
The faucet sends 20 BOING per request (once a minute), so a few requests cover the bond. Need help? Reach out with
your **AccountId** and node P2P multiaddr (`/ip4/<public-ip>/tcp/4001`):

- Email: [nico.builds@boing.network](mailto:nico.builds@boing.network?subject=Boing%20testnet%20validator%20stake%20request)
- Discord: <https://discord.gg/boing> (operators channel)
- Also register your node on the [operator registration](https://boing.network/testnet/register/operator) page.

## 5. Sync from the testnet bootnodes

Official bootnode (also on <https://boing.network/bootnodes>):

```
/ip4/169.155.48.188/tcp/4001
```

**Binary:**

```bash
# These must match the public nodes or your node computes different roots:
export BOING_CHAIN_ID=6913 BOING_VALIDATOR_SET=open BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT=400 \
       BOING_NAME_REGISTRY_ACTIVATION_HEIGHT=640
# With BOING_CHAIN_ID=6913 the node applies the testnet network preset (genesis allocations, historical
# fee rules, anchor validator), so it replays from genesis with identical roots. No --faucet-enable needed.
./boing-node \
  --data-dir ./boing-data \
  --p2p-listen /ip4/0.0.0.0/tcp/4001 \
  --bootnodes /ip4/169.155.48.188/tcp/4001 \
  --rpc-port 8545
```

**Docker:**

```bash
docker run -d --name boing-node --restart unless-stopped \
  -p 4001:4001 -p 127.0.0.1:8545:8545 \
  -v boing-data:/data \
  -e BOING_CHAIN_ID=6913 -e BOING_VALIDATOR_SET=open -e BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT=400 \
  -e BOING_NAME_REGISTRY_ACTIVATION_HEIGHT=640 \
  -e BOING_BOOTNODES=/ip4/169.155.48.188/tcp/4001 \
  boing-node
```

Check it is syncing: `boing_getSyncState` / `boing_health` on your node should report the same `head_height`
as `https://testnet-rpc.boing.network/` within a few blocks. A fresh node syncs the whole chain from genesis
(about a minute for the first ~700 blocks). Don't set `--validators` / `BOING_VALIDATORS`; the preset supplies the anchors.

## 6. Register

From **Boing Express, Validate tab** with the owner wallet (or `boing validator register --node-key-hex <node key>`):

1. Unlock the owner wallet on Boing Testnet and open **Validate**.
2. Paste your node key (or generate one locally), check the node id, then **Register validator (100 BOING bond)**.
3. The validator shows as **pending** and joins at the next epoch; the tab shows status, uptime, missed blocks, bond and jail stats.

To leave: **Deregister**; the bond unlocks after about 100 blocks, then **Claim unbond**. Jailed? **Unjail** after the cooldown.

## 7. Run as a validator

Run with the validator flags and the open-mode settings used by the public nodes (keep the key in an env
file, not on the command line), synced before your registration's epoch boundary:

```bash
# validator.env (chmod 600)
BOING_CHAIN_ID=6913
BOING_VALIDATOR_SET=open
BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT=400
BOING_NAME_REGISTRY_ACTIVATION_HEIGHT=640
BOING_VALIDATOR_KEY=<32-byte node key hex>

set -a; . ./validator.env; set +a
./boing-node --validator --data-dir ./boing-data \
  --p2p-listen /ip4/0.0.0.0/tcp/4001 \
  --bootnodes /ip4/169.155.48.188/tcp/4001 \
  --rpc-port 8545
```

Docker: add `--env-file validator.env -e BOING_VALIDATOR=1`.

**Fly.io:** copy `fly.testnet-1.toml` (or the retired `fly.testnet-2.toml` template) to your own `fly.toml`, change `app` and remove the public-RPC/CORS
extras. Keep the `[env]` consensus values above, add `BOING_VALIDATOR_SET=open`,
`BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT=400` and `BOING_BOOTNODES=...`, and put the key in a secret:
`fly secrets set BOING_VALIDATOR_KEY=...` (type it at the prompt or read it from a file; never commit it). Then run
`fly volumes create boing_data --size 10` and `fly deploy --remote-only`. The image entrypoint maps
`BOING_VALIDATOR=1` to the CLI flag.

**Unjail:** `boing validator status --node <id> --rpc-url https://testnet-rpc.boing.network` shows `jailed`
and the release height. After the 500-block cooldown run `BOING_SECRET_HEX=... boing validator unjail --rpc-url ...`
(add `--dry-run` first), and keep the node online: you rejoin at the next epoch boundary.

## 8. Monitoring

```bash
curl -fsS http://127.0.0.1:8545/live
curl -s -X POST http://127.0.0.1:8545 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_health","params":[]}'
```

Watch: `head_height` keeps pace with the public RPC, peer count ≥ 1, disk space, process restarts.
Use a process supervisor (systemd `Restart=always`, or Docker `--restart unless-stopped`) and alert on `/live` failing.
The [operator dashboard and leaderboard](https://boing.network/testnet/operators) show your node once registered.

Resource limits (since `testnet-v0.1.16`, node-local, no consensus impact): the mempool holds at most
`BOING_MEMPOOL_MAX_TXS` transactions (default 10,000; when full, the newest txs of the sender with the most
pending txs make room), and `GET /ws` accepts `BOING_RPC_WS_MAX_CONNECTIONS` connections (default 512;
`0` = unlimited).

## Future: stake-based validator set (not active)

The node also implements a stake-weighted mode that may be enabled later at an announced **activation
height**. It is **not active**: today validators join through open registration (100 BOING bond). If enabled:

- **Joining.** At each epoch boundary (every **100** committed blocks, heights that are multiples of 100 at or
  after the activation height) every node recomputes the set from chain state: all accounts with at least
  **10,000 BOING** bonded, highest stake first (ties broken by AccountId), capped at the **top 21**. If your
  bonded stake qualifies, you become a validator at the next boundary. Your node must be running and synced
  with `BOING_VALIDATOR=1` *before* that boundary, otherwise the network is waiting on a signer that is offline.
- **Leaving.** Unbonding reduces your active stake immediately; if you drop below 10,000 BOING (or out of the
  top 21) you leave the set at the next boundary. Unbonded funds still unlock after the usual delay.
- **Determinism.** Every node computes the same set from the same committed state; the local node is never
  added just because it is running. The set is persisted in the data directory (`stake_validator_set.json`)
  and restored on restart.
- **Bootstrap fallback.** If no account qualifies at a boundary, the static bootstrap list is used, so the chain
  never ends up with an empty set. Before the activation height the static list is used unconditionally.
- **Operator flags** (all nodes must use identical values): `--validator-set stake`
  (`BOING_VALIDATOR_SET=stake`), `--stake-validator-activation-height <H>`
  (`BOING_STAKE_VALIDATOR_ACTIVATION_HEIGHT`), and optionally `BOING_STAKE_VALIDATOR_TOP_N`,
  `BOING_STAKE_VALIDATOR_EPOCH_LEN`, `BOING_STAKE_VALIDATOR_MIN_STAKE`. The activation will be announced with
  the required release and values; do not enable stake mode early on your own.

Planning to stop validating? Keep the node online until the boundary after your unbond has removed you.

## Open validator registration

> **Status: live on testnet since block 400.** The public testnet nodes run with `BOING_VALIDATOR_SET=open`
> and `BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT=400`; your node needs the **same** values (and the default
> knobs) or it will disagree on the state root. You can also register from the boing.express **Validate** tab.

Anyone can join without permission:

- **Register:** `boing validator register --node-key-hex <node seed>` (or `BOING_VALIDATOR_KEY`). The owner
  (`--secret-hex` / `BOING_SECRET_HEX`, defaulting to the node key) pays and locks a **100 BOING bond**
  (100 × 10^18 base units from height 1050).
  The node key signs a proof of possession binding it to the owner and to this chain (chain id 6913; required from
  height 920), so nobody can register a key they don't hold or replay a proof from another network. Use
  `testnet-v0.1.15` or newer (older CLIs sign the old proof, which is rejected).
  One validator per owner. From height 1000 the registry holds at most 80 records (4 x the open-validator
  cap; pending, active, jailed and exiting together); further registrations fail with "registry is full"
  until someone leaves.
- **Join:** you enter the set at the next **epoch boundary (every 100 blocks)**. The set is capped
  (default 20 open validators), ordered by registration height then id, identical on every node.
- **Liveness:** each produced block records the expected leader and the actual proposer. Missing more than
  **10%** of your scheduled proposals over the last **1,000 produced blocks** (at least 10 samples) gets you
  **jailed** at the next boundary. Only produced blocks count; idle time with no transactions never penalizes you.
  Jailing keeps your bond (no slashing on testnet). After the cooldown (500 blocks), run
  `boing validator unjail` to rejoin at the next boundary. From height 1000 a jailed validator cannot
  deregister before the cooldown ends either, so the bond stays locked for the whole jail period.
- **Leave:** `boing validator deregister`. You stay in the set until the next boundary; the bond unlocks after
  100 blocks and is claimed with `ClaimUnbond`.
- **Status:** `boing validator status [--node <id>]` (RPC `boing_getValidators`) shows each validator's status,
  uptime %, missed/scheduled proposals and bond.
- **Anchors:** the operator-run bootstrap validators are permanent and are never dropped. Commits need a quorum of
  the anchors; open validators propose and vote but can't stall the chain if they go offline (an anchor takes over
  a missed slot after ~3 s).

## 9. Duties

- Keep the node **online and in sync**; missed rounds hurt the network and your leaderboard standing.
- Upgrade promptly when a new `testnet-v*` release is announced. Consensus changes are coordinated: upgrade
  in the announced window.
- Protect the validator key; report a suspected compromise immediately (email/Discord above).
- Keep port 4001 reachable so other nodes can peer with you.
- This is a **testnet**: BOING has no monetary value and the chain may be reset.