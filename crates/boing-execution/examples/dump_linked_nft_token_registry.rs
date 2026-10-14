//! Print linked NFT↔token registry bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_linked_nft_token_registry
//! ```

fn main() {
    let code = boing_execution::linked_nft_token_registry_bytecode();
    eprintln!("linked_nft_token_registry_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
