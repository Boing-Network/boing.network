//! JSON-RPC helpers for L1 media blobs.

use boing_primitives::{
    build_das_light_client_batch_from_bytes, build_das_light_client_batch_vrf,
    build_das_succinct_proof_stub, build_das_succinct_proof_stub_vrf, chunk_media_blob,
    match_sample_prefer_on_chain, media_blob_das_sample, parse_media_blob_id,
    phase8_cap_activation_checklist, verify_das_succinct_proof, AccountId, BlobRetention,
    BlobSampleOfferSource, BlobSampleSettleQuote, DasSuccinctProof, DasSuccinctProofKind,
    MediaBlobSampleOffer, MAX_MEDIA_BLOB_BYTES, MAX_MEDIA_BLOB_CONTENT_TYPE_LEN,
    MAX_MEDIA_BLOB_INLINE_BYTES, MAX_MEDIA_BLOB_INLINE_PREFER_BYTES,
    MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS, MAX_MEDIA_BLOB_TTL_BLOCKS, MEDIA_BLOBS_GOSSIP_TOPIC,
    MEDIA_BLOB_DAS_CHUNK_BYTES, MEDIA_BLOB_FOREVER_MULTIPLIER, MEDIA_BLOB_GAS_BASE,
    MEDIA_BLOB_GAS_PER_WORD, MEDIA_BLOB_RS_PARITY_SHARDS, MEDIA_BLOB_URI_PREFIX,
    MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS, PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES,
    PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES, PHASE6_MAX_SAMPLE_OFFERS, PHASE6_MAX_VIDEO_SEGMENTS,
    PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES, PHASE6_VIDEO_SEGMENT_TARGET_BYTES,
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
    "boing_quoteBlobSampleSettle",
    "boing_verifyBlobDasSuccinctProof",
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

fn offer_json_with_source(o: &MediaBlobSampleOffer, source: BlobSampleOfferSource) -> Value {
    let mut v = offer_json(o);
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "source".into(),
            json!(match source {
                BlobSampleOfferSource::OnChain => "onChain",
                BlobSampleOfferSource::Local => "local",
            }),
        );
    }
    v
}

fn quote_json(q: &BlobSampleSettleQuote) -> Value {
    json!({
        "blobId": hex0x(&q.blob_id),
        "chunkIndex": q.chunk_index,
        "chunkCount": q.chunk_count,
        "provider": hex0x(&q.provider.0),
        "protocolSampleFee": q.protocol_sample_fee.to_string(),
        "askFee": q.ask_fee.to_string(),
        "total": q.total.to_string(),
    })
}

fn proof_json(p: &DasSuccinctProof) -> Value {
    json!({
        "kind": p.kind,
        "kindName": match DasSuccinctProofKind::from_u8(p.kind) {
            Some(DasSuccinctProofKind::StubBlake3V0) => "stubBlake3V0",
            Some(DasSuccinctProofKind::ReservedRealV1) => "reservedRealV1",
            None => "unknown",
        },
        "commitment": hex0x(&p.commitment),
        "blobId": hex0x(&p.blob_id),
        "chunksRoot": hex0x(&p.chunks_root),
        "seedHeight": p.seed_height,
        "vrfOutput": p.vrf_output.as_ref().map(|v| hex0x(v)),
        "sampleIndices": p.sample_indices,
        "note": "StubBlake3V0 is a BLAKE3 commitment — not a SNARK. ReservedRealV1 rejects until wired.",
    })
}

