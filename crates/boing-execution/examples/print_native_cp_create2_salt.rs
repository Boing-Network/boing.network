//! Print **`0x` + hex** of native CP pool CREATE2 salts (v1–v7).
//!
//! ```bash
//! cargo run -p boing-execution --example print_native_cp_create2_salt
//! ```

fn main() {
    for (label, s) in [
        ("NATIVE_CP_POOL_CREATE2_SALT_V1", boing_execution::NATIVE_CP_POOL_CREATE2_SALT_V1),
        ("NATIVE_CP_POOL_CREATE2_SALT_V2", boing_execution::NATIVE_CP_POOL_CREATE2_SALT_V2),
        ("NATIVE_CP_POOL_CREATE2_SALT_V3", boing_execution::NATIVE_CP_POOL_CREATE2_SALT_V3),
        ("NATIVE_CP_POOL_CREATE2_SALT_V4", boing_execution::NATIVE_CP_POOL_CREATE2_SALT_V4),
        ("NATIVE_CP_POOL_CREATE2_SALT_V5", boing_execution::NATIVE_CP_POOL_CREATE2_SALT_V5),
        ("NATIVE_CP_POOL_CREATE2_SALT_V6", boing_execution::NATIVE_CP_POOL_CREATE2_SALT_V6),
        ("NATIVE_CP_POOL_CREATE2_SALT_V7", boing_execution::NATIVE_CP_POOL_CREATE2_SALT_V7),
    ] {
        print!("{label}=");
        print!("0x");
        for b in s {
            print!("{b:02x}");
        }
        println!();
    }
}
