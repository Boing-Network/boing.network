//! Print the **nonce-derived** contract `AccountId` for a deploy (no CREATE2 salt).
//!
//! Matches `boing_primitives::nonce_derived_contract_address` — the same rule used when submitting
//! `ContractDeployWithPurpose` / related deploy payloads **without** `create2_salt` (see
//! `native_amm_rpc_happy_path` in `boing-node`).
//!
//! ```bash
//! cargo run -p boing-primitives --example nonce_derived_contract_address -- \
//!   0x0123abcd...64_hex_chars 0
//! ```
//!
//! Use the deployer's **32-byte account id** and the **nonce on the deploy transaction** (the
//! account nonce at the time that tx is included). After deploy you can confirm with
//! `boing_getContractStorage` against the printed address.

use boing_primitives::{nonce_derived_contract_address, AccountId};
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

fn main() {
    init_tracing();

    let mut args = std::env::args().skip(1);
    let sender_hex = args.next().unwrap_or_else(|| {
        boing_telemetry::component_error(
            "boing_primitives::examples::nonce_derived_contract_address",
            "example",
            "missing_args",
            "missing deployer hex",
        );
        eprintln!(
            "usage: cargo run -p boing-primitives --example nonce_derived_contract_address -- \\
  <deployer_0x+64_hex> <deploy_nonce_u64>"
        );
        std::process::exit(2);
    });
    let nonce_str = args.next().unwrap_or_else(|| {
        boing_telemetry::component_error(
            "boing_primitives::examples::nonce_derived_contract_address",
            "example",
            "missing_args",
            "missing deploy nonce",
        );
        eprintln!("missing <deploy_nonce_u64>");
        std::process::exit(2);
    });

    let nonce: u64 = nonce_str.parse().unwrap_or_else(|_| {
        boing_telemetry::component_error(
            "boing_primitives::examples::nonce_derived_contract_address",
            "example",
            "nonce_parse_failed",
            format!("invalid u64: {nonce_str}"),
        );
        eprintln!("deploy nonce must be a decimal u64");
        std::process::exit(2);
    });

    let raw = hex::decode(sender_hex.trim_start_matches("0x").trim_start_matches("0X"))
        .unwrap_or_else(|e| {
            boing_telemetry::component_error(
                "boing_primitives::examples::nonce_derived_contract_address",
                "example",
                "deployer_hex_invalid",
                e.to_string(),
            );
            eprintln!("invalid deployer hex: {e}");
            std::process::exit(2);
        });
    if raw.len() != 32 {
        boing_telemetry::component_error(
            "boing_primitives::examples::nonce_derived_contract_address",
            "example",
            "deployer_wrong_length",
            format!("got {} bytes", raw.len()),
        );
        eprintln!(
            "deployer must be 32 bytes (64 hex chars), got {} bytes",
            raw.len()
        );
        std::process::exit(2);
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&raw);
    let sender = AccountId(bytes);
    let contract = nonce_derived_contract_address(&sender, nonce);
    println!("0x{}", hex::encode(contract.0));
}