fn parse_hex32(s: &str, field: &str) -> Result<[u8; 32], (i32, String)> {
    let raw = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(raw).map_err(|e| (-32602, format!("{field}: {e}")))?;
    if bytes.len() != 32 {
        return Err((-32602, format!("{field} must be 32 bytes")));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
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

fn parse_u128(v: &Value, key: &str) -> Result<u128, (i32, String)> {
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

fn parse_u128_field(params: &Value, key: &str) -> Result<u128, (i32, String)> {
    let v = params
        .get(key)
        .ok_or((-32602, format!("expected {key}")))?;
    parse_u128(v, key)
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
            let next = node.chain.height().saturating_add(1);
            let phase7_from = node.media_blobs_phase7_cap_activation_height();
            let mut m = serde_json::Map::new();
            let put = |m: &mut serde_json::Map<String, Value>, k: &str, v: Value| {
                m.insert(k.into(), v);
            };
            put(&mut m, "active", json!(node.media_blobs_active_at_tip()));
            put(
                &mut m,
                "activationHeight",
                json!(node.media_blobs_activation_height()),
            );
            put(&mut m, "maxBytes", json!(MAX_MEDIA_BLOB_BYTES));
            put(&mut m, "maxInlineBytes", json!(MAX_MEDIA_BLOB_INLINE_BYTES));
            put(
                &mut m,
                "maxInlinePreferBytes",
                json!(MAX_MEDIA_BLOB_INLINE_PREFER_BYTES),
            );
            put(
                &mut m,
                "maxContentTypeLen",
                json!(MAX_MEDIA_BLOB_CONTENT_TYPE_LEN),
            );
            put(&mut m, "maxTtlBlocks", json!(MAX_MEDIA_BLOB_TTL_BLOCKS));
            put(&mut m, "gasBase", json!(MEDIA_BLOB_GAS_BASE));
            put(&mut m, "gasPerWord", json!(MEDIA_BLOB_GAS_PER_WORD));
            put(
                &mut m,
                "foreverMultiplier",
                json!(MEDIA_BLOB_FOREVER_MULTIPLIER),
            );
            put(&mut m, "uriPrefix", json!(MEDIA_BLOB_URI_PREFIX));
            put(&mut m, "gossipTopic", json!(MEDIA_BLOBS_GOSSIP_TOPIC));
            put(&mut m, "putBlobVariant", json!(21));
            put(&mut m, "blobAnnounceVariant", json!(22));
            put(&mut m, "blobsRootInHeader", json!(true));
            put(
                &mut m,
                "blobsRootHeaderActivationHeight",
                json!(header_from),
            );
            put(
                &mut m,
                "blobsRootVerifiedWhenActivated",
                json!(header_from.is_some()),
            );
            put(
                &mut m,
                "phase3TargetMaxBytes",
                json!(PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES),
            );
            put(&mut m, "dasChunkBytes", json!(MEDIA_BLOB_DAS_CHUNK_BYTES));
            put(&mut m, "dasSampleRpc", json!("boing_getBlobDasSample"));
            put(
                &mut m,
                "dasLightClientBatchRpc",
                json!("boing_getBlobDasLightClientBatch"),
            );
            put(&mut m, "rsParityShards", json!(MEDIA_BLOB_RS_PARITY_SHARDS));
            put(&mut m, "renewMediaBlobPinVariant", json!(23));
            put(&mut m, "payBlobSampleVariant", json!(24));
            put(&mut m, "registerBlobSampleOfferVariant", json!(25));
            put(&mut m, "settleBlobSampleMatchVariant", json!(26));
            put(
                &mut m,
                "phase7ActivatedMaxBytes",
                json!(boing_primitives::PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES),
            );
            put(&mut m, "phase7CapActivationHeight", json!(phase7_from));
            put(
                &mut m,
                "phase7CapActiveAtTip",
                json!(phase7_from.is_some_and(|h| next >= h)),
            );
            put(
                &mut m,
                "effectiveMaxBytesAtTip",
                json!(node.executor.vm().media_blob_max_bytes(next)),
            );
            put(
                &mut m,
                "maxRenewExtendBlocks",
                json!(MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS),
            );
            put(
                &mut m,
                "videoPinHorizonBlocksHint",
                json!(MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS),
            );
            put(
                &mut m,
                "foreverPinFeePolicyField",
                json!("blob_forever_pin_fee_per_kib"),
            );
            put(
                &mut m,
                "pinRenewFeePolicyField",
                json!("blob_pin_renew_fee_per_kib_block"),
            );
            put(
                &mut m,
                "sampleFeePolicyField",
                json!("blob_sample_fee_per_chunk"),
            );
            put(
                &mut m,
                "phase6TargetMaxBytes",
                json!(PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES),
            );
            put(
                &mut m,
                "phase6VideoSegmentTargetBytes",
                json!(PHASE6_VIDEO_SEGMENT_TARGET_BYTES),
            );
            put(
                &mut m,
                "phase6MaxVideoSegments",
                json!(PHASE6_MAX_VIDEO_SEGMENTS),
            );
            put(
                &mut m,
                "phase6MaxDasLightClientSamples",
                json!(PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES),
            );
            put(
                &mut m,
                "phase6MaxSampleOffers",
                json!(PHASE6_MAX_SAMPLE_OFFERS),
            );
            put(
                &mut m,
                "sampleOfferRpcs",
                json!([
                    "boing_putBlobSampleOffer",
                    "boing_listBlobSampleOffers",
                    "boing_matchBlobSampleOffer",
                    "boing_quoteBlobSampleSettle"
                ]),
            );
            put(
                &mut m,
                "dasSuccinctProofRpc",
                json!("boing_verifyBlobDasSuccinctProof"),
            );
            put(
                &mut m,
                "dasSuccinctProofKinds",
                json!(["stubBlake3V0", "reservedRealV1"]),
            );
            put(
                &mut m,
                "capActivationChecklist",
                json!(phase8_cap_activation_checklist(phase7_from)),
            );
            put(
                &mut m,
                "note",
                json!("Phase 8: consensus-replay sample book (Register/Settle) + local gossip book; versioned succinct DAS proof envelope (StubBlake3V0 only); gated 256 MiB via BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT (unset = 16 MiB Fly-safe)."),
            );
            Ok(Value::Object(m))
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
            let vrf_hex = params
                .get(2)
                .or_else(|| params.get("vrfOutput"))
                .and_then(|v| v.as_str());
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
            let (batch, proof, seed_mode) = if let Some(vrf_s) = vrf_hex {
                let vrf = parse_hex32(vrf_s, "vrfOutput")?;
                let bundle = chunk_media_blob(&data).map_err(|e| (-32000, e.to_string()))?;
                let batch = build_das_light_client_batch_vrf(&bundle, &vrf, sample_count as usize)
                    .map_err(|e| (-32000, e.to_string()))?;
                let proof = build_das_succinct_proof_stub_vrf(&batch, &vrf);
                (batch, proof, "vrf")
            } else {
                let batch =
                    build_das_light_client_batch_from_bytes(&data, tip, sample_count as usize)
                        .map_err(|e| (-32000, e.to_string()))?;
                let proof = build_das_succinct_proof_stub(&batch);
                (batch, proof, "height")
            };
            if batch.blob_id != id {
                return Err((-32000, "stored bytes do not match blob id".into()));
            }
            Ok(json!({
                "blobId": hex0x(&batch.blob_id),
                "chunksRoot": hex0x(&batch.chunks_root),
                "seedHeight": batch.seed_height,
                "seedMode": seed_mode,
                "coverageRatio": batch.coverage_ratio(),
                "samples": batch.samples.iter().map(|s| json!({
                    "chunkIndex": s.chunk_index,
                    "chunkCount": s.chunk_count,
                    "chunkHash": hex0x(&s.chunk_hash),
                    "siblings": s.siblings.iter().map(|x| hex0x(x)).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "succinctProof": proof_json(&proof),
                "note": "Merkle LC samples + versioned succinct proof envelope. StubBlake3V0 is not a SNARK. Optional vrfOutput binds indices (caller must verify ECVRF).",
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
            let source_filter = params
                .get(2)
                .or_else(|| params.get("source"))
                .and_then(|v| v.as_str())
                .unwrap_or("all");
            let tip = node.chain.height();
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            let _ = store.prune_sample_offers(tip);
            let mut offers = Vec::new();
            if source_filter == "all" || source_filter == "onChain" {
                let on = store.onchain_sample_market_snapshot();
                for o in on.list_offers(&id, chunk_index, tip) {
                    offers.push(offer_json_with_source(o, BlobSampleOfferSource::OnChain));
                }
            }
            if source_filter == "all" || source_filter == "local" {
                let local = store.sample_market_snapshot();
                for o in local.list_offers(&id, chunk_index, tip) {
                    offers.push(offer_json_with_source(o, BlobSampleOfferSource::Local));
                }
            }
            Ok(json!({
                "blobId": hex0x(&id),
                "tipHeight": tip,
                "sourceFilter": source_filter,
                "offers": offers,
                "note": "onChain = Register/Settle replay book; local = boing_putBlobSampleOffer gossip book.",
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
            let protocol_fee = params
                .get("protocolSampleFee")
                .map(|v| parse_u128(v, "protocolSampleFee"))
                .transpose()?
                .unwrap_or(0);
            let tip = node.chain.height();
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            let _ = store.prune_sample_offers(tip);
            let on = store.onchain_sample_market_snapshot();
            let local = store.sample_market_snapshot();
            let (m, quote, src) = match_sample_prefer_on_chain(
                &on,
                &local,
                &blob_id,
                chunk_index,
                chunk_count,
                payer,
                paid_fee,
                tip,
                provider.as_ref(),
                protocol_fee,
            )
            .map_err(|e| (-32000, e.to_string()))?;
            Ok(json!({
                "matchedAtHeight": m.matched_at_height,
                "paidFee": m.paid_fee.to_string(),
                "payer": hex0x(&m.payer.0),
                "source": match src {
                    BlobSampleOfferSource::OnChain => "onChain",
                    BlobSampleOfferSource::Local => "local",
                },
                "offer": offer_json_with_source(&m.offer, src),
                "quote": quote_json(&quote),
                "note": "Prefers consensus-replay (Register) book, then local. Submit SettleBlobSampleMatch (26) on-chain; protocol fee via fee policy, ask_fee is provider quote (not SMT-escrowed yet).",
            }))
        }
        "boing_quoteBlobSampleSettle" => {
            let obj = params
                .as_object()
                .or_else(|| params.get(0).and_then(|v| v.as_object()))
                .ok_or((-32602, "expected quote object".into()))?;
            let params = Value::Object(obj.clone());
            let blob_id = parse_id_param(&params)?;
            let chunk_index = parse_u32(
                params
                    .get("chunkIndex")
                    .ok_or((-32602, "expected chunkIndex".into()))?,
                "chunkIndex",
            )?;
            let protocol_fee = params
                .get("protocolSampleFee")
                .map(|v| parse_u128(v, "protocolSampleFee"))
                .transpose()?
                .unwrap_or(0);
            let tip = node.chain.height();
            let store = node.media_blob_store().ok_or((
                -32000,
                "media blob store unavailable (no data dir)".into(),
            ))?;
            let _ = store.prune_sample_offers(tip);
            let on = store.onchain_sample_market_snapshot();
            let local = store.sample_market_snapshot();
            let (offer, src) = if let Some(o) = on.best_offer(&blob_id, chunk_index, tip) {
                (o.clone(), BlobSampleOfferSource::OnChain)
            } else if let Some(o) = local.best_offer(&blob_id, chunk_index, tip) {
                (o.clone(), BlobSampleOfferSource::Local)
            } else {
                return Err((-32000, "no matching sample offer".into()));
            };
            let quote = BlobSampleSettleQuote::from_offer(&offer, protocol_fee);
            Ok(json!({
                "source": match src {
                    BlobSampleOfferSource::OnChain => "onChain",
                    BlobSampleOfferSource::Local => "local",
                },
                "offer": offer_json_with_source(&offer, src),
                "quote": quote_json(&quote),
            }))
        }
        "boing_verifyBlobDasSuccinctProof" => {
            let obj = params
                .as_object()
                .or_else(|| params.get(0).and_then(|v| v.as_object()))
                .ok_or((-32602, "expected proof+batch object".into()))?;
            let params = Value::Object(obj.clone());
            let id = parse_id_param(&params)?;
            let sample_count = params
                .get("sampleCount")
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
                return Ok(json!({"ok": false, "reason": "blob not found"}));
            };
            let vrf_hex = params.get("vrfOutput").and_then(|v| v.as_str());
            let (batch, built) = if let Some(vrf_s) = vrf_hex {
                let vrf = parse_hex32(vrf_s, "vrfOutput")?;
                let bundle = chunk_media_blob(&data).map_err(|e| (-32000, e.to_string()))?;
                let batch = build_das_light_client_batch_vrf(&bundle, &vrf, sample_count as usize)
                    .map_err(|e| (-32000, e.to_string()))?;
                let built = build_das_succinct_proof_stub_vrf(&batch, &vrf);
                (batch, built)
            } else {
                let batch =
                    build_das_light_client_batch_from_bytes(&data, tip, sample_count as usize)
                        .map_err(|e| (-32000, e.to_string()))?;
                let built = build_das_succinct_proof_stub(&batch);
                (batch, built)
            };
            // Optional client-supplied commitment to check against rebuilt proof.
            let ok = if let Some(c) = params.get("commitment").and_then(|v| v.as_str()) {
                let got = parse_hex32(c, "commitment")?;
                let mut proof = built.clone();
                proof.commitment = got;
                match verify_das_succinct_proof(&proof, &batch) {
                    Ok(()) => true,
                    Err(_) => false,
                }
            } else {
                verify_das_succinct_proof(&built, &batch).is_ok()
            };
            Ok(json!({
                "ok": ok,
                "proof": proof_json(&built),
                "note": "Verifies StubBlake3V0 against local merkle LC batch. Not a SNARK verifier.",
            }))
        }
        _ => Err((-32601, format!("unknown media blob method: {method}"))),
    }
}
