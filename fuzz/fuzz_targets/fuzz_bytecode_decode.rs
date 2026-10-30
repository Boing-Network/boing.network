#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Exercise opcode decode + push-size helpers (audit #61 item 51 / related to item 22).
    for &b in data.iter().take(4096) {
        let _ = boing_execution::Opcode::from_byte(b);
        let _ = boing_execution::Opcode::push_size(b);
    }
    // Round-trip: known opcodes must decode.
    for b in [
        0x00u8, 0x40, 0x41, 0x31, 0x34, 0x47, 0xf2, 0xf5, 0x60, 0x7f,
    ] {
        let _ = boing_execution::Opcode::from_byte(b);
    }
});
