# Boing mini-IR (subset transpiler — T3)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


**Status:** version 1 — structural JSON that transpiles to Boing VM bytecode (`crates/boing-execution/src/bytecode.rs`). This is **not** a single high-level contract language or a full HLL; it is a machine-readable layer between hand-written asm ([`tools/boing-vm-assemble.mjs`](../tools/boing-vm-assemble.mjs)) and future richer compilers.

**Tool:** [`tools/boing-vm-transpile-ir.mjs`](../tools/boing-vm-transpile-ir.mjs)

---

## Goals

- Deterministic, easy-to-generate IR from tests, codegen experiments, or small DSL front-ends.
- Same opcode semantics and bytes as the VM spec ([`TECHNICAL-SPECIFICATION.md`](TECHNICAL-SPECIFICATION.md) §7).
- Explicit **labels** and **`push_jumpdest`** so control flow does not require manual PC math.

**Non-goals (v1):** optimization, register allocation, bytecode compatibility with non-Boing VMs, verifying gas (use the spec gas table).

---

## Document shape

```json
{
  "version": 1,
  "ops": [ /* ordered list of op objects */ ]
}
```

Each element of `ops` is a JSON object with **exactly one** key (the operation). Keys are **lowercase** snake-style names below.

---

## Operations

### Labels

| Form | Meaning |
|------|--------|
| `{ "label": "<name>" }` | Bind `<name>` to the **byte offset** of the **next** emitted instruction (not an opcode; emits 0 bytes). |

### Stack / ALU (no immediate)

`true` may be replaced by `{}` in JSON; the transpiler accepts both.

`stop`, `add`, `sub`, `mul`, `div`, `mod`, `addmod`, `mulmod`, `lt`, `gt`, `eq`, `iszero`, `and`, `or`, `xor`, `not`, `shl`, `shr`, `sar`, `dup1`, `address`, `caller`, `mload`, `mstore`, `sload`, `sstore`, `jump`, `jumpi`, `log0`, `log1`, `log2`, `log3`, `log4`, `return`

### Immediate pushes

| Form | Meaning |
|------|--------|
| `{ "push1": <n> }` | `n` integer 0–255. |
| `{ "push32": "<hex>" }` | 64 hex digits (32-byte big-endian word); optional `0x` prefix. |
| `{ "push": { "n": <2..31>, "hex": "<hex>" } }` | `n`-byte big-endian immediate (`2×n` hex digits). |

### Jumps

| Form | Meaning |
|------|--------|
| `{ "push_jumpdest": "<label>" }` | Emits **`PUSH32`** of the resolved **byte offset** of `<label>` (32-byte big-endian, zero-padded). You typically follow with `jump` or `jumpi`. |

Offsets are computed after layout; **forward** and **backward** references are allowed. All labels referenced in `push_jumpdest` must appear somewhere in `ops` before the file ends (the label may be defined after the jump — patch pass).

---

## Deploy init code

To use [init-code deploy](TECHNICAL-SPECIFICATION.md) (`0xFD` prefix), prepend the marker in your pipeline **after** transpile (hex or bytes), or add a dedicated step in your bundler. The transpiler outputs **raw** VM bytecode only.

---

## Example

See [`tools/examples/mini-ir-stop.json`](../tools/examples/mini-ir-stop.json) and [`tools/examples/mini-ir-caller-return.json`](../tools/examples/mini-ir-caller-return.json).

```bash
node tools/boing-vm-transpile-ir.mjs tools/examples/mini-ir-stop.json
# → 0x00
```

---

## Versioning

Bump `"version"` when breaking IR shape. The transpiler rejects unknown `version`.

---

## See also

- [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) — Phase 4 T3
- [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) — Track V (opcode source of truth)
