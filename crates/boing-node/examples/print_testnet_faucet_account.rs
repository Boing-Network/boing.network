//! One-off: print deterministic testnet faucet AccountId hex (for docs / explorer).
use boing_node::faucet::testnet_faucet_account_id;

fn main() {
    let id = testnet_faucet_account_id();
    println!("{}", hex::encode(id.0));
}
