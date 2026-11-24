---
title: Bootnodes
description: The official Boing public testnet bootnode, how to check it, and addresses you should no longer use.
---

A **bootnode** is a stable peer your node dials first to find the rest of the network. Pass it with
`--bootnodes` (or `BOING_BOOTNODES` in Docker). Separate several addresses with commas.

## Official bootnode

| Multiaddr | Host | Status |
|-----------|------|--------|
| `/ip4/169.155.48.188/tcp/4001` | Fly.io `boing-testnet-1` | **Live** |

```bash
--bootnodes /ip4/169.155.48.188/tcp/4001
```

The same list is published as JSON by the website at
[boing.network/api/networks](https://boing.network/api/networks) (format in
[NETWORKS-API.md](https://github.com/Boing-Network/boing.network/blob/main/docs/NETWORKS-API.md)) and on
[boing.network/bootnodes](https://boing.network/bootnodes).

## Retired addresses

Remove these from your configuration. Dialing them only adds a failed connection on every start.

| Multiaddr | Host | Retired |
|-----------|------|---------|
| `/ip4/109.105.220.118/tcp/4001` | `boing-testnet-2` | 2026-10-24 |

## Check that you can reach it

```bash
nc -vz 169.155.48.188 4001
```

A successful TCP connect means your network allows the connection. From a checkout of the repository,
`npm run public-testnet-node-preflight` checks the bootnode, your local node, and the public tip together.

If the bootnode is reachable but your node does not sync, see [Troubleshooting](/nodes/troubleshooting/).

## Run your own bootnode

Any synced node with a static public IP and inbound TCP 4001 open can act as a bootnode for others. Share its
multiaddr (`/ip4/<public-ip>/tcp/4001`) in the [Discord](https://discord.gg/K3w9nbUw4N) or list it through the
[operator registration](https://boing.network/testnet/register/operator) page. The peer id stays stable because
it is stored in `<data-dir>/p2p_identity.key`.
