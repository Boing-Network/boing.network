//! JSON-RPC helpers for L1 media blobs.

use boing_primitives::{
    build_das_light_client_batch_from_bytes, build_das_light_client_batch_vrf,
    build_das_succinct_proof_stub, build_das_succinct_proof_stub_vrf, chunk_media_blob,
    match_sample_prefer_on_chain, media_blob_das_sample, parse_media_blob_id,
    phase28_cap_activation_checklist, phase28_das_succinct_proof_plan, verify_das_succinct_proof,
    AccountId, BlobRetention, BlobSampleOfferSource, BlobSampleServeChallengeStatus,
    BlobSampleSettleQuote, BlsSignatureG2, DasSuccinctProof, DasSuccinctProofKind,
    MediaBlobSampleOffer, MAX_MEDIA_BLOB_BYTES, MAX_MEDIA_BLOB_CONTENT_TYPE_LEN,
    MAX_MEDIA_BLOB_INLINE_BYTES, MAX_MEDIA_BLOB_INLINE_PREFER_BYTES,
    MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS, MAX_MEDIA_BLOB_TTL_BLOCKS, MEDIA_BLOBS_GOSSIP_TOPIC,
    MEDIA_BLOB_DAS_CHUNK_BYTES, MEDIA_BLOB_FOREVER_MULTIPLIER, MEDIA_BLOB_GAS_BASE,
    MEDIA_BLOB_GAS_PER_WORD, MEDIA_BLOB_RS_PARITY_SHARDS, MEDIA_BLOB_URI_PREFIX,
    MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS, PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES,
    PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES, PHASE6_MAX_SAMPLE_OFFERS, PHASE6_MAX_VIDEO_SEGMENTS,
    PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES, PHASE6_VIDEO_SEGMENT_TARGET_BYTES,
    PHASE28_BLS_AGGREGATE_WIRED, PHASE28_BLS_DUAL_KEY_REGISTRY_FILE, PHASE28_BLS_POP_REQUIRED,
    PHASE28_BLS_PUBLIC_KEY_G1_BYTES, PHASE28_BLS_SIGNATURE_G2_BYTES,
    PHASE28_EXPIRED_APPEAL_RETENTION_BLOCKS, PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX,
    PHASE28_RESOLVE_THRESHOLD_DEN, PHASE28_RESOLVE_THRESHOLD_NUM, PHASE28_RESOLVE_WINDOW_BLOCKS,
    PHASE28_RESOLVED_APPEAL_RETENTION_BLOCKS, PHASE28_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS,
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
    "boing_getLeaderVrfOutput",
    "boing_listSmtBlobSampleOffers",
    "boing_listBlobSampleServeChallenges",
    "boing_getBlobSampleServeChallenge",
    "boing_getBlobDasSuccinctProofPlan",
    "boing_listBlsDualKeys",
    "boing_verifyBlsAggregate",
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
                json!(phase28_cap_activation_checklist(phase7_from)),
            );
            put(&mut m, "cancelBlobSampleOfferVariant", json!(27));
            put(&mut m, "challengeBlobSampleServeVariant", json!(28));
            put(&mut m, "respondBlobSampleServeVariant", json!(29));
            put(
                &mut m,
                "submitSlashAppealVariant",
                json!(boing_primitives::PHASE14_SUBMIT_SLASH_APPEAL_TX_INDEX),
            );
            put(
                &mut m,
                "resolveSlashAppealVariant",
                json!(boing_primitives::PHASE14_RESOLVE_SLASH_APPEAL_TX_INDEX),
            );
            put(
                &mut m,
                "resolveSlashAppealAttestedVariant",
                json!(PHASE28_RESOLVE_SLASH_APPEAL_ATTESTED_TX_INDEX),
            );
            put(&mut m, "consensusSlashAppealTxs", json!(true));
            put(&mut m, "consensusEquivocationSlashAppeals", json!(true));
            put(&mut m, "consensusSlashAppealThresholdResolve", json!(true));
            put(
                &mut m,
                "consensusSlashAppealStakeWeightedResolve",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealAttestedResolve",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealCommitteeAttestedRequired",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealElectorateSnapshot",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealResolveExpiry",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealBallotReverify",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealExpiredPrune",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealExpiredRetentionBlocks",
                json!(PHASE28_EXPIRED_APPEAL_RETENTION_BLOCKS),
            );
            put(
                &mut m,
                "consensusSlashAppealResolvedPrune",
                json!(true),
            );
            put(
                &mut m,
                "consensusSlashAppealResolvedRetentionBlocks",
                json!(PHASE28_RESOLVED_APPEAL_RETENTION_BLOCKS),
            );
            put(
                &mut m,
                "blsAggregateScaffold",
                json!(true),
            );
            put(
                &mut m,
                "blsAggregateWired",
                json!(PHASE28_BLS_AGGREGATE_WIRED),
            );
            put(
                &mut m,
                "blsPublicKeyG1Bytes",
                json!(PHASE28_BLS_PUBLIC_KEY_G1_BYTES),
            );
            put(
                &mut m,
                "blsSignatureG2Bytes",
                json!(PHASE28_BLS_SIGNATURE_G2_BYTES),
            );
            put(
                &mut m,
                "blsDualKeyRegistryFile",
                json!(PHASE28_BLS_DUAL_KEY_REGISTRY_FILE),
            );
            put(
                &mut m,
                "blsDualKeyCount",
                json!(node.bls_dual_key_registry.len()),
            );
            put(
                &mut m,
                "blsDualKeyRpcs",
                json!([
                    "boing_listBlsDualKeys",
                    "boing_registerBlsDualKey",
                    "boing_verifyBlsAggregate"
                ]),
            );
            put(
                &mut m,
                "blsPopRequired",
                json!(PHASE28_BLS_POP_REQUIRED),
            );
            put(
                &mut m,
                "blsLiveTx32StillEd25519",
                json!(true),
            );
            put(
                &mut m,
                "serveChallengeTerminalPrune",
                json!(true),
            );
            put(
                &mut m,
                "serveChallengeTerminalRetentionBlocks",
                json!(PHASE28_TERMINAL_SERVE_CHALLENGE_RETENTION_BLOCKS),
            );
            put(
                &mut m,
                "slashAppealBallotReverifyRpc",
                json!("boing_reverifySlashAppealBallots"),
            );
            put(
                &mut m,
                "slashAppealResolveThresholdNum",
                json!(PHASE28_RESOLVE_THRESHOLD_NUM),
            );
            put(
                &mut m,
                "slashAppealResolveThresholdDen",
                json!(PHASE28_RESOLVE_THRESHOLD_DEN),
            );
            put(
                &mut m,
                "slashAppealResolveWindowBlocks",
                json!(PHASE28_RESOLVE_WINDOW_BLOCKS),
            );
            put(
                &mut m,
                "maxAppealEvidenceBytes",
                json!(boing_primitives::PHASE14_MAX_APPEAL_EVIDENCE_BYTES),
            );
            put(
                &mut m,
                "serveChallengeWindowBlocks",
                json!(boing_primitives::PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS),
            );
            put(
                &mut m,
                "askEscrowAccount",
                json!(hex0x(&boing_primitives::blob_sample_ask_escrow_account().0)),
            );
            put(
                &mut m,
                "smtSampleOfferCount",
                json!(node.state.sample_offers().len()),
            );
            put(
                &mut m,
                "smtServeChallengeCount",
                json!(node.state.sample_serve().len()),
            );
            put(
                &mut m,
                "dasSuccinctProofPlan",
                json!(phase28_das_succinct_proof_plan()),
            );
            put(
                &mut m,
                "serveChallengeBond",
                json!(boing_primitives::PHASE11_SERVE_CHALLENGE_BOND.to_string()),
            );
            put(
                &mut m,
                "failedServePenalty",
                json!(boing_primitives::PHASE11_FAILED_SERVE_PENALTY.to_string()),
            );
            put(
                &mut m,
                "failedServeAppealWindowBlocks",
                json!(boing_primitives::PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS),
            );
            put(
                &mut m,
                "failedServeAppealRestoresBalance",
                json!(true),
            );
            put(
                &mut m,
                "requireSettleForChallenge",
                json!(true),
            );
            put(
                &mut m,
                "settleReceiptCount",
                json!(node.state.sample_offers().settle_receipt_count()),
            );
            put(
                &mut m,
                "settleReceiptTtlBlocks",
                json!(boing_primitives::PHASE14_SETTLE_RECEIPT_TTL_BLOCKS),
            );
            put(
                &mut m,
                "serveChallengeBondEscrow",
                json!(hex0x(
                    &boing_primitives::blob_sample_serve_challenge_bond_escrow_account().0
                )),
            );
            put(
                &mut m,
                "note",
                json!("Phase 28: BLS dual-key PoP required on boing_registerBlsDualKey (PHASE28_BLS_POP_REQUIRED=true); blst aggregate library/RPC still wired. Live tx 32 stays Ed25519 verify-each. Slash appeals stay V5 (do not clear); bls_dual_key_registry.bin stays BDKR v1. Aggregate submit / flip committee still later. ReservedRealV1 verify still rejects; 256 MiB gated via BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT (unset = 16 MiB Fly-safe). Admission height: BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT (do not change once set)."),
            );
            Ok(Value::Object(m))
        }
        "boing_listBlsDualKeys" => {
            let entries: Vec<Value> = node
                .bls_dual_key_registry
                .list()
                .into_iter()
                .map(|id| {
                    json!({
                        "accountId": hex0x(&id.account_id.0),
                        "blsPubkey": hex0x(id.bls_pubkey.as_bytes()),
                    })
                })
                .collect();
            Ok(json!({
                "count": entries.len(),
                "file": PHASE28_BLS_DUAL_KEY_REGISTRY_FILE,
                "wired": PHASE28_BLS_AGGREGATE_WIRED,
                "popRequired": PHASE28_BLS_POP_REQUIRED,
                "liveTx32StillEd25519": true,
                "entries": entries,
            }))
        }
        "boing_verifyBlsAggregate" => {
            let obj = params
                .as_object()
                .or_else(|| {
                    params
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_object())
                })
                .ok_or((
                    -32602,
                    "expected {message, aggregate, accountIds} (or positional object)".into(),
                ))?;
            let msg_hex = obj
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or((-32602, "expected message hex (32 bytes)".into()))?;
            let message = parse_hex32(msg_hex, "message")?;
            let agg_hex = obj
                .get("aggregate")
                .and_then(|v| v.as_str())
                .ok_or((-32602, "expected aggregate hex (96 bytes)".into()))?;
            let agg_raw = hex::decode(agg_hex.strip_prefix("0x").unwrap_or(agg_hex))
                .map_err(|e| (-32602, format!("aggregate: {e}")))?;
            let aggregate = BlsSignatureG2::try_from_slice(&agg_raw)
                .map_err(|e| (-32602, e.to_string()))?;
            let accounts_v = obj
                .get("accountIds")
                .or_else(|| obj.get("accounts"))
                .ok_or((-32602, "expected accountIds array".into()))?;
            let arr = accounts_v
                .as_array()
                .ok_or((-32602, "accountIds must be an array".into()))?;
            let mut accounts = Vec::with_capacity(arr.len());
            for v in arr {
                let s = v
                    .as_str()
                    .ok_or((-32602, "accountIds entries must be hex strings".into()))?;
                accounts.push(parse_account(s)?);
            }
            match node.bls_dual_key_registry.verify_aggregate_for_accounts(
                &message,
                &aggregate,
                &accounts,
            ) {
                Ok(()) => Ok(json!({
                    "ok": true,
                    "wired": PHASE28_BLS_AGGREGATE_WIRED,
                    "liveTx32StillEd25519": true,
                    "signerCount": accounts.len(),
                })),
                Err(e) => Ok(json!({
                    "ok": false,
                    "error": e.to_string(),
                    "wired": PHASE28_BLS_AGGREGATE_WIRED,
                    "liveTx32StillEd25519": true,
                    "signerCount": accounts.len(),
                })),
            }
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
            let use_leader_vrf = params
                .get("useLeaderVrf")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
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
            let leader_vrf = if use_leader_vrf {
                node.consensus.leader_vrf_output(tip)
            } else {
                None
            };
            let (batch, proof, seed_mode) = if let Some(vrf_s) = vrf_hex {
                let vrf = parse_hex32(vrf_s, "vrfOutput")?;
                let bundle = chunk_media_blob(&data).map_err(|e| (-32000, e.to_string()))?;
                let batch = build_das_light_client_batch_vrf(&bundle, &vrf, sample_count as usize)
                    .map_err(|e| (-32000, e.to_string()))?;
                let proof = build_das_succinct_proof_stub_vrf(&batch, &vrf);
                (batch, proof, "vrf")
            } else if let Some(vrf) = leader_vrf {
                let bundle = chunk_media_blob(&data).map_err(|e| (-32000, e.to_string()))?;
                let batch = build_das_light_client_batch_vrf(&bundle, &vrf, sample_count as usize)
                    .map_err(|e| (-32000, e.to_string()))?;
                let proof = build_das_succinct_proof_stub_vrf(&batch, &vrf);
                (batch, proof, "leaderVrf")
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
        "boing_getLeaderVrfOutput" => {
            let round = params
                .get(0)
                .or_else(|| params.get("round"))
                .or_else(|| params.get("height"))
                .map(|v| parse_u64_field(&json!({ "round": v }), &["round"], None))
                .transpose()?
                .unwrap_or_else(|| node.chain.height());
            let mode = format!("{:?}", node.consensus.leader_election());
            match node.consensus.leader_vrf_output(round) {
                Some(out) => Ok(json!({
                    "round": round,
                    "leaderElection": mode,
                    "vrfOutput": hex0x(&out),
                    "note": "VRF mode: ECVRF winner output when full set present, else dummy_vrf_output. RoundRobin returns null — use height-bound LC seeds.",
                })),
                None => Ok(json!({
                    "round": round,
                    "leaderElection": mode,
                    "vrfOutput": Value::Null,
                    "note": "RoundRobin (or no VRF output): DAS LC should use height-bound seeds.",
                })),
            }
        }
        "boing_listSmtBlobSampleOffers" => {
            let id = parse_id_param(params)?;
            let chunk_index = params
                .get(1)
                .or_else(|| params.get("chunkIndex"))
                .map(|v| parse_u32(v, "chunkIndex"))
                .transpose()?;
            let tip = node.chain.height();
            let offers = node.state.sample_offers().list(&id, chunk_index, tip);
            Ok(json!({
                "blobId": hex0x(&id),
                "tipHeight": tip,
                "source": "smt",
                "offers": offers.iter().map(|o| {
                    let mut v = offer_json(&o.to_media_offer());
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("source".into(), json!("smt"));
                        obj.insert("registeredAtHeight".into(), json!(o.registered_at_height));
                        obj.insert("escrowedAsk".into(), json!(o.ask_fee.to_string()));
                    }
                    v
                }).collect::<Vec<_>>(),
                "note": "SMT-resident SampleOfferRegistry (Phase 9). ask_fee is escrowed in the system ask account.",
            }))
        }
        "boing_listBlobSampleServeChallenges" => {
            // Phase 24: [blobId] or { blobId?, status?, includeTerminal? }.
            // blobId optional when using object form (lists all).
            let filter_obj = if params.is_object() {
                Some(params.clone())
            } else {
                params.as_array().and_then(|a| a.first()).and_then(|v| {
                    if v.is_object() {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
            };
            let blob_id_opt = match filter_obj.as_ref() {
                Some(o) => o
                    .get("blobId")
                    .or_else(|| o.get("id"))
                    .and_then(|v| v.as_str())
                    .and_then(parse_media_blob_id),
                None => parse_id_param(params).ok(),
            };
            let status_filter = filter_obj.as_ref().and_then(|o| {
                o.get("status")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_ascii_lowercase())
            });
            // Default true for back-compat (list still shows Responded/FailedServe).
            let include_terminal = filter_obj
                .as_ref()
                .and_then(|o| {
                    o.get("includeTerminal")
                        .or_else(|| o.get("include_terminal"))
                        .and_then(|v| v.as_bool())
                })
                .unwrap_or(true);
            let tip = node.chain.height();
            let mut reg = node.state.sample_serve().clone();
            let expired = reg.resolve_expired(tip);
            let list: Vec<_> = match blob_id_opt {
                Some(id) => reg.list_for_blob(&id, tip),
                None => reg
                    .list_all()
                    .into_iter()
                    .filter(|c| {
                        !(c.status == BlobSampleServeChallengeStatus::Open
                            && tip > c.deadline_height)
                    })
                    .collect(),
            };
            let challenges: Vec<Value> = list
                .iter()
                .filter(|c| {
                    let status = match c.status {
                        BlobSampleServeChallengeStatus::Open => "open",
                        BlobSampleServeChallengeStatus::Responded => "responded",
                        BlobSampleServeChallengeStatus::FailedServe => "failedServe",
                    };
                    if !include_terminal
                        && matches!(
                            c.status,
                            BlobSampleServeChallengeStatus::Responded
                                | BlobSampleServeChallengeStatus::FailedServe
                        )
                    {
                        return false;
                    }
                    if let Some(ref want) = status_filter {
                        return status == want.as_str();
                    }
                    true
                })
                .map(|c| {
                    json!({
                        "blobId": hex0x(&c.blob_id),
                        "chunkIndex": c.chunk_index,
                        "chunkCount": c.chunk_count,
                        "provider": hex0x(&c.provider.0),
                        "challenger": hex0x(&c.challenger.0),
                        "openedAtHeight": c.opened_at_height,
                        "deadlineHeight": c.deadline_height,
                        "bondAmount": c.bond_amount.to_string(),
                        "providerPenalty": c.provider_penalty.to_string(),
                        "bondSettled": c.bond_settled,
                        "slashId": c.slash_id,
                        "status": match c.status {
                            BlobSampleServeChallengeStatus::Open => "open",
                            BlobSampleServeChallengeStatus::Responded => "responded",
                            BlobSampleServeChallengeStatus::FailedServe => "failedServe",
                        },
                    })
                })
                .collect();
            Ok(json!({
                "blobId": blob_id_opt.map(|id| hex0x(&id)),
                "tipHeight": tip,
                "resolvedExpiredThisCall": expired,
                "challenges": challenges,
                "note": "Phase 24: optional status (open|responded|failedServe) and includeTerminal (default true). Terminal rows prune after deadline+4096 when bondSettled. RPC clone resolve is display-only.",
            }))
        }
        "boing_getBlobSampleServeChallenge" => {
            // Phase 24 get-by-key: { blobId, chunkIndex, provider, challenger }
            let obj = if params.is_object() {
                params.clone()
            } else {
                params
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_object())
                    .map(|o| Value::Object(o.clone()))
                    .ok_or((-32602, "expected object { blobId, chunkIndex, provider, challenger }".into()))?
            };
            let blob_s = obj
                .get("blobId")
                .or_else(|| obj.get("id"))
                .and_then(|v| v.as_str())
                .ok_or((-32602, "expected blobId".into()))?;
            let id = parse_media_blob_id(blob_s)
                .ok_or((-32602, format!("invalid blob id: {blob_s}")))?;
            let chunk_index = parse_u32(
                obj.get("chunkIndex")
                    .or_else(|| obj.get("chunk_index"))
                    .ok_or((-32602, "expected chunkIndex".into()))?,
                "chunkIndex",
            )?;
            let provider = parse_account(
                obj.get("provider")
                    .and_then(|v| v.as_str())
                    .ok_or((-32602, "expected provider".into()))?,
            )?;
            let challenger = parse_account(
                obj.get("challenger")
                    .and_then(|v| v.as_str())
                    .ok_or((-32602, "expected challenger".into()))?,
            )?;
            let tip = node.chain.height();
            let mut reg = node.state.sample_serve().clone();
            let _ = reg.resolve_expired(tip);
            match reg.get(&id, chunk_index, &provider, &challenger) {
                Some(c) => Ok(json!({
                    "tipHeight": tip,
                    "challenge": {
                        "blobId": hex0x(&c.blob_id),
                        "chunkIndex": c.chunk_index,
                        "chunkCount": c.chunk_count,
                        "provider": hex0x(&c.provider.0),
                        "challenger": hex0x(&c.challenger.0),
                        "openedAtHeight": c.opened_at_height,
                        "deadlineHeight": c.deadline_height,
                        "bondAmount": c.bond_amount.to_string(),
                        "providerPenalty": c.provider_penalty.to_string(),
                        "bondSettled": c.bond_settled,
                        "slashId": c.slash_id,
                        "status": match c.status {
                            BlobSampleServeChallengeStatus::Open => "open",
                            BlobSampleServeChallengeStatus::Responded => "responded",
                            BlobSampleServeChallengeStatus::FailedServe => "failedServe",
                        },
                    },
                })),
                None => Ok(Value::Null),
            }
        }
        "boing_getBlobDasSuccinctProofPlan" => Ok(json!({
            "kinds": ["stubBlake3V0", "reservedRealV1"],
            "plan": phase28_das_succinct_proof_plan(),
            "maxOpaqueProofBytes": boing_primitives::PHASE10_MAX_OPAQUE_PROOF_BYTES,
            "note": "ReservedRealV1: envelope structure checks; verify still rejects until a real succinct system is wired. ResolveSlashAppealAttested (32) is Ed25519 ballot verify-each — live cast unchanged in Phase 28; library/RPC BLS aggregate is wired; dual-key registration requires BLS PoP. Committee nets must use 32; 31 remains unattested stake-weighted ⌈2W/3⌉ for solo/unpinned/|pinned|≤1 — not SNARK. Phase 25 ancient Approved/Rejected appeal prune; Phase 24 serve-challenge terminal prune; Phase 23 Expired appeal prune; Phase 22 ballot V2 + re-verify.",
        })),
        _ => Err((-32601, format!("unknown media blob method: {method}"))),
    }
}
