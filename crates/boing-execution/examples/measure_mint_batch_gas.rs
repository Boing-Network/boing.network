//! Measure `mint_batch` gas at production meters (SSTORE=20k).
//!
//! ```text
//! cargo run -p boing-execution --example measure_mint_batch_gas --release
//! ```
use boing_execution::{
    encode_mint_batch_calldata, encode_owner_of_calldata, reference_nft_collection_template_bytecode,
    Interpreter, GAS_PER_CONTRACT_CALL, MAX_REFERENCE_NFT_MINT_BATCH,
};
use boing_primitives::{Account, AccountId};
use boing_state::StateStore;
use std::time::Instant;

fn tid_word(n: u64) -> [u8; 32] {
    let mut t = [0u8; 32];
    t[24..].copy_from_slice(&n.to_be_bytes());
    t
}

fn hash_word(n: u64) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[24..].copy_from_slice(&n.to_be_bytes());
    h[0] = 0xaa;
    h
}

fn measure(n: usize, with_metadata: bool) -> Result<(u64, u128), String> {
    if n > MAX_REFERENCE_NFT_MINT_BATCH as usize {
        return Err(format!("n={n} > MAX={}", MAX_REFERENCE_NFT_MINT_BATCH));
    }
    let deployer = AccountId([0x11u8; 32]);
    let to = AccountId([0x22u8; 32]);
    let collection = AccountId([0x33u8; 32]);
    let mut state = StateStore::new();
    state.insert(Account {
        id: collection,
        state: Default::default(),
    });
    let code = reference_nft_collection_template_bytecode();
    state.set_contract_code(collection, code.clone());
    let gas_limit = GAS_PER_CONTRACT_CALL.max(500_000_000);
    let mut it = Interpreter::new(code.clone(), gas_limit);
    it.run(
        deployer,
        collection,
        &encode_owner_of_calldata(&tid_word(0)),
        &mut state,
    )
    .map_err(|e| format!("admin touch: {e:?}"))?;

    let ids: Vec<[u8; 32]> = (1..=n as u64).map(tid_word).collect();
    let hashes: Vec<[u8; 32]> = if with_metadata {
        (1..=n as u64).map(hash_word).collect()
    } else {
        vec![[0u8; 32]; n]
    };
    let data = encode_mint_batch_calldata(&to, &ids, &hashes);
    let mut it = Interpreter::new(code, gas_limit);
    let t0 = Instant::now();
    it.run(deployer, collection, &data, &mut state)
        .map_err(|e| format!("mint n={n}: {e:?}"))?;
    Ok((it.gas_used, t0.elapsed().as_micros()))
}

fn main() {
    println!("MAX_REFERENCE_NFT_MINT_BATCH={}", MAX_REFERENCE_NFT_MINT_BATCH);
    println!("GAS_PER_CONTRACT_CALL={}", GAS_PER_CONTRACT_CALL);
    println!("n\tgas_owner+meta\tfee_BOING\tcalldata\tms\tgas_owner_only");
    for n in [1usize, 10, 50, 100, 200, 500] {
        let (g, us) = measure(n, true).unwrap();
        let (g0, _) = measure(n, false).unwrap();
        let fee = (g + 20_999) / 21_000;
        let cd = 96 + 64 * n;
        println!("{n}\t{g}\t{fee}\t{cd}\t{:.1}\t{g0}", us as f64 / 1000.0);
    }
}
