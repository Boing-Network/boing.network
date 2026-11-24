---
title: Troubleshooting
description: Fixes for Boing testnet nodes that stop syncing, fork at an activation height, or cannot find peers.
---

Start with the symptom that matches what you see. Compare your node with the public RPC first:

```bash
curl -s -X POST http://127.0.0.1:8545 -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_getSyncState","params":[]}'
curl -s -X POST https://testnet-rpc.boing.network/ -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_getSyncState","params":[]}'
```

## Stuck at height 0

**Cause:** nodes keep only the most recent 100,000 blocks in memory. Once the testnet passed that, the bootnode
answered "not found" when a fresh node asked for block 1, so the new node stayed at height 0 without logging an
error. The fix lets the serving node read old blocks from disk; it needs to be running on the **bootnode**, not on
your node.

**What to do:**

1. Make sure your configuration only lists the [current bootnode](/nodes/bootnodes/).
2. Restart your node. Current builds log `block_sync_not_found` or `block_sync_request_failed` (at most every
   30 seconds) when a peer can't serve a block, so you can see what is happening.
3. If you still see `block_sync_not_found` for low heights, report it in [Discord](https://discord.gg/K3w9nbUw4N).

While catching up, log lines like `Block does not chain to our tip` for new blocks are expected and harmless.

## Stops at a specific height

The chain switched on new rules at fixed heights. A node whose binary or settings don't include a rule computes a
different state root at that height and stops following the chain (often with `state root mismatch`).

| Stops at | Usual cause | Fix |
|----------|-------------|-----|
| At or after 400 or 640 | Missing `BOING_VALIDATOR_SET=open`, `BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT=400` or `BOING_NAME_REGISTRY_ACTIVATION_HEIGHT=640` | Set all four [consensus settings](/nodes/run-a-node/#install-and-start) |
| 1000, 1050 or 1150 | Release older than `testnet-v0.1.18` | Upgrade |
| 1187 or 1188 | Release up to `testnet-v0.1.18` (no testnet preset) | Upgrade to `testnet-v0.1.24` or newer |
| 1312 | Old build that rejects block-sync responses over 10 MiB | Upgrade |
| 1500 | Older than `testnet-v0.1.21`, or `BOING_STORAGE_TREE_V2_ACTIVATION_HEIGHT` set to anything except 1500 | Upgrade and unset the variable |
| 1600 | Older than `testnet-v0.1.22` | Upgrade |
| 1700 | Older than `testnet-v0.1.24` | Upgrade |

After upgrading, start the node again on the same data directory. It continues from its stored height. If it
still won't move past the height, stop it, delete the data directory (keep a copy of `p2p_identity.key` if you
want the same peer id), and sync again.

`testnet-v0.1.20` was withdrawn. Don't use it.

## Slow at blocks 1311 to 1317

These blocks hold large media uploads, up to about 160 MiB each. Current builds accept block-sync responses up to
512 MiB and wait up to 120 seconds per request. Give it a few minutes. If your machine is short on memory,
expect a few hundred MB of extra use while it passes these blocks.

The limits are `BOING_P2P_BLOCK_SYNC_MAX_RESPONSE_BYTES` and `BOING_P2P_BLOCK_SYNC_TIMEOUT_SECS`. You should not
need to change them.

## No peers

- Check that you can reach the bootnode: `nc -vz 169.155.48.188 4001`.
- Check that `--p2p-listen /ip4/0.0.0.0/tcp/4001` is set. Without it, P2P is off.
- Open inbound TCP 4001 in your firewall and on your router or cloud security group so other nodes can dial you.
- Remove [retired bootnodes](/nodes/bootnodes/#retired-addresses) from your configuration.

## Peer id changes on every restart

Your node can't keep `<data-dir>/p2p_identity.key`. Make sure `--data-dir` points at a persistent, writable
directory (a mounted volume in Docker). A corrupt key file stops startup on purpose; move it aside to get a new
peer id.

## Browser apps can't reach my RPC

Browsers need CORS headers from your node. Add your app's origin with `BOING_RPC_CORS_EXTRA_ORIGINS`, or call the
node through a same-origin proxy. See
[ENVIRONMENT-VARIABLES.md](https://github.com/Boing-Network/boing.network/blob/main/docs/ENVIRONMENT-VARIABLES.md).

## Still stuck?

Ask in the [Discord](https://discord.gg/K3w9nbUw4N) with your node's height, the last 50 lines of its log, the
release or commit you run, and your start command (without keys).
