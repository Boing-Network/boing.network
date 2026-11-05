//! Print create-and-seed factory bytecode hex + length.
fn main() {
    let code = boing_execution::native_dex_create_and_seed_bytecode();
    println!("// create_and_seed_factory_v1: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
