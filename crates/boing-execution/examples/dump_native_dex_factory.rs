//! Print canonical **native DEX pair directory** bytecode (hex) for deploy scripts / docs.
//!
//! ```bash
//! cargo run -p boing-execution --example dump_native_dex_factory
//! ```

fn main() {
    let code = boing_execution::native_dex_factory_bytecode();
    eprintln!("native_dex_factory_bytecode: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
