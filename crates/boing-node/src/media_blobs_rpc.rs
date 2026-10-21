//! JSON-RPC helpers for L1 media blobs.

use boing_primitives::{
    build_das_light_client_batch_from_bytes, chunk_media_blob, media_blob_das_sample,
    parse_media_blob_id, AccountId, BlobRetention, MediaBlobSampleOffer, MAX_MEDIA_BLOB_BYTES,
    MAX_MEDIA_BLOB_CONTENT_TYPE_LEN, MAX_MEDIA_BLOB_INLINE_BYTES,
    MAX_MEDIA_BLOB_INLINE_PREFER_BYTES, MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS,
    MAX_MEDIA_BLOB_TTL_BLOCKS, MEDIA_BLOBS_GOSSIP_TOPIC, MEDIA_BLOB_DAS_CHUNK_BYTES,
    MEDIA_BLOB_FOREVER_MULTIPLIER, MEDIA_BLOB_GAS_BASE, MEDIA_BLOB_GAS_PER_WORD,
    MEDIA_BLOB_RS_PARITY_SHARDS, MEDIA_BLOB_URI_PREFIX, MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS,
    PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES, PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES,
    PHASE6_MAX_SAMPLE_OFFERS, PHASE6_MAX_VIDEO_SEGMENTS, PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES,
    PHASE6_VIDEO_SEGMENT_TARGET_BYTES,
};
use serde_json::{json, Value};

use crate::media_blob_store::BlobMetaRecord;
use crate::node::BoingNode;

