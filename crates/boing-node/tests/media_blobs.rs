//! L1 media blobs MVP: activation gating, PutBlob fee path, sidecar persist, getBlob RPC.

use std::path::PathBuf;

use boing_node::media_blob_store::MediaBlobStore;
use boing_node::node::BoingNode;
use boing_primitives::{
    blob_id_of, AccessList, Account, AccountId, AccountState, BlobRetention, SignedTransaction,
    Transaction, TransactionPayload, MAX_MEDIA_BLOB_BYTES,
};
use ed25519_dalek::SigningKey;
use serde_json::json;

fn key(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}
fn idk(k: &SigningKey) -> AccountId {
    AccountId(k.verifying_key().to_bytes())
}

fn tmp_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "boing-media-blobs-it-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn funded_node(activation: Option<u64>, data: &PathBuf) -> (BoingNode, SigningKey) {
    let sk = key(42);
    let id = idk(&sk);
    let mut n = BoingNode::with_validators(vec![id], id, Some(sk.clone()));
    n.state.insert(Account {
        id,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    n.attach_media_blob_store(MediaBlobStore::open(data).unwrap());
    if let Some(h) = activation {
        n.enable_media_blobs(h);
    }
    (n, sk)
}

fn put_tx(sk: &SigningKey, nonce: u64, data: Vec<u8>, retention: BlobRetention) -> SignedTransaction {
    let sender = idk(sk);
    SignedTransaction::new(
        Transaction {
            nonce,
            sender,
            payload: TransactionPayload::PutBlob {
                retention,
                content_type: "image/png".into(),
                data,
            },
            access_list: AccessList::new(vec![sender], vec![sender]),
        },
        sk,
    )
}

#[test]
fn put_blob_rejected_when_inactive() {
    let dir = tmp_dir("off");
    let (n, sk) = funded_node(None, &dir);
    let signed = put_tx(&sk, 0, b"x".to_vec(), BlobRetention::Forever);
    let err = n.submit_transaction(signed).unwrap_err();
    assert!(
        format!("{err}").contains("media blob"),
        "unexpected: {err}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn put_blob_round_trip_rpc() {
    let dir = tmp_dir("on");
    let (mut n, sk) = funded_node(Some(0), &dir);
    let data = b"tiny-png-bytes".to_vec();
    let id = blob_id_of(&data);
    let signed = put_tx(
        &sk,
        0,
        data.clone(),
        BlobRetention::Prunable { ttl_blocks: 1_000 },
    );
    n.submit_transaction(signed).unwrap();
    n.produce_block_if_ready()
        .expect("single-validator node should produce PutBlob block");

    let params = n
        .media_blobs_rpc("boing_getMediaBlobParams", &json!([]))
        .unwrap();
    assert_eq!(params["maxBytes"], MAX_MEDIA_BLOB_BYTES);
    assert_eq!(params["active"], true);

    let meta = n
        .media_blobs_rpc(
            "boing_getBlobMeta",
            &json!([format!("0x{}", hex::encode(id))]),
        )
        .unwrap();
    assert_eq!(meta["size"], data.len() as u64);
    assert_eq!(meta["contentType"], "image/png");

    let blob = n
        .media_blobs_rpc(
            "boing_getBlob",
            &json!([format!("0x{}", hex::encode(id))]),
        )
        .unwrap();
    assert_eq!(blob["dataHex"], format!("0x{}", hex::encode(&data)));

    let oversize = vec![0u8; MAX_MEDIA_BLOB_BYTES + 1];
    let bad = put_tx(&sk, 1, oversize, BlobRetention::Forever);
    assert!(n.submit_transaction(bad).is_err());

    let _ = std::fs::remove_dir_all(dir);
}
