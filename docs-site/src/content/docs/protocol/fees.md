---
title: Fees & denomination
description: 18-decimal BOING, micro execution fees, and optional priority tips on public testnet.
---

## Denomination

Public testnet uses **18-decimal** ledger units. From `native_currency.base_units_from_height` (**1050** on chain 6913), amounts in RPC responses are decimal **strings** in base units (`1 BOING = 10^18`).

Always read `boing_getNetworkInfo.native_currency` rather than hardcoding historical whole-BOING behavior from early heights.

## Fee policy (live)

Call **`boing_getNetworkFeePolicy`**. A recent public-testnet snapshot included:

| Field | Snapshot |
|-------|----------|
| `gas_price_base_units` | `"1000000000000"` |
| `execution_gas_fee_bps` | `100` (micro fee path) |
| `optional_priority_tips` | `true` |
| `tip_proposer_bps` | `10000` (tips to proposer by default) |
| Blob / pin fee knobs | Present; independent of execution BPS |

Product intent on the node: a **tiny always-on execution fee** (spam cost) plus **optional `priority_tip`** for preferred inclusion under congestion. Waiving gas (`execution_gas_fee_bps = 0`) is supported but not recommended.

Exact formulas are returned on the method (`fee_formula`, `fee_formula_base_units`). Specialist write-up: [OPTIONAL-PRIORITY-FEES.md](https://github.com/Boing-Network/boing.network/blob/main/docs/OPTIONAL-PRIORITY-FEES.md). Related printable materials: [PDF downloads](/resources/pdfs/).

## Signing note

Zero tip uses the v2 chain-bound digest. Non-zero tip uses the priority signing domain so relays cannot alter the tip. Encoding SoT: [BOING-SIGNED-TRANSACTION-ENCODING.md](https://github.com/Boing-Network/boing.network/blob/main/docs/BOING-SIGNED-TRANSACTION-ENCODING.md).
