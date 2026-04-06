//! Print `0x`-hex of native CP pool bytecode (**v1**–**v5**) for deploy / QA / docs.
//!
//! ```text
//! cargo run -p boing-execution --example dump_native_amm_pool
//! ```

fn main() {
    for (label, code) in [
        ("v1_ledger_only", boing_execution::constant_product_pool_bytecode()),
        ("v2_token_hooks", boing_execution::constant_product_pool_bytecode_v2()),
        ("v3_ledger_fee_bps", boing_execution::constant_product_pool_bytecode_v3()),
        ("v4_token_hooks_fee_bps", boing_execution::constant_product_pool_bytecode_v4()),
        ("v5_swap_to_recipient", boing_execution::constant_product_pool_bytecode_v5()),
    ] {
        eprintln!("// {label}: {} bytes", code.len());
        print!("0x");
        for b in &code {
            print!("{b:02x}");
        }
        println!();
    }
}
