//! Golden vectors for the transaction signing digest, including the tipped `BOING_TX_PRIORITY`
//! domain. The same vectors are published as `docs/fixtures/tx-signing-golden-vectors.json` for
//! boing.express, the SDK and any other signer; this test fails if Rust and the fixture diverge.
//!
//! Regenerate the fixture after an *intentional* change with
//! `BOING_UPDATE_GOLDEN=1 cargo test -p boing-primitives --test tx_signing_golden`.

use boing_primitives::{
    signable_transaction_hash_v2, signable_transaction_hash_with_tip, AccessList, AccountId,
    SignedTransaction, Transaction, TransactionPayload, TX_SIGNING_DOMAIN_PRIORITY,
    TX_SIGNING_DOMAIN_V2,
};
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

const SEED_HEX: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326091a2b8c7304ee93770";
const FIXTURE_REL: &str = "../../docs/fixtures/tx-signing-golden-vectors.json";

/// Pinned independently of the fixture file: tipped transfer on testnet.
const PINNED_TIPPED_TRANSFER_TESTNET_DIGEST: &str = "ecce575ea959a0ffd15a858e1f8be2d2a1395179d298ff8e117a413992770208";

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&hex::decode(SEED_HEX).unwrap().try_into().unwrap())
}

struct Case {
    name: &'static str,
    chain_id: u64,
    nonce: u64,
    payload: TransactionPayload,
    payload_json: Value,
    read: Vec<AccountId>,
    write: Vec<AccountId>,
    tip: u128,
}

fn cases(sender: AccountId) -> Vec<Case> {
    let to = AccountId::from_bytes([1u8; 32]);
    let other = AccountId::from_bytes([2u8; 32]);
    let transfer = |amount: u128| TransactionPayload::Transfer { to, amount };
    let transfer_json = |amount: &str| {
        json!({"kind": "transfer", "to": format!("0x{}", hex::encode([1u8; 32])), "amount": amount})
    };
    vec![
        Case {
            name: "transfer_untipped_testnet_is_v2",
            chain_id: 6913,
            nonce: 7,
            payload: transfer(100),
            payload_json: transfer_json("100"),
            read: vec![sender],
            write: vec![sender, to],
            tip: 0,
        },
        Case {
            name: "transfer_tip_1_testnet",
            chain_id: 6913,
            nonce: 7,
            payload: transfer(100),
            payload_json: transfer_json("100"),
            read: vec![sender],
            write: vec![sender, to],
            tip: 1,
        },
        Case {
            name: "transfer_tip_1_mainnet_differs_from_testnet",
            chain_id: 6914,
            nonce: 7,
            payload: transfer(100),
            payload_json: transfer_json("100"),
            read: vec![sender],
            write: vec![sender, to],
            tip: 1,
        },
        Case {
            name: "contract_call_tip_1e12_with_access_list",
            chain_id: 6913,
            nonce: 0,
            payload: TransactionPayload::ContractCall {
                contract: other,
                calldata: vec![1, 2, 3],
            },
            payload_json: json!({"kind": "contractCall", "contract": format!("0x{}", hex::encode([2u8; 32])), "calldata": "0x010203"}),
            read: vec![sender, other],
            write: vec![other],
            tip: 1_000_000_000_000,
        },
        Case {
            name: "transfer_tip_u128_max",
            chain_id: 6913,
            nonce: u64::MAX,
            payload: transfer(u128::MAX),
            payload_json: transfer_json(&u128::MAX.to_string()),
            read: vec![],
            write: vec![],
            tip: u128::MAX,
        },
    ]
}

