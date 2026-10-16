//! Print interface registry bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_interface_registry
//! ```

fn main() {
    let code = boing_execution::interface_registry_bytecode();
    eprintln!("interface_registry_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}