---
title: Host public RPC
description: How the Boing public testnet RPC is hosted, and the checklist for operators who serve RPC to others.
---

This page is for operators who serve JSON-RPC to wallets and apps. To run a node for yourself, start with
[Run a full node](/nodes/run-a-node/).

## How the public testnet is hosted

| Piece | Role |
|-------|------|
| Fly.io **`boing-testnet-1`** | Validator, bootnode, and RPC origin |
| Cloudflare Worker **`public-rpc-gateway`** | Public HTTPS entry point at `testnet-rpc.boing.network` |
| Cloudflare Pages **`boing-network`** | Website and testnet portal |
| Cloudflare Pages **`boing-docs`** | This documentation site |

`boing-testnet-2` was retired on 2026-10-24. The `testnet-rpc.boing.network` hostname always points at the Worker,
never at a home tunnel.

## Checklist for an RPC node

1. Run the same `boing-node` build on every node you operate.
2. Leave consensus heights to the testnet preset. Only set the four
   [consensus settings](/nodes/run-a-node/#install-and-start).
3. Restart after changing environment variables; redeploy after changing the binary.
4. Verify with `boing_health`, `boing_getNetworkInfo`, and `boing_getMediaBlobParams`.
5. Allow the browser origins you serve with `BOING_RPC_CORS_EXTRA_ORIGINS`.
6. Set `BOING_OPERATOR_RPC_TOKEN` if you need operator methods. They are disabled without it, and it is required
   on the planned mainnet (6914).
7. Put a rate limit or WAF in front of the node
   ([PUBLIC-RPC-WAF-GUIDANCE.md](https://github.com/Boing-Network/boing.network/blob/main/docs/PUBLIC-RPC-WAF-GUIDANCE.md)).

Native DEX pools are not created automatically. Operators deploy them with the scripts in
[`examples/native-boing-tutorial`](https://github.com/Boing-Network/boing.network/tree/main/examples/native-boing-tutorial).

## Runbooks

| Topic | Document |
|-------|----------|
| Operator hub | [TESTNET-RPC-INFRA.md](https://github.com/Boing-Network/boing.network/blob/main/docs/TESTNET-RPC-INFRA.md) |
| Fly.io hosting and upgrades | [FLY-IO.md](https://github.com/Boing-Network/boing.network/blob/main/docs/FLY-IO.md) |
| Runbook | [RUNBOOK.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RUNBOOK.md) · [PDF](https://boing.network/pdfs/RUNBOOK.pdf) |
| All environment variables | [ENVIRONMENT-VARIABLES.md](https://github.com/Boing-Network/boing.network/blob/main/docs/ENVIRONMENT-VARIABLES.md) |
| Copy-paste commands | [NODE-OPERATOR-COMMANDS.md](https://github.com/Boing-Network/boing.network/blob/main/docs/NODE-OPERATOR-COMMANDS.md) |
| systemd, Docker Compose, Kubernetes examples | [`tools/`](https://github.com/Boing-Network/boing.network/tree/main/tools) |
| Node listing API | [NETWORKS-API.md](https://github.com/Boing-Network/boing.network/blob/main/docs/NETWORKS-API.md) |
