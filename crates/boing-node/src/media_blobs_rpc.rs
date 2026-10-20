//! JSON-RPC helpers for L1 media blobs.

use boing_primitives::{
    parse_media_blob_id, BlobRetention, MAX_MEDIA_BLOB_BYTES, MAX_MEDIA_BLOB_CONTENT_TYPE_LEN,
    MAX_MEDIA_BLOB_TTL_BLOCKS, MEDIA_BLOB_FOREVER_MULTIPLIER, MEDIA_BLOB_GAS_BASE,
    MEDIA_BLOB_GAS_PER_WORD, MEDIA_BLOB_URI_PREFIX,
};
use serde_json::{json, Value};

use crate::media_blob_store::BlobMetaRecord;
use crate::node::BoingNode;

pub const MEDIA_BLOB_RPC_METHODS: &[&str] = &[
    "boing_getBlob",
    "boing_getBlobMeta",
    "boing_getMediaBlobParams",
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

/// Dispatch media-blob RPC. `Err((code, message))` on bad params / missing store.
pub fn dispatch(node: &BoingNode, method: &str, params: &Value) -> Result<Value, (i32, String)> {
    match method {
        "boing_getMediaBlobParams" => Ok(json!({
            "active": node.media_blobs_active_at_tip(),
            "activationHeight": node.media_blobs_activation_height(),
            "maxBytes": MAX_MEDIA_BLOB_BYTES,
            "maxContentTypeLen": MAX_MEDIA_BLOB_CONTENT_TYPE_LEN,
            "maxTtlBlocks": MAX_MEDIA_BLOB_TTL_BLOCKS,
            "gasBase": MEDIA_BLOB_GAS_BASE,
            "gasPerWord": MEDIA_BLOB_GAS_PER_WORD,
            "foreverMultiplier": MEDIA_BLOB_FOREVER_MULTIPLIER,
            "uriPrefix": MEDIA_BLOB_URI_PREFIX,
            "note": "PutBlob is a signed TransactionPayload (index 21); submit via boing_submitTransaction. Bytes are sidecar-stored under data/blobs/ — not contract SSTORE.",
        })),
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
        _ => Err((-32601, format!("unknown media blob method: {method}"))),
    }
}
