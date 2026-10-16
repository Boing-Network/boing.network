# Run a Boing testnet validator

Public testnet: chain **6913** (`0x1b01`), RPC `https://testnet-rpc.boing.network/`, explorer <https://boing.observer>.
Website version: <https://boing.network/testnet/operators/validator>.

> **How the validator set works today.** The public testnet currently runs a **static validator set**
> (one operator validator, `boing-testnet-1`, plus the full node `boing-testnet-2`). Your node syncs and serves
> RPC as soon as it is up; it **signs blocks only once its AccountId is in the set**. Stake is handed out
> manually and admission is coordinated with the operator (see [Request stake](#4-request-stake)). The node
> also supports a stake-weighted set (`--validator-set stake`, top 21 stakers with at least **10,000 BOING**,
> refreshed every 100 blocks); the public testnet will switch to it once enough operators are bonded.

## 1. Hardware and network

| | Minimum | Recommended |
|---|---|---|
| CPU | 2 vCPU (x86_64 or arm64) | 4 vCPU |
| RAM | 2 GB | 4 GB |
| Disk | 10 GB SSD | 50 GB SSD (chain data grows) |
| Network | Public IPv4, inbound **TCP 4001** (P2P) open | Static IP, stable uptime |

RPC port **8545** only needs to be public if you want to serve RPC to others.

## 2. Get the node

**Binary (Linux x86_64, macOS arm64, Windows x86_64):** download `release-<platform>.zip` from the
[testnet-v0.1.12 release](https://github.com/Boing-Network/boing.network/releases/tag/testnet-v0.1.12); it
contains `boing-node` and the `boing` CLI.

```bash
curl -LO https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.12/release-linux-x86_64.zip
unzip release-linux-x86_64.zip && chmod +x boing-node boing
./boing-node --version
```

**Docker (build from source):**

```bash
git clone https://github.com/Boing-Network/boing.network && cd boing.network
git checkout testnet-v0.1.12
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

## 4. Request stake

Validators need at least **10,000 BOING** bonded. The faucet (20 BOING per request) is for gas, not stake;
stake is **allocated manually** by the testnet operator.

Send your request with: your **AccountId**, node P2P multiaddr (`/ip4/<public-ip>/tcp/4001`), hardware/location,
and how long you plan to run it.

- Email: [nico.builds@boing.network](mailto:nico.builds@boing.network?subject=Boing%20testnet%20validator%20stake%20request)
- Discord: <https://discord.gg/boing> (operators channel)
- Also register your node on the [operator registration](https://boing.network/testnet/register/operator) page.

## 5. Sync from the testnet bootnodes

Official bootnodes (also on <https://boing.network/bootnodes>):

```
/ip4/169.155.48.188/tcp/4001
/ip4/109.105.220.118/tcp/4001
```

**Binary:**

```bash
export BOING_CHAIN_ID=6913
./boing-node \
  --data-dir ./boing-data \
  --p2p-listen /ip4/0.0.0.0/tcp/4001 \
  --bootnodes /ip4/169.155.48.188/tcp/4001,/ip4/109.105.220.118/tcp/4001 \
  --rpc-port 8545
```

**Docker:**

```bash
docker run -d --name boing-node --restart unless-stopped \
  -p 4001:4001 -p 127.0.0.1:8545:8545 \
  -v boing-data:/data \
  -e BOING_CHAIN_ID=6913 \
  -e BOING_BOOTNODES=/ip4/169.155.48.188/tcp/4001,/ip4/109.105.220.118/tcp/4001 \
  boing-node
```

Check it is syncing: `boing_getSyncState` / `boing_health` on your node should report the same `head_height`
as `https://testnet-rpc.boing.network/` within a few blocks.

## 6. Bond (stake)

Once you have received stake on your validator AccountId, bond it from the same account. The `boing` CLI has no
staking command yet; bond from **Boing Express → Staking**:

1. Unlock the validator wallet in Boing Express on Boing Testnet.
2. Staking → enter the amount (at least **10,000**) → **Bond**. Keep a few BOING unbonded for fees.
3. Confirm with `boing_getAccount` (`stake` field) or on boing.observer.

**Unbond** from the same Staking section; unbonded BOING unlocks after the unbonding period
(`unbond_unlock_height` in `boing_getAccount`), then use **Claim**.

## 7. Run as a validator

After the operator confirms your AccountId has been added to the validator set, restart with the validator flags
(keep the key in an env file, not on the command line):

```bash
# validator.env (chmod 600)
BOING_CHAIN_ID=6913
BOING_VALIDATOR_KEY=<32-byte secret hex>

set -a; . ./validator.env; set +a
./boing-node --validator --data-dir ./boing-data \
  --p2p-listen /ip4/0.0.0.0/tcp/4001 \
  --bootnodes /ip4/169.155.48.188/tcp/4001,/ip4/109.105.220.118/tcp/4001 \
  --rpc-port 8545
```

Docker: add `--env-file validator.env -e BOING_VALIDATOR=1`.

## 8. Monitoring

```bash
curl -fsS http://127.0.0.1:8545/live
curl -s -X POST http://127.0.0.1:8545 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_health","params":[]}'
```

Watch: `head_height` keeps pace with the public RPC, peer count ≥ 1, disk space, process restarts.
Use a process supervisor (systemd `Restart=always`, or Docker `--restart unless-stopped`) and alert on `/live` failing.
The [operator dashboard and leaderboard](https://boing.network/testnet/operators) show your node once registered.

## 9. Duties

- Keep the node **online and in sync**; missed rounds hurt the network and your leaderboard standing.
- Upgrade promptly when a new `testnet-v*` release is announced. Consensus changes are coordinated: upgrade
  in the announced window.
- Protect the validator key; report a suspected compromise immediately (email/Discord above).
- Keep port 4001 reachable so other nodes can peer with you.
- This is a **testnet**: BOING has no monetary value and the chain may be reset.