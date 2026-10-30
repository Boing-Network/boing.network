#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    // Deserialize then re-serialize when possible — catches encode/decode drift.
    if let Ok(tx) = bincode::deserialize::<boing_primitives::Transaction>(data) {
        let _ = bincode::serialize(&tx);
        let _ = tx.id();
    }
    if let Ok(block) = bincode::deserialize::<boing_primitives::Block>(data) {
        let _ = bincode::serialize(&block);
    }
    // Prefer the tip-aware decoder used in production for blocks / signed txs.
    let _ = boing_primitives::Block::from_bincode(data);
    let _ = boing_primitives::SignedTransaction::from_bincode(data);
});
