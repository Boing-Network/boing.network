//! JSON-RPC helpers for L1 media blobs.

use boing_primitives::{
    chunk_media_blob, media_blob_das_sample, parse_media_blob_id, BlobRetention,
    MAX_MEDIA_BLOB_BYTES, MAX_MEDIA_BLOB_CONTENT_TYPE_LEN, MAX_MEDIA_BLOB_INLINE_BYTES,
    MAX_MEDIA_BLOB_INLINE_PREFER_BYTES, MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS,
    MAX_MEDIA_BLOB_TTL_BLOCKS, MEDIA_BLOBS_GOSSIP_TOPIC, MEDIA_BLOB_DAS_CHUNK_BYTES,
    MEDIA_BLOB_FOREVER_MULTIPLIER, MEDIA_BLOB_GAS_BASE, MEDIA_BLOB_GAS_PER_WORD,
    MEDIA_BLOB_RS_PARITY_SHARDS, MEDIA_BLOB_URI_PREFIX, MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS,
    PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES,
};
use serde_json::{json, Value};

use crate::media_blob_store::BlobMetaRecord;
use crate::node::BoingNode;

pub const MEDIA_BLOB_RPC_METHODS: &[&str] = &[
    "boing_getBlob",
    "boing_getBlobMeta",
    "boing_getMediaBlobParams",
    "boing_getBlobDasSample",
];

fn hex0x(b: &[u8]) -> String {
    format!("0x{}", hex::encode(b))
}

fn retention_json(r: BlobRetention) -> Value {
    match r {
        BlobRetention::Forever => json!({"kind": "forever"}),
        BlobRetention::Prunable { ttl_blocks } => {
            json!({"kind": "prunable", "ttlBlocks": ttl_blocks})
        }
    }
}

fn meta_json(m: &BlobMetaRecord) -> Value {
    json!({
        "blobId": hex0x(&m.blob_id),
        "uri": format!("{MEDIA_BLOB_URI_PREFIX}{}", hex0x(&m.blob_id)),
        "size": m.size,
        "contentType": m.content_type,
        "retention": retention_json(m.retention),
        "publisher": hex0x(&m.publisher),
        "includedHeight": m.included_height,
        "txId": hex0x(&m.tx_id),
        "pinUntilHeight": m.pin_until_height,
        "expiryHeight": m.expiry_height(),
    })
}

fn parse_id_param(params: &Value) -> Result<[u8; 32], (i32, String)> {
    let s = params
        .get(0)
        .and_then(|v| v.as_str())
        .or_else(|| params.get("blobId").and_then(|v| v.as_str()))
        .or_else(|| params.get("id").and_then(|v| v.as_str()))
        .ok_or((
            -32602,
            "expected blobId string (0x…, hex, or boing://blob/0x…)".into(),
        ))?;
    parse_media_blob_id(s).ok_or((-32602, format!("invalid blob id: {s}")))
}

fn parse_chunk_index(params: &Value) -> Result<u32, (i32, String)> {
    let v = params
        .get(1)
        .or_else(|| params.get("chunkIndex"))
        .ok_or((-32602, "expected chunkIndex (number)".into()))?;
    if let Some(n) = v.as_u64() {
        return u32::try_from(n).map_err(|_| (-32602, "chunkIndex out of u32 range".into()));
    }
    if let Some(s) = v.as_str() {
        return s
            .parse()
            .map_err(|_| (-32602, format!("invalid chunkIndex: {s}")));
    }
    Err((-32602, "chunkIndex must be a number or decimal string".into()))
}

