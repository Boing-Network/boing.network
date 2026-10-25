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

    // Phase 6: DAS light-client batch + node-local sample offer book
    let lc = n
        .media_blobs_rpc(
            "boing_getBlobDasLightClientBatch",
            &json!([format!("0x{}", hex::encode(id)), 4]),
        )
        .unwrap();
    assert!(lc["samples"].as_array().unwrap().len() >= 1);
    assert_eq!(lc["blobId"], format!("0x{}", hex::encode(id)));

    let provider = format!("0x{}", hex::encode([9u8; 32]));
    let put_offer = n
        .media_blobs_rpc(
            "boing_putBlobSampleOffer",
            &json!([{
                "blobId": format!("0x{}", hex::encode(id)),
                "chunkIndex": 0,
                "chunkCount": 1,
                "provider": provider,
                "askFee": "7",
                "expiresAtHeight": 0,
            }]),
        )
        .unwrap();
    assert_eq!(put_offer["ok"], true);
    let listed = n
        .media_blobs_rpc(
            "boing_listBlobSampleOffers",
            &json!([format!("0x{}", hex::encode(id))]),
        )
        .unwrap();
    assert_eq!(listed["offers"].as_array().unwrap().len(), 1);
    let matched = n
        .media_blobs_rpc(
            "boing_matchBlobSampleOffer",
            &json!([{
                "blobId": format!("0x{}", hex::encode(id)),
                "chunkIndex": 0,
                "chunkCount": 1,
                "payer": format!("0x{}", hex::encode(idk(&sk).0)),
                "paidFee": "7",
            }]),
        )
        .unwrap();
    assert_eq!(matched["offer"]["askFee"], "7");

    let params = n
        .media_blobs_rpc("boing_getMediaBlobParams", &json!([]))
        .unwrap();
    assert_eq!(params["maxBytes"], MAX_MEDIA_BLOB_BYTES);
    assert_eq!(
        params["phase6TargetMaxBytes"],
        boing_primitives::PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES
    );
    assert_eq!(params["effectiveMaxBytesAtTip"], MAX_MEDIA_BLOB_BYTES);
    assert_eq!(params["phase7CapActiveAtTip"], false);
    assert_eq!(params["registerBlobSampleOfferVariant"], 25);
    assert_eq!(params["settleBlobSampleMatchVariant"], 26);
    assert_eq!(params["submitSlashAppealVariant"], 30);
    assert_eq!(params["resolveSlashAppealVariant"], 31);
    assert_eq!(params["consensusSlashAppealTxs"], true);
    assert_eq!(params["consensusEquivocationSlashAppeals"], true);
    assert_eq!(params["consensusSlashAppealThresholdResolve"], true);
    assert_eq!(params["consensusSlashAppealStakeWeightedResolve"], true);
    assert_eq!(
        params["slashAppealResolveThresholdNum"],
        boing_primitives::PHASE17_RESOLVE_THRESHOLD_NUM
    );
    assert_eq!(
        params["slashAppealResolveThresholdDen"],
        boing_primitives::PHASE17_RESOLVE_THRESHOLD_DEN
    );
    assert_eq!(
        params["maxAppealEvidenceBytes"],
        boing_primitives::PHASE14_MAX_APPEAL_EVIDENCE_BYTES
    );

    // Phase 7 gated cap: still 16 MiB until activation; then effective max rises.
    n.enable_media_blobs_phase7_cap(0);
    let params2 = n
        .media_blobs_rpc("boing_getMediaBlobParams", &json!([]))
        .unwrap();
    assert_eq!(params2["phase7CapActiveAtTip"], true);
    assert_eq!(
        params2["effectiveMaxBytesAtTip"],
        boing_primitives::PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES
    );
    // Default constant unchanged (Fly boot safety).
    assert_eq!(MAX_MEDIA_BLOB_BYTES, 16 * 1024 * 1024);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn das_sample_uses_cache_and_respects_blob_removal() {
    let dir = tmp_dir("das-cache");
    let (mut n, sk) = funded_node(Some(0), &dir);
    // Multi-chunk blob (unique content so the global cache entry is ours alone).
    let data: Vec<u8> = (0..600_000u32).map(|i| (i * 7 + 13) as u8).collect();
    let id = blob_id_of(&data);
    n.submit_transaction(put_tx(&sk, 0, data.clone(), BlobRetention::Forever))
        .unwrap();
    n.produce_block_if_ready().expect("block");
    let idhex = format!("0x{}", hex::encode(id));

    let (h0, m0, _, _) = boing_node::das_cache::global_stats();
    let first = n
        .media_blobs_rpc("boing_getBlobDasSample", &json!([idhex, 1]))
        .unwrap();
    let second = n
        .media_blobs_rpc("boing_getBlobDasSample", &json!([idhex, 1]))
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first["chunkIndex"], 1);
    let batch = n
        .media_blobs_rpc("boing_getBlobDasLightClientBatch", &json!([idhex, 3]))
        .unwrap();
    assert_eq!(batch["blobId"], idhex);
    let (h1, m1, _, _) = boing_node::das_cache::global_stats();
    assert!(m1 >= m0 + 1, "first call must miss");
    assert!(h1 >= h0 + 2, "later calls must hit the cache ({h0}->{h1})");

    // Removing the stored bytes must still yield null even though the bundle is cached.
    let want = format!("{}.bin", hex::encode(id));
    let file = walk(&dir)
        .into_iter()
        .find(|p| p.file_name().map(|f| f.to_string_lossy() == want).unwrap_or(false))
        .expect("blob data file on disk");
    std::fs::remove_file(file).unwrap();
    let after = n
        .media_blobs_rpc("boing_getBlobDasSample", &json!([idhex, 1]))
        .unwrap();
    assert!(after.is_null(), "removed blob must not be served from cache");
    let _ = std::fs::remove_dir_all(dir);
}

fn walk(p: &PathBuf) -> Vec<PathBuf> {
    let mut v = vec![];
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let pb = e.path();
            if pb.is_dir() {
                v.extend(walk(&pb));
            } else {
                v.push(pb);
            }
        }
    }
    v
}
