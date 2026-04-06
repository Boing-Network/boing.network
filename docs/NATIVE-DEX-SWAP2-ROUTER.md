# Native DEX two-hop swap router (Boing VM)

**Superseded by** [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](./NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md) (**2–4** hops, same bytecode entrypoint).

Bytecode: `native_dex_swap2_router_bytecode()` (**alias** of `native_dex_multihop_swap_router_bytecode()`) in `crates/boing-execution/src/native_dex_multihop_swap_router.rs`.

## Purpose

Execute **two** native constant-product **`swap`** / **`swap_to`** calls **in one transaction**, without a general “batch” opcode. Each hop uses the same **`Call`** pattern as [NATIVE-DEX-LEDGER-ROUTER.md](./NATIVE-DEX-LEDGER-ROUTER.md); the pool sees **`Caller` = this router** for both hops.

## Calldata

| Mode | Selector (word0 low byte) | Outer size | Inner size per hop |
|------|---------------------------|------------|---------------------|
| Ledger-style | **`0xE5`** | **352** bytes | **128** (`swap` on v1 / v3 pools) |
| `swap_to` (v5) | **`0xE6`** | **416** bytes | **160** |

**Layout `0xE5` (128-byte inners):** word0 = selector; word1 = **pool1**; words 2–5 = **inner1**; word6 = **pool2**; words 7–10 = **inner2**.

**Layout `0xE6` (160-byte inners):** word0 = selector; word1 = **pool1**; words 2–6 = **inner1**; word7 = **pool2**; words 8–12 = **inner2**.

Encoders: `encode_swap2_router_calldata_128`, `encode_swap2_router_calldata_160` (`boing_execution` / `boing-sdk`).

## Economics and path validity

This router only **sequences** two VM calls. It does **not**:

- Guarantee a profitable or liquid path
- Move intermediate **reference tokens** between pools (native AMM **swap** input is **ledger reserve**–based; token-hook details are in [NATIVE-AMM-CALLDATA.md](./NATIVE-AMM-CALLDATA.md))

Integrators must ensure the **second** pool’s trade direction and reserves match the intended path (often: encode **`swap_to`** so hop 1 pays the router and hop 2 pays the end user).

## CREATE2

Salt: `NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1` (label `BOING_NATIVEDEX_MHOP_V1`). `NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1` is a **deprecated alias** (same bytes). Rust / `boing-sdk` `create2.ts`.

```bash
cargo run -p boing-execution --example dump_native_dex_swap2_router
```

## Relationship to “full DEX”

See [BOING-NATIVE-DEX-CAPABILITY.md](./BOING-NATIVE-DEX-CAPABILITY.md) for what Boing VM can and cannot do today. This artifact fills the **multi-hop swap in one tx** gap for up to **two** pools; three+ hops still require another program or multiple transactions.
