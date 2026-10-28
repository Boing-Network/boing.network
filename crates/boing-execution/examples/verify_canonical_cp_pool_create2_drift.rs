//! Compare the **operator-published** canonical testnet CP pool id (docs / `boing-sdk`) with the
//! **predicted** CREATE2 address from **this tree's** [`constant_product_pool_bytecode`] and
//! [`NATIVE_CP_POOL_CREATE2_SALT_V1`] for the documented deployer.
//!
//! If bytecode changes after a pool is frozen on-chain, predicted and published ids **diverge**
//! until ops rotates the published constant. Run this before claiming CREATE2 parity in docs.
//!
//! ```bash
//! cargo run -p boing-execution --example verify_canonical_cp_pool_create2_drift
//! BOING_STRICT_CP_POOL_CREATE2=1 cargo run -p boing-execution --example verify_canonical_cp_pool_create2_drift
//! ```

use boing_execution::{constant_product_pool_bytecode, NATIVE_CP_POOL_CREATE2_SALT_V1};
use boing_primitives::{create2_contract_address, AccountId};

/// Same as [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](../../docs/OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published.
const OPS_DEPLOYER_HEX: &str = "c063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f";

/// Published canonical pool `AccountId` (live testnet / SDK mirror).
const PUBLISHED_POOL_HEX: &str = "ce4f819369630e89c4634112fdf01e1907f076bc30907f0402591abfca66518d";

fn account_from_64_hex(s: &str) -> AccountId {
    let v = hex::decode(s).expect("deployer/published hex must decode");
    assert_eq!(v.len(), 32, "expected 32-byte account id");
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    AccountId(a)
}

fn main() {
    let deployer = account_from_64_hex(OPS_DEPLOYER_HEX);
    let bytecode = constant_product_pool_bytecode();
    let predicted = create2_contract_address(&deployer, &NATIVE_CP_POOL_CREATE2_SALT_V1, &bytecode);
    let predicted_hex = hex::encode(predicted.0);
    let published_lower = PUBLISHED_POOL_HEX.to_ascii_lowercase();
    let matches = predicted_hex == published_lower;

    println!(
        "{}",
        serde_json::json!({
            "ok": true,
            "published_pool_hex": format!("0x{}", published_lower),
            "predicted_create2_v1_hex": format!("0x{}", predicted_hex),
            "deployer_hex": format!("0x{}", OPS_DEPLOYER_HEX),
            "bytecode_len": bytecode.len(),
            "create2_matches_published": matches,
            "note": if matches {
                "Repo bytecode CREATE2 matches published canonical pool id."
            } else {
                "Drift: published id is still the live contract to use for RPC reads; fresh CREATE2 deploy from this bytecode would land at predicted_create2_v1_hex. Rotate docs/SDK after ops deploys there, or restore bytecode parity with the published deployment."
            }
        })
    );

    if !matches {
        eprintln!(
            "verify_canonical_cp_pool_create2_drift: CREATE2 prediction does not match published canonical pool id (see JSON above)."
        );
        if std::env::var("BOING_STRICT_CP_POOL_CREATE2")
            .ok()
            .as_deref()
            == Some("1")
        {
            std::process::exit(1);
        }
    }
}
