//! Print **predicted CREATE2** `AccountId`s for native DEX aux contracts for a **deployer** `AccountId`.
//!
//! **Deployer resolution (first match):**
//! 1. Env **`CANONICAL_BOING_TESTNET_DEPLOYER_HEX`**
//! 2. CLI **`--deployer 0x…`**
//! 3. Legacy in-tree default (kept in sync by `sync-canonical-testnet-from-manifest.mjs`)
//!
//! ```bash
//! cargo run -p boing-execution --example print_canonical_testnet_dex_create2_addresses
//! cargo run -p boing-execution --example print_canonical_testnet_dex_create2_addresses -- --json
//! cargo run -p boing-execution --example print_canonical_testnet_dex_create2_addresses -- --deployer 0x... --json
//! ```

use boing_execution::{
    lp_share_token_bytecode, native_amm_lp_vault_bytecode, native_dex_factory_bytecode,
    native_dex_ledger_router_bytecode, native_dex_ledger_router_bytecode_v2,
    native_dex_ledger_router_bytecode_v3, native_dex_multihop_swap_router_bytecode,
    NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1, NATIVE_DEX_FACTORY_CREATE2_SALT_V1,
    NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1, NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2,
    NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3, NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1,
    NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1,
};
use boing_primitives::{create2_contract_address, AccountId};

/// Default deployer bytes — **update** via `scripts/sync-canonical-testnet-from-manifest.mjs` after rotations.
fn default_canonical_testnet_dex_deployer() -> AccountId {
    AccountId([
        0xc0, 0x63, 0x51, 0x2f, 0x42, 0x86, 0x8f, 0x12, 0x78, 0xc5, 0x9a, 0x1f, 0x61, 0xec, 0x09,
        0x44, 0x78, 0x5c, 0x30, 0x4d, 0xbc, 0x48, 0xde, 0xc7, 0xe4, 0xc4, 0x1f, 0x70, 0xf6, 0x66,
        0x73, 0x3f,
    ])
}

fn parse_deployer_hex(s: &str) -> AccountId {
    let raw = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    let v = hex::decode(raw).unwrap_or_else(|e| {
        eprintln!("invalid deployer hex: {e}");
        std::process::exit(2);
    });
    if v.len() != 32 {
        eprintln!("deployer must be 32 bytes, got {}", v.len());
        std::process::exit(2);
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    AccountId(a)
}

fn resolve_deployer_from_env_and_args() -> AccountId {
    if let Ok(h) = std::env::var("CANONICAL_BOING_TESTNET_DEPLOYER_HEX") {
        return parse_deployer_hex(&h);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--deployer" {
            if let Some(h) = args.get(i + 1) {
                return parse_deployer_hex(h);
            }
            eprintln!("--deployer requires a 0x + 64 hex argument");
            std::process::exit(2);
        }
        i += 1;
    }
    default_canonical_testnet_dex_deployer()
}

fn wants_json() -> bool {
    std::env::args().any(|a| a == "--json")
}

fn print_row(label: &str, addr: &AccountId) {
    println!("{label}\t0x{}", hex::encode(addr.0));
}

fn main() {
    let deployer = resolve_deployer_from_env_and_args();
    let json = wants_json();

    let factory = create2_contract_address(
        &deployer,
        &NATIVE_DEX_FACTORY_CREATE2_SALT_V1,
        &native_dex_factory_bytecode(),
    );
    let lr1 = create2_contract_address(
        &deployer,
        &NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1,
        &native_dex_ledger_router_bytecode(),
    );
    let lr2 = create2_contract_address(
        &deployer,
        &NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2,
        &native_dex_ledger_router_bytecode_v2(),
    );
    let lr3 = create2_contract_address(
        &deployer,
        &NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3,
        &native_dex_ledger_router_bytecode_v3(),
    );
    let mh = create2_contract_address(
        &deployer,
        &NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1,
        &native_dex_multihop_swap_router_bytecode(),
    );
    let vault = create2_contract_address(
        &deployer,
        &NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1,
        &native_amm_lp_vault_bytecode(),
    );
    let share = create2_contract_address(
        &deployer,
        &NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1,
        &lp_share_token_bytecode(),
    );

    if json {
        println!(
            "{}",
            serde_json::json!({
                "deployer": format!("0x{}", hex::encode(deployer.0)),
                "native_dex_factory": format!("0x{}", hex::encode(factory.0)),
                "native_dex_ledger_router_v1": format!("0x{}", hex::encode(lr1.0)),
                "native_dex_ledger_router_v2": format!("0x{}", hex::encode(lr2.0)),
                "native_dex_ledger_router_v3": format!("0x{}", hex::encode(lr3.0)),
                "native_dex_multihop_swap_router": format!("0x{}", hex::encode(mh.0)),
                "native_amm_lp_vault": format!("0x{}", hex::encode(vault.0)),
                "native_lp_share_token": format!("0x{}", hex::encode(share.0)),
            })
        );
        return;
    }

    println!("# Native DEX CREATE2 — deployer:");
    print_row("deployer", &deployer);
    println!("# Predicted CREATE2 (deploy matching bytecode + salt to materialize on-chain):");
    print_row("native_dex_factory", &factory);
    print_row("native_dex_ledger_router_v1", &lr1);
    print_row("native_dex_ledger_router_v2", &lr2);
    print_row("native_dex_ledger_router_v3", &lr3);
    print_row("native_dex_multihop_swap_router", &mh);
    print_row("native_amm_lp_vault", &vault);
    print_row("native_lp_share_token", &share);
}
