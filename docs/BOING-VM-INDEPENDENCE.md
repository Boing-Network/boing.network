# Boing VM — independence from other networks

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


Boing’s execution layer is **only** the **Boing VM**: a stack machine defined in this repository and in [`TECHNICAL-SPECIFICATION.md`](TECHNICAL-SPECIFICATION.md) §7. The node does **not** embed or call another chain’s bytecode engine as its execution runtime.

## What this means

| Area | Boing |
|------|--------|
| **Execution** | Transactions run in `crates/boing-execution` (`Interpreter`, opcodes in `bytecode.rs`). |
| **Dependencies** | The Rust workspace **does not** pull in foreign VM implementations (e.g. `revm`) or chain-specific execution crates as libraries (verify with `Cargo.toml` / `Cargo.lock`). |
| **Semantics** | Opcode **meaning** and **gas** are **Boing-defined** in the spec and code. They are not delegated to an external VM spec. |
| **Bytes** | Some opcode **single-byte values** may resemble those used elsewhere historically; that is a **protocol choice for familiarity**, not a promise of bytecode or ABI compatibility with other networks. |

## QA and tooling

- **`boing-qa`** validates deploy bytecode against the **Boing** opcode set and well-formedness rules only.
- **Assemblers / IR** (`tools/boing-vm-assemble.mjs`, `tools/boing-vm-transpile-ir.mjs`) target **Boing** encodings from [`bytecode.rs`](../crates/boing-execution/src/bytecode.rs).

## Docs that compare to other ecosystems

Some documents (e.g. capability roadmap) mention other ecosystems **only as informal comparisons** (“what users expect elsewhere”). Those comparisons are **not** normative: the **source of truth** for behavior is always Boing specs + this codebase.

## Related

- [BOING-INFRASTRUCTURE-INDEPENDENCE.md](BOING-INFRASTRUCTURE-INDEPENDENCE.md) — chain, signing, RPC, and hosting independence.
- [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) — wallet is Boing-native (Ed25519, `boing_*` RPC).
