# Boing Fuzz Testing

Fuzz targets for the Boing blockchain. Uses [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and [libFuzzer](https://llvm.org/docs/LibFuzzer.html).

## Prerequisites

1. Install Rust nightly: `rustup toolchain install nightly`
2. Install cargo-fuzz: `cargo install cargo-fuzz`

## Running

The fuzz crate is not a workspace member, so `cargo build` at the project root is unaffected.

**Option A** — from the `fuzz/` directory (uses `rust-toolchain.toml` for nightly):

```bash
cd fuzz
cargo fuzz run fuzz_target_1 -- -runs=1000
```

**Option B** — from the project root (use nightly explicitly):

```bash
rustup run nightly cargo fuzz run fuzz_target_1 -- -runs=1000
```

Run continuously (Ctrl+C to stop):

```bash
cd fuzz && cargo fuzz run fuzz_target_1
```

> **Note:** On Windows, if you see `STATUS_DLL_NOT_FOUND` when running the fuzz target, ensure the Visual C++ Redistributable is installed and that the build completed successfully.

## Targets

- **fuzz_target_1**: Fuzzes bincode deserialization of `Block` and `Transaction` from boing-primitives.
- **fuzz_bytecode_decode**: Opcode byte decode + PUSH size helpers (`boing-execution`).
- **fuzz_tx_roundtrip**: Deserialize → serialize round-trip plus `from_bincode` paths.
- **fuzz_qa_match**: Scam-pattern / deploy QA matcher (`boing-qa`).

## Adding Targets

```bash
cd fuzz && cargo fuzz add <target_name>
```

Then edit `fuzz/fuzz_targets/<target_name>.rs`.
