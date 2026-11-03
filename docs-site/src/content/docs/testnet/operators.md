---
title: Operators
description: Public testnet RPC hosting, activation knobs, and where the deep runbooks live.
---

## What runs production RPC today

| Piece | Role |
|-------|------|
| Fly **`boing-testnet-1`** | Hosted validator / RPC origin |
| Cloudflare Worker **`public-rpc-gateway`** | Public HTTPS front door at `testnet-rpc.boing.network` |
| Pages **`boing-network`** | Marketing site + portal |
| Pages **`boing-docs`** | This documentation site |

`boing-testnet-2` was retired. Do not point the public hostname at a home Cloudflare Tunnel.

## Operator checklist (high level)

1. Run a matching `boing-node` binary across validators.
2. Keep activation env identical (`BOING_*_ACTIVATION_HEIGHT`, media blob heights, QA canonical, etc.).
3. Restart after env-only changes; redeploy after binary changes.
4. Verify with `boing_health` / `boing_getNetworkInfo` / `boing_getMediaBlobParams`.
5. Configure CORS for browser origins you support.
6. Protect operator RPCs with `BOING_OPERATOR_RPC_TOKEN` (required on planned mainnet 6914).

## Deep runbooks (not duplicated here)

| Topic | Location |
|-------|----------|
| Operator hub | [TESTNET-RPC-INFRA.md](https://github.com/Boing-Network/boing.network/blob/main/docs/TESTNET-RPC-INFRA.md) |
| Fly | [FLY-IO.md](https://github.com/Boing-Network/boing.network/blob/main/docs/FLY-IO.md) |
| Runbook | [RUNBOOK.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RUNBOOK.md) · [PDF](https://boing.network/pdfs/RUNBOOK.pdf) |
| Env catalog | [ENVIRONMENT-VARIABLES.md](https://github.com/Boing-Network/boing.network/blob/main/docs/ENVIRONMENT-VARIABLES.md) |
| Public testnet env example | `tools/boing-node-public-testnet.env.example` |
| Networks API | [NETWORKS-API.md](https://github.com/Boing-Network/boing.network/blob/main/docs/NETWORKS-API.md) |

Native DEX bootstrap is a **manual** tutorial/orchestrator path (`examples/native-boing-tutorial`); the node does not auto-seed pools.
