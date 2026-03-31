//! Print `0x`-hex of `constant_product_pool_bytecode()` for deploy / QA / docs.
//!
//! ```text
//! cargo run -p boing-execution --example dump_native_amm_pool
//! ```

fn main() {
    let code = boing_execution::constant_product_pool_bytecode();
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
    eprintln!("// bytecode length: {} bytes", code.len());
}