pub const MEDIA_BLOB_RPC_METHODS: &[&str] = &[
    "boing_getBlob",
    "boing_getBlobMeta",
    "boing_getMediaBlobParams",
    "boing_getBlobDasSample",
    "boing_getBlobDasLightClientBatch",
    "boing_putBlobSampleOffer",
    "boing_listBlobSampleOffers",
    "boing_matchBlobSampleOffer",
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

fn offer_json(o: &MediaBlobSampleOffer) -> Value {
    json!({
        "blobId": hex0x(&o.blob_id),
        "chunkIndex": o.chunk_index,
        "chunkCount": o.chunk_count,
        "provider": hex0x(&o.provider.0),
        "askFee": o.ask_fee.to_string(),
        "expiresAtHeight": o.expires_at_height,
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
    parse_u32(v, "chunkIndex")
}

fn parse_u32(v: &Value, name: &str) -> Result<u32, (i32, String)> {
    if let Some(n) = v.as_u64() {
        return u32::try_from(n).map_err(|_| (-32602, format!("{name} out of u32 range")));
    }
    if let Some(s) = v.as_str() {
        return s
            .parse()
            .map_err(|_| (-32602, format!("invalid {name}: {s}")));
    }
    Err((-32602, format!("{name} must be a number or decimal string")))
}

fn parse_u64_field(params: &Value, keys: &[&str], default: Option<u64>) -> Result<u64, (i32, String)> {
    for k in keys {
        if let Some(v) = params.get(*k) {
            if let Some(n) = v.as_u64() {
                return Ok(n);
            }
            if let Some(s) = v.as_str() {
                return s
                    .parse()
                    .map_err(|_| (-32602, format!("invalid {k}: {s}")));
            }
            return Err((-32602, format!("{k} must be a number or decimal string")));
        }
    }
    // Positional: params[2] style handled by callers
    if let Some(d) = default {
        return Ok(d);
    }
    Err((-32602, format!("expected one of {}", keys.join("/"))))
}

fn parse_u128_field(params: &Value, key: &str) -> Result<u128, (i32, String)> {
    let v = params
        .get(key)
        .ok_or((-32602, format!("expected {key}")))?;
    if let Some(n) = v.as_u64() {
        return Ok(n as u128);
    }
    if let Some(s) = v.as_str() {
        return s
            .parse()
            .map_err(|_| (-32602, format!("invalid {key}: {s}")));
    }
    Err((-32602, format!("{key} must be a number or decimal string")))
}

fn parse_account(s: &str) -> Result<AccountId, (i32, String)> {
    let h = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(h).map_err(|_| (-32602, format!("invalid account hex: {s}")))?;
    if bytes.len() != 32 {
        return Err((-32602, format!("account must be 32 bytes: {s}")));
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&bytes);
    Ok(AccountId(a))
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
                "dasLightClientBatchRpc": "boing_getBlobDasLightClientBatch",
                "rsParityShards": MEDIA_BLOB_RS_PARITY_SHARDS,
                "renewMediaBlobPinVariant": 23,
                "payBlobSampleVariant": 24,
                "maxRenewExtendBlocks": MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS,
                "videoPinHorizonBlocksHint": MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS,
                "foreverPinFeePolicyField": "blob_forever_pin_fee_per_kib",
                "pinRenewFeePolicyField": "blob_pin_renew_fee_per_kib_block",
                "sampleFeePolicyField": "blob_sample_fee_per_chunk",
                "phase6TargetMaxBytes": PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES,
                "phase6VideoSegmentTargetBytes": PHASE6_VIDEO_SEGMENT_TARGET_BYTES,
                "phase6MaxVideoSegments": PHASE6_MAX_VIDEO_SEGMENTS,
                "phase6MaxDasLightClientSamples": PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES,
                "phase6MaxSampleOffers": PHASE6_MAX_SAMPLE_OFFERS,
                "sampleOfferRpcs": [
                    "boing_putBlobSampleOffer",
                    "boing_listBlobSampleOffers",
                    "boing_matchBlobSampleOffer",
                ],
                "note": "Phase 5 live: RS (parity=2) + Renew/PayBlobSample. Phase 6: node-local sample offer book + DAS light-client batch RPC + segment helpers. Design target 256 MiB via segments — live Put/Announce cap remains 16 MiB. Sample book is not consensus.",
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
                "note": "Light-client DAS hook: merkle inclusion of one data chunk. XOR parity recovers one missing chunk locally.",
            }))
        }
        "boing_getBlobDasLightClientBatch" => {
            let id = parse_id_param(params)?;
            let sample_count = params
                .get(1)
                .or_else(|| params.get("sampleCount"))
                .map(|v| parse_u32(v, "sampleCount"))
                .transpose()?
                .unwrap_or(8)
                .min(PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES as u32)
                .max(1);
            let tip = node.chain.height();
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
            let batch = build_das_light_client_batch_from_bytes(&data, tip, sample_count as usize)
                .map_err(|e| (-32000, e.to_string()))?;
            if batch.blob_id != id {
                return Err((-32000, "stored bytes do not match blob id".into()));
            }
            Ok(json!({
                "blobId": hex0x(&batch.blob_id),
                "chunksRoot": hex0x(&batch.chunks_root),
                "seedHeight": batch.seed_height,
                "coverageRatio": batch.coverage_ratio(),
                "samples": batch.samples.iter().map(|s| json!({
                    "chunkIndex": s.chunk_index,
                    "chunkCount": s.chunk_count,
                    "chunkHash": hex0x(&s.chunk_hash),
                    "siblings": s.siblings.iter().map(|x| hex0x(x)).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "note": "Phase 6 DAS light-client batch: height-bound deterministic merkle samples. Not VRF; not a succinct aggregate proof.",
            }))
        }
        "boing_putBlobSampleOffer" => {
            let obj = params
                .as_object()
                .or_else(|| params.get(0).and_then(|v| v.as_object()))
                .ok_or((-32602, "expected offer object".into()))?;
            let params = Value::Object(obj.clone());
            let blob_id = parse_id_param(&params)?;
            let chunk_index = parse_u32(
                params
                    .get("chunkIndex")
                    .ok_or((-32602, "expected chunkIndex".into()))?,
                "chunkIndex",
            )?;
            let chunk_count = parse_u32(
                params
                    .get("chunkCount")
                    .ok_or((-32602, "expected chunkCount".into()))?,
                "chunkCount",
            )?;
            let provider_s = params
                .get("provider")
                .and_then(|v| v.as_str())
                .ok_or((-32602, "expected provider hex".into()))?;
            let provider = parse_account(provider_s)?;
            let ask_fee = parse_u128_field(&params, "askFee")?;
            let expires_at_height =
                parse_u64_field(&params, &["expiresAtHeight"], Some(0))?;
            let offer = MediaBlobSampleOffer {
                blob_id,
                chunk_index,
                chunk_count,
                provider,
                ask_fee,
                expires_at_height,
            };
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            store
                .upsert_sample_offer(offer.clone())
                .map_err(|e| (-32000, e))?;
            Ok(json!({
                "ok": true,
                "offer": offer_json(&offer),
                "note": "Node-local sample offer book (sidecar). Not consensus; pairs with on-chain PayBlobSample fee.",
            }))
        }
        "boing_listBlobSampleOffers" => {
            let id = parse_id_param(params)?;
            let chunk_index = params
                .get(1)
                .or_else(|| params.get("chunkIndex"))
                .map(|v| parse_u32(v, "chunkIndex"))
                .transpose()?;
            let tip = node.chain.height();
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            let _ = store.prune_sample_offers(tip);
            let market = store.sample_market_snapshot();
            let offers = market.list_offers(&id, chunk_index, tip);
            Ok(json!({
                "blobId": hex0x(&id),
                "tipHeight": tip,
                "offers": offers.iter().map(|o| offer_json(o)).collect::<Vec<_>>(),
            }))
        }
        "boing_matchBlobSampleOffer" => {
            let obj = params
                .as_object()
                .or_else(|| params.get(0).and_then(|v| v.as_object()))
                .ok_or((-32602, "expected match object".into()))?;
            let params = Value::Object(obj.clone());
            let blob_id = parse_id_param(&params)?;
            let chunk_index = parse_u32(
                params
                    .get("chunkIndex")
                    .ok_or((-32602, "expected chunkIndex".into()))?,
                "chunkIndex",
            )?;
            let chunk_count = parse_u32(
                params
                    .get("chunkCount")
                    .ok_or((-32602, "expected chunkCount".into()))?,
                "chunkCount",
            )?;
            let payer = parse_account(
                params
                    .get("payer")
                    .and_then(|v| v.as_str())
                    .ok_or((-32602, "expected payer hex".into()))?,
            )?;
            let paid_fee = parse_u128_field(&params, "paidFee")?;
            let provider = match params.get("provider").and_then(|v| v.as_str()) {
                Some(s) => Some(parse_account(s)?),
                None => None,
            };
            let tip = node.chain.height();
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            let _ = store.prune_sample_offers(tip);
            let market = store.sample_market_snapshot();
            let m = market
                .match_pay_blob_sample(
                    &blob_id,
                    chunk_index,
                    chunk_count,
                    payer,
                    paid_fee,
                    tip,
                    provider.as_ref(),
                )
                .map_err(|e| (-32000, e.to_string()))?;
            Ok(json!({
                "matchedAtHeight": m.matched_at_height,
                "paidFee": m.paid_fee.to_string(),
                "payer": hex0x(&m.payer.0),
                "offer": offer_json(&m.offer),
                "note": "Off-chain match against node-local book. Submit PayBlobSample on-chain separately to pay the protocol sample fee.",
            }))
        }
        _ => Err((-32601, format!("unknown media blob method: {method}"))),
    }
}