fn build_fixture() -> Value {
    let sk = signing_key();
    let sender = AccountId(sk.verifying_key().to_bytes());
    let vectors: Vec<Value> = cases(sender)
        .into_iter()
        .map(|c| {
            let tx = Transaction {
                nonce: c.nonce,
                sender,
                payload: c.payload.clone(),
                access_list: AccessList::new(c.read.clone(), c.write.clone()),
            };
            let digest = signable_transaction_hash_with_tip(&tx, c.chain_id, c.tip);
            // Independent preimage construction (not via the library hasher).
            let domain: &[u8] = if c.tip == 0 {
                TX_SIGNING_DOMAIN_V2
            } else {
                TX_SIGNING_DOMAIN_PRIORITY
            };
            let mut pre = Vec::new();
            pre.extend_from_slice(domain);
            pre.extend_from_slice(&c.chain_id.to_le_bytes());
            pre.extend_from_slice(&c.nonce.to_le_bytes());
            pre.extend_from_slice(&sender.0);
            pre.extend_from_slice(&bincode::serialize(&tx.payload).unwrap());
            pre.extend_from_slice(&bincode::serialize(&tx.access_list).unwrap());
            if c.tip != 0 {
                pre.extend_from_slice(&c.tip.to_le_bytes());
            }
            assert_eq!(*blake3::hash(&pre).as_bytes(), digest, "{}: preimage mismatch", c.name);
            if c.tip == 0 {
                assert_eq!(digest, signable_transaction_hash_v2(&tx, c.chain_id));
            }
            let signed = SignedTransaction::new_for_chain_with_tip(tx.clone(), &sk, c.chain_id, c.tip);
            let signed_bytes = bincode::serialize(&signed).unwrap();
            let decoded = SignedTransaction::from_bincode(&signed_bytes).unwrap();
            assert_eq!(decoded.priority_tip, c.tip, "{}: tip must round-trip", c.name);
            json!({
                "name": c.name,
                "chain_id": c.chain_id,
                "nonce": c.nonce.to_string(),
                "sender": format!("0x{}", hex::encode(sender.0)),
                "payload": c.payload_json,
                "access_list": {
                    "read": c.read.iter().map(|a| format!("0x{}", hex::encode(a.0))).collect::<Vec<_>>(),
                    "write": c.write.iter().map(|a| format!("0x{}", hex::encode(a.0))).collect::<Vec<_>>(),
                },
                "priority_tip": c.tip.to_string(),
                "domain": String::from_utf8_lossy(domain),
                "payload_bincode_hex": hex::encode(bincode::serialize(&tx.payload).unwrap()),
                "access_list_bincode_hex": hex::encode(bincode::serialize(&tx.access_list).unwrap()),
                "preimage_hex": hex::encode(&pre),
                "signing_digest_hex": hex::encode(digest),
                "signature_hex": hex::encode(signed.signature.0),
                "signed_tx_bincode_hex": hex::encode(&signed_bytes),
            })
        })
        .collect();
    json!({
        "description": "Golden vectors for the Boing transaction signing digest (v2 and tipped BOING_TX_PRIORITY). Generated by crates/boing-primitives/tests/tx_signing_golden.rs. Digest = BLAKE3(domain || chain_id_le8 || nonce_le8 || sender32 || bincode(payload) || bincode(access_list) [|| tip_le16 when tip > 0]); domain is BOING_TX_V2 when tip == 0, BOING_TX_PRIORITY otherwise. Signature = Ed25519(digest) with the deterministic seed below. u64/u128 values are decimal strings.",
        "version": 1,
        "signer_seed_hex": SEED_HEX,
        "signer_public_key_hex": hex::encode(sender.0),
        "vectors": vectors,
    })
}

#[test]
fn golden_vectors_match_published_fixture() {
    let fresh = build_fixture();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_REL);
    if std::env::var("BOING_UPDATE_GOLDEN").is_ok() {
        std::fs::write(&path, serde_json::to_string_pretty(&fresh).unwrap() + "\n").unwrap();
        return;
    }
    let on_disk: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("fixture missing; run with BOING_UPDATE_GOLDEN=1"),
    )
    .expect("fixture is valid JSON");
    assert_eq!(on_disk, fresh, "docs/fixtures/tx-signing-golden-vectors.json is stale");
}

#[test]
fn pinned_tipped_digest_never_changes() {
    let fresh = build_fixture();
    let vs = fresh["vectors"].as_array().unwrap();
    let v = vs
        .iter()
        .find(|v| v["name"] == "transfer_tip_1_testnet")
        .unwrap();
    assert_eq!(v["signing_digest_hex"], PINNED_TIPPED_TRANSFER_TESTNET_DIGEST);
    // Tip, chain and domain each change the digest.
    let digests: std::collections::HashSet<_> = vs
        .iter()
        .map(|v| v["signing_digest_hex"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(digests.len(), vs.len());
}