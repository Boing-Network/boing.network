//! Print builder attestation registry bytecode (hex).
//!
//! ```bash
//! cargo run -p boing-execution --example dump_builder_attestation_registry
//! ```

fn main() {
    let code = boing_execution::builder_attestation_registry_bytecode();
    eprintln!("builder_attestation_registry_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
