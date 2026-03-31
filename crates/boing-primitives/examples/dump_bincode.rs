//! One-off: print bincode bytes for TransactionPayload / AccessList (must match JS SDK).
//! Run: `cargo run -p boing-primitives --example dump_bincode`
use boing_primitives::{
    signable_transaction_hash, AccessList, AccountId, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;

fn main() {
    let id = AccountId::from_bytes([1u8; 32]);
    let transfer = TransactionPayload::Transfer { to: id, amount: 100 };
    let p = bincode::serialize(&transfer).unwrap();
    println!("Transfer: len={} hex={}", p.len(), hex::encode(&p));

    let call = TransactionPayload::ContractCall {
        contract: id,
        calldata: vec![1, 2, 3],
    };
    let p = bincode::serialize(&call).unwrap();
    println!("ContractCall: len={} hex={}", p.len(), hex::encode(&p));

    let deploy = TransactionPayload::ContractDeploy {
        bytecode: vec![0xde, 0xad],
        create2_salt: None,
    };
    let p = bincode::serialize(&deploy).unwrap();
    println!("ContractDeploy: len={} hex={}", p.len(), hex::encode(&p));

    let bond = TransactionPayload::Bond { amount: 1 };
    let p = bincode::serialize(&bond).unwrap();
    println!("Bond: len={} hex={}", p.len(), hex::encode(&p));

    let unbond = TransactionPayload::Unbond { amount: 2 };
    let p = bincode::serialize(&unbond).unwrap();
    println!("Unbond: len={} hex={}", p.len(), hex::encode(&p));

    let dwp = TransactionPayload::ContractDeployWithPurpose {
        bytecode: vec![0xab],
        purpose_category: "defi".to_string(),
        description_hash: Some(vec![0xcc; 32]),
        create2_salt: None,
    };
    let p = bincode::serialize(&dwp).unwrap();
    println!("DeployWithPurpose: len={} hex={}", p.len(), hex::encode(&p));

    let dwpm = TransactionPayload::ContractDeployWithPurposeAndMetadata {
        bytecode: vec![0xef],
        purpose_category: "meme".to_string(),
        description_hash: None,
        asset_name: Some("Token".to_string()),
        asset_symbol: Some("TKN".to_string()),
        create2_salt: None,
    };
    let p = bincode::serialize(&dwpm).unwrap();
    println!("DeployWithMeta: len={} hex={}", p.len(), hex::encode(&p));

    let al = AccessList::default();
    let a = bincode::serialize(&al).unwrap();
    println!("AccessList empty: len={} hex={}", a.len(), hex::encode(&a));

    // Deterministic key for golden vectors (SDK tests must match).
    let key = SigningKey::from_bytes(&[
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x60, 0x91, 0xa2, 0xb8, 0xc7, 0x30, 0x4e, 0xe9,
        0x37, 0x70,
    ]);
    let sender = AccountId::from_bytes(key.verifying_key().to_bytes());
    let tx = Transaction {
        nonce: 7,
        sender,
        payload: TransactionPayload::Transfer {
            to: AccountId::from_bytes([2u8; 32]),
            amount: 9,
        },
        access_list: AccessList::default(),
    };
    let signable = signable_transaction_hash(&tx);
    println!("SignableHash: hex={}", hex::encode(signable));

    let signed = SignedTransaction::new(tx, &key);
    let s = bincode::serialize(&signed).unwrap();
    println!(
        "SignedTransaction: len={} hex={}",
        s.len(),
        hex::encode(&s)
    );

    let sig_only = bincode::serialize(&signed.signature).unwrap();
    println!("Signature alone: len={} hex={}", sig_only.len(), hex::encode(&sig_only));
}
