//! Print **`create2_contract_address(deployer, salt, bytecode)`** — must match VM deploy when
//! `create2_salt: Some(salt)` on a deploy payload.
//!
//! **Bytecode** is read from a file: trim whitespace, strip optional `0x`, decode as hex (full pool
//! bytecode is too large for argv). Example:
//!
//! ```bash
//! cargo run -p boing-execution --example dump_native_amm_pool | tr -d '\n\r' > /tmp/pool.hex
//! cargo run -p boing-primitives --example create2_contract_address -- \
//!   0x<DEPLOYER_64_HEX> 0x<SALT_64_HEX> /tmp/pool.hex
//! ```
//!
//! For the documented native CP pool salt v1, use the hex of **`NATIVE_CP_POOL_CREATE2_SALT_V1`**
//! from `boing_execution::native_amm` (see `cargo run -p boing-execution --example print_native_cp_create2_salt`).

use boing_primitives::{create2_contract_address, AccountId};
use std::fs;
use std::path::Path;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

fn decode_hex_32(s: &str) -> [u8; 32] {
    let raw = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    let v = hex::decode(raw).unwrap_or_else(|e| {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "decode_hex_32_failed",
            e.to_string(),
        );
        eprintln!("invalid hex: {e}");
        std::process::exit(2);
    });
    if v.len() != 32 {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "wrong_hex_length",
            format!("expected 32 bytes, got {}", v.len()),
        );
        eprintln!("expected 32 bytes (64 hex chars), got {} bytes", v.len());
        std::process::exit(2);
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    a
}

fn main() {
    init_tracing();

    let mut args = std::env::args().skip(1);
    let deployer_hex = args.next().unwrap_or_else(|| {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "missing_args",
            "missing deployer hex",
        );
        eprintln!(
            "usage: cargo run -p boing-primitives --example create2_contract_address -- \\
  <deployer_0x+64_hex> <salt_0x+64_hex> <path_to_bytecode_hex_file>"
        );
        std::process::exit(2);
    });
    let salt_hex = args.next().unwrap_or_else(|| {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "missing_args",
            "missing salt hex",
        );
        eprintln!("missing <salt_0x+64_hex>");
        std::process::exit(2);
    });
    let path = args.next().unwrap_or_else(|| {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "missing_args",
            "missing bytecode path",
        );
        eprintln!("missing <path_to_bytecode_hex_file>");
        std::process::exit(2);
    });

    let contents = fs::read_to_string(Path::new(&path)).unwrap_or_else(|e| {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "read_bytecode_file_failed",
            format!("{path}: {e}"),
        );
        eprintln!("read {}: {e}", path);
        std::process::exit(2);
    });
    let bc_raw = contents
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    let bytecode = hex::decode(bc_raw).unwrap_or_else(|e| {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "bytecode_hex_invalid",
            e.to_string(),
        );
        eprintln!("bytecode file: invalid hex: {e}");
        std::process::exit(2);
    });
    if bytecode.is_empty() {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "bytecode_empty",
            "bytecode file decoded to empty",
        );
        eprintln!("bytecode is empty");
        std::process::exit(2);
    }

    let deployer_bytes = hex::decode(
        deployer_hex
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X"),
    )
    .unwrap_or_else(|e| {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "deployer_hex_invalid",
            e.to_string(),
        );
        eprintln!("deployer hex: {e}");
        std::process::exit(2);
    });
    if deployer_bytes.len() != 32 {
        boing_telemetry::component_error(
            "boing_primitives::examples::create2_contract_address",
            "example",
            "deployer_wrong_length",
            format!("got {} bytes", deployer_bytes.len()),
        );
        eprintln!("deployer must be 32 bytes");
        std::process::exit(2);
    }
    let mut db = [0u8; 32];
    db.copy_from_slice(&deployer_bytes);
    let deployer = AccountId(db);

    let salt = decode_hex_32(&salt_hex);
    let contract = create2_contract_address(&deployer, &salt, &bytecode);
    println!("0x{}", hex::encode(contract.0));
}
