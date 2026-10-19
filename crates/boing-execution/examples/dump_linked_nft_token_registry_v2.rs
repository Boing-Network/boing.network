//! Print linked NFT↔token registry **v2** bytecode (hex).
//!
//! ```bash
//! # runtime (what is stored on chain; also a valid plain deploy for a fresh network)
//! cargo run -p boing-execution --example dump_linked_nft_token_registry_v2
//! # seeded 0xFD deploy bytecode; seed file lines: `claim <asset> <claimer>`,
//! # `link <collection> <token>` (in slot order; `link 0x00.. 0x00..` keeps a tombstone),
//! # `deployer <asset> <deployer>`; `#` starts a comment
//! cargo run -p boing-execution --example dump_linked_nft_token_registry_v2 -- --seed seed.txt
//! ```

use boing_execution::{
    linked_nft_token_registry_v2_deploy_bytecode, linked_nft_token_registry_v2_runtime_bytecode,
    LinkedNftTokenRegistryV2Seed,
};
use boing_primitives::AccountId;

fn id(s: &str) -> AccountId {
    let h = s.trim().trim_start_matches("0x");
    assert_eq!(h.len(), 64, "expected 32-byte hex AccountId, got {s}");
    let mut a = [0u8; 32];
    for (i, b) in a.iter_mut().enumerate() {
        *b = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).expect("hex");
    }
    AccountId(a)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = if let Some(pos) = args.iter().position(|a| a == "--seed") {
        let text = std::fs::read_to_string(&args[pos + 1]).expect("read seed file");
        let mut seed = LinkedNftTokenRegistryV2Seed::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(parts.len(), 3, "bad seed line: {line}");
            let pair = (id(parts[1]), id(parts[2]));
            match parts[0] {
                "claim" => seed.claims.push(pair),
                "link" => seed.links.push(pair),
                "deployer" => seed.deployers.push(pair),
                other => panic!("unknown seed kind {other}"),
            }
        }
        eprintln!(
            "seed: {} claims, {} links, {} deployer records",
            seed.claims.len(),
            seed.links.len(),
            seed.deployers.len()
        );
        linked_nft_token_registry_v2_deploy_bytecode(&seed)
    } else {
        linked_nft_token_registry_v2_runtime_bytecode()
    };
    eprintln!("linked_nft_token_registry_v2: {} bytes", code.len());
    print!("0x");
    for b in &code {
        print!("{b:02x}");
    }
    println!();
}
