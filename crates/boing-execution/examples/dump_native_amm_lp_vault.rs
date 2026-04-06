//! Print canonical **native AMM LP vault** bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_native_amm_lp_vault
//! ```

fn main() {
    let code = boing_execution::native_amm_lp_vault_bytecode();
    eprintln!("native_amm_lp_vault_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
