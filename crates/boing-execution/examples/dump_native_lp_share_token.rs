//! Print canonical **native LP share token** bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_native_lp_share_token
//! ```

fn main() {
    let code = boing_execution::lp_share_token_bytecode();
    eprintln!("lp_share_token_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
