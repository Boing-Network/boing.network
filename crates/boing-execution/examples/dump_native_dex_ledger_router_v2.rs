//! Print canonical **native DEX ledger router v2** bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_native_dex_ledger_router_v2
//! ```

fn main() {
    let code = boing_execution::native_dex_ledger_router_bytecode_v2();
    eprintln!("native_dex_ledger_router_bytecode_v2: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
