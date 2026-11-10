---
title: Network & activations
description: Public testnet identity, tip fields, and activation heights from the live boing-node RPC.
---

Always treat **`boing_getNetworkInfo`** (and **`boing_health`**) as the live source. The table below reflects a probe of `https://testnet-rpc.boing.network/` and will drift as the tip moves.

## Identity

| Field | Value (public testnet) |
|-------|-------------------------|
| `chain_id` | **6913** |
| `chain_id` hex | **`0x1b01`** (SDK: `BOING_TESTNET_CHAIN_ID_HEX`) |
| `chain_name` | Boing Testnet |
| Public RPC | `https://testnet-rpc.boing.network/` |
| Consensus model (reported) | `single_round_quorum_commit` |
| Native symbol | **BOING**, 18 decimals |

Mainnet **6914** is listed in `docs/chain-metadata.json` as **planned** with empty `rpc_urls`.

## Tip fields

From the same method: `head_height`, `finalized_height`, `latest_block_hash`, plus `chain_native` aggregates (`account_count`, `total_balance`, `total_stake`, …). Use these for dashboards — do not invent circulating-supply definitions from them (`rpc.not_available` currently includes `staking_apy`).

## Gated heights (testnet 6913)

Reported by the public node at probe time:

| Knob | Height | Meaning (short) |
|------|--------|-----------------|
| Native base units | **1050** (`native_currency.base_units_from_height`) | Ledger uses 18-decimal base units from this height |
| Media blob admission | **1200** (`boing_getMediaBlobParams.activationHeight`) | Put/Announce path active |
| PHASE7 256 MiB cap | **1306** (`phase7CapActivationHeight`) | Cap active at tip when `phase7CapActiveAtTip` is true |
| Account tree v3 | **1500** | Full 256-bit account-key routing |
| VM hardening | **1500** (`vm_hardening_activation_height`) | Hardening rules from this height |
| Equivocation slash | **1600** | Slash path for equivocation |
| VM jump strict | **1600** | Strict jump rules |
| QA canonical | **1700** | On-chain QA policy / public Unsure pool semantics |

`storage_tree_v2_activation_height` may be **`null`** on the public node (legacy 64-bit contract-storage routing until configured). Missing keys on older nodes mean “unknown” — re-probe.

Operators set many of these via `BOING_*` env / CLI flags; mismatched heights across validators fork admission. See [Operators](/testnet/operators/).

## Client hints

`client_hints` on network info / health (advisory only):

- `min_wallet_version`: **0.1.18**
- `recommended_wallet_version`: **0.1.26**
- `recommended_sdk`: **0.5.41**
- `sdk_npm_package`: **`@boing-network/boing-sdk`**

## Machine-readable pointers

- Repo metadata: [chain-metadata.json](https://github.com/Boing-Network/boing.network/blob/main/docs/chain-metadata.json)
- HTTP discovery: `/.well-known/boing-rpc`, `/openapi.json`, `/live`, `/ready`