/// Dispatch media-blob RPC. `Err((code, message))` on bad params / missing store.
pub fn dispatch(node: &BoingNode, method: &str, params: &Value) -> Result<Value, (i32, String)> {
    match method {
        "boing_getMediaBlobParams" => {
            let header_from = node.blobs_root_header_activation_height();
            Ok(json!({
                "active": node.media_blobs_active_at_tip(),
                "activationHeight": node.media_blobs_activation_height(),
                "maxBytes": MAX_MEDIA_BLOB_BYTES,
                "maxInlineBytes": MAX_MEDIA_BLOB_INLINE_BYTES,
                "maxInlinePreferBytes": MAX_MEDIA_BLOB_INLINE_PREFER_BYTES,
                "maxContentTypeLen": MAX_MEDIA_BLOB_CONTENT_TYPE_LEN,
                "maxTtlBlocks": MAX_MEDIA_BLOB_TTL_BLOCKS,
                "gasBase": MEDIA_BLOB_GAS_BASE,
                "gasPerWord": MEDIA_BLOB_GAS_PER_WORD,
                "foreverMultiplier": MEDIA_BLOB_FOREVER_MULTIPLIER,
                "uriPrefix": MEDIA_BLOB_URI_PREFIX,
                "gossipTopic": MEDIA_BLOBS_GOSSIP_TOPIC,
                "putBlobVariant": 21,
                "blobAnnounceVariant": 22,
            "blobsRootInHeader": true,
            "blobsRootHeaderActivationHeight": header_from,
            "blobsRootVerifiedWhenActivated": header_from.is_some(),
                "phase3TargetMaxBytes": PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES,
                "dasChunkBytes": MEDIA_BLOB_DAS_CHUNK_BYTES,
                "dasSampleRpc": "boing_getBlobDasSample",
                "rsParityShards": MEDIA_BLOB_RS_PARITY_SHARDS,
                "renewMediaBlobPinVariant": 23,
                "payBlobSampleVariant": 24,
                "maxRenewExtendBlocks": MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS,
                "videoPinHorizonBlocksHint": MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS,
                "foreverPinFeePolicyField": "blob_forever_pin_fee_per_kib",
                "pinRenewFeePolicyField": "blob_pin_renew_fee_per_kib_block",
                "sampleFeePolicyField": "blob_sample_fee_per_chunk",
                "phase6TargetMaxBytes": boing_primitives::PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES,
                "phase6MaxVideoSegments": boing_primitives::PHASE6_MAX_VIDEO_SEGMENTS,
                "phase6MaxDasLightClientSamples": boing_primitives::PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES,
                "note": "Phase 5 live: RS (parity=2) + RenewMediaBlobPin (23) / PayBlobSample (24). Phase 6 scaffold only: sampling-market match + DAS light-client batch + segmented video target (256 MiB design — not consensus). Live cap 16 MiB.",
            }))
        }
        "boing_getBlobMeta" => {
            let id = parse_id_param(params)?;
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            match store.get_meta(&id) {
                Some(m) => Ok(meta_json(&m)),
                None => Ok(Value::Null),
            }
        }
        "boing_getBlob" => {
            let id = parse_id_param(params)?;
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            let meta = store.get_meta(&id);
            let bytes = store
                .get_bytes(&id)
                .map_err(|e| (-32000, e.to_string()))?;
            match (meta, bytes) {
                (None, None) | (None, Some(_)) | (Some(_), None) => Ok(Value::Null),
                (Some(m), Some(data)) => Ok(json!({
                    "meta": meta_json(&m),
                    "dataHex": hex0x(&data),
                })),
            }
        }
        "boing_getBlobDasSample" => {
            let id = parse_id_param(params)?;
            let chunk_index = parse_chunk_index(params)?;
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            let bytes = store
                .get_bytes(&id)
                .map_err(|e| (-32000, e.to_string()))?;
            let Some(data) = bytes else {
                return Ok(Value::Null);
            };
            let bundle = chunk_media_blob(&data).map_err(|e| (-32000, e.to_string()))?;
            if bundle.blob_id != id {
                return Err((-32000, "stored bytes do not match blob id".into()));
            }
            let sample =
                media_blob_das_sample(&bundle, chunk_index).map_err(|e| (-32000, e.to_string()))?;
            Ok(json!({
                "blobId": hex0x(&sample.blob_id),
                "chunkIndex": sample.chunk_index,
                "chunkCount": sample.chunk_count,
                "chunkHash": hex0x(&sample.chunk_hash),
                "chunksRoot": hex0x(sample.chunks_root.as_bytes()),
                "siblings": sample.siblings.iter().map(|s| hex0x(s)).collect::<Vec<_>>(),
                "parityChunkHex": hex0x(&bundle.parity_chunk),
                "originalLen": bundle.original_len,
                "note": "Light-client DAS hook: merkle inclusion of one data chunk. XOR parity recovers one missing chunk locally — not a full sampling market.",
            }))
        }
        _ => Err((-32601, format!("unknown media blob method: {method}"))),
    }
}
