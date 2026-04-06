//! Print canonical **native DEX ledger router** bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_native_dex_ledger_router
//! ```

fn main() {
    let code = boing_execution::native_dex_ledger_router_bytecode();
    eprintln!("native_dex_ledger_router_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
