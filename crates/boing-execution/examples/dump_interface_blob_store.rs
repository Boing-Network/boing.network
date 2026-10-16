//! Print interface blob store bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_interface_blob_store
//! ```

fn main() {
    let code = boing_execution::interface_blob_store_bytecode();
    eprintln!("interface_blob_store_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
