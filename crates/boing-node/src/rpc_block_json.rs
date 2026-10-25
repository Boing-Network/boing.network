//! Compact JSON for `boing_getBlockByHeight` / `boing_getBlockByHash`.
//!
//! Default serde of `Block` emits large `Vec<u8>` fields (deploy bytecode, calldata,
//! blob bytes) as **JSON number arrays**, which inflate ~3–4× and can produce
//! multi‑hundred‑MiB responses that OOM the node and truncate on the public RPC edge.
//!
//! This module always emits those fields as **`0x` hex** (same style as receipts), and
//! supports **`include_code: false`** to replace heavy byte vectors with
//! `{ omitted, size, hash }` stubs so explorers can sample tip windows safely.

use boing_primitives::{
    hasher, AccessList, AccountId, BatchCall, Block, Hash, QaPoolVoteKind, Transaction,
    TransactionPayload,
};
use boing_primitives::signature::Signature;
use serde_json::{json, Value};

fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn account_json(a: &AccountId) -> Value {
    Value::String(hex0x(&a.0))
}

fn hash_json(h: &Hash) -> Value {
    Value::String(hex0x(&h.0))
}

fn sig_json(s: &Signature) -> Value {
    Value::String(hex0x(&s.0))
}

fn u128_json(n: u128) -> Value {
    // Match `json_value_lossless`: integers above JS MAX_SAFE_INTEGER as decimal strings.
    const JS_MAX_SAFE: u128 = (1 << 53) - 1;
    if n > JS_MAX_SAFE {
        Value::String(n.to_string())
    } else {
        Value::Number((n as u64).into())
    }
}

fn bytes32_json(b: &[u8; 32]) -> Value {
    Value::String(hex0x(b))
}

/// Hex when `include_code`, else a stub with BLAKE3 hash so clients can still fingerprint.
fn bytes_field(bytes: &[u8], include_code: bool) -> Value {
    if include_code {
        Value::String(hex0x(bytes))
    } else {
        let mut h = hasher();
        h.update(bytes);
        let digest = h.finalize();
        json!({
            "omitted": true,
            "size": bytes.len(),
            "hash": hex0x(digest.as_bytes()),
        })
    }
}

fn opt_bytes_field(bytes: &Option<Vec<u8>>, include_code: bool) -> Value {
    match bytes {
        None => Value::Null,
        Some(b) if b.is_empty() => Value::Null,
        Some(b) => bytes_field(b, include_code),
    }
}

fn access_list_json(al: &AccessList) -> Value {
    json!({
        "read": al.read.iter().map(account_json).collect::<Vec<_>>(),
        "write": al.write.iter().map(account_json).collect::<Vec<_>>(),
    })
}

fn vote_json(v: &QaPoolVoteKind) -> Value {
    // Matches `#[serde(rename_all = "lowercase")]` on `QaPoolVoteKind`.
    match v {
        QaPoolVoteKind::Allow => Value::String("allow".into()),
        QaPoolVoteKind::Reject => Value::String("reject".into()),
        QaPoolVoteKind::Abstain => Value::String("abstain".into()),
    }
}

fn batch_call_json(call: &BatchCall, include_code: bool) -> Value {
    match call {
        BatchCall::Transfer { to, amount } => json!({
            "Transfer": { "to": account_json(to), "amount": u128_json(*amount) }
        }),
        BatchCall::ContractCall { contract, calldata } => json!({
            "ContractCall": {
                "contract": account_json(contract),
                "calldata": bytes_field(calldata, include_code),
            }
        }),
        BatchCall::ContractCallWithValue {
            contract,
            calldata,
            value,
        } => json!({
            "ContractCallWithValue": {
                "contract": account_json(contract),
                "calldata": bytes_field(calldata, include_code),
                "value": u128_json(*value),
            }
        }),
    }
}

fn payload_json(payload: &TransactionPayload, include_code: bool) -> Value {
    match payload {
        TransactionPayload::Transfer { to, amount } => json!({
            "Transfer": { "to": account_json(to), "amount": u128_json(*amount) }
        }),
        TransactionPayload::ContractCall { contract, calldata } => json!({
            "ContractCall": {
                "contract": account_json(contract),
                "calldata": bytes_field(calldata, include_code),
            }
        }),
        TransactionPayload::ContractDeploy {
            bytecode,
            create2_salt,
        } => json!({
            "ContractDeploy": {
                "bytecode": bytes_field(bytecode, include_code),
                "create2_salt": create2_salt.as_ref().map(bytes32_json),
            }
        }),
        TransactionPayload::ContractDeployWithPurpose {
            bytecode,
            purpose_category,
            description_hash,
            create2_salt,
        } => json!({
            "ContractDeployWithPurpose": {
                "bytecode": bytes_field(bytecode, include_code),
                "purpose_category": purpose_category,
                "description_hash": opt_bytes_field(description_hash, include_code),
                "create2_salt": create2_salt.as_ref().map(bytes32_json),
            }
        }),
        TransactionPayload::ContractDeployWithPurposeAndMetadata {
            bytecode,
            purpose_category,
            description_hash,
            asset_name,
            asset_symbol,
            create2_salt,
        } => json!({
            "ContractDeployWithPurposeAndMetadata": {
                "bytecode": bytes_field(bytecode, include_code),
                "purpose_category": purpose_category,
                "description_hash": opt_bytes_field(description_hash, include_code),
                "asset_name": asset_name,
                "asset_symbol": asset_symbol,
                "create2_salt": create2_salt.as_ref().map(bytes32_json),
            }
        }),
        TransactionPayload::Bond { amount } => json!({ "Bond": { "amount": u128_json(*amount) } }),
        TransactionPayload::Unbond { amount } => {
            json!({ "Unbond": { "amount": u128_json(*amount) } })
        }
        TransactionPayload::ClaimUnbond => Value::String("ClaimUnbond".into()),
        TransactionPayload::QaPoolVote { subject, vote } => json!({
            "QaPoolVote": { "subject": hash_json(subject), "vote": vote_json(vote) }
        }),
        TransactionPayload::Batch { calls } => json!({
            "Batch": {
                "calls": calls.iter().map(|c| batch_call_json(c, include_code)).collect::<Vec<_>>(),
            }
        }),
        TransactionPayload::ValidatorRegister {
            node_id,
            node_signature,
        } => json!({
            "ValidatorRegister": {
                "node_id": account_json(node_id),
                "node_signature": bytes_field(node_signature, include_code),
            }
        }),
        TransactionPayload::ValidatorDeregister { node_id } => {
            json!({ "ValidatorDeregister": { "node_id": account_json(node_id) } })
        }
        TransactionPayload::ValidatorUnjail { node_id } => {
            json!({ "ValidatorUnjail": { "node_id": account_json(node_id) } })
        }
        TransactionPayload::NameCommit { commitment } => {
            json!({ "NameCommit": { "commitment": bytes32_json(commitment) } })
        }
        TransactionPayload::NameRegister {
            name,
            salt,
            target,
            years,
        } => json!({
            "NameRegister": {
                "name": name,
                "salt": bytes32_json(salt),
                "target": account_json(target),
                "years": years,
            }
        }),
        TransactionPayload::NameRenew { name, years } => {
            json!({ "NameRenew": { "name": name, "years": years } })
        }
        TransactionPayload::NameTransfer { name, new_owner } => {
            json!({ "NameTransfer": { "name": name, "new_owner": account_json(new_owner) } })
        }
        TransactionPayload::NameSetTarget { name, target } => {
            json!({ "NameSetTarget": { "name": name, "target": account_json(target) } })
        }
        TransactionPayload::NameSetPrimary { name } => {
            json!({ "NameSetPrimary": { "name": name } })
        }
        TransactionPayload::NameRelease { name } => json!({ "NameRelease": { "name": name } }),
        TransactionPayload::ContractCallWithValue {
            contract,
            calldata,
            value,
        } => json!({
            "ContractCallWithValue": {
                "contract": account_json(contract),
                "calldata": bytes_field(calldata, include_code),
                "value": u128_json(*value),
            }
        }),
        TransactionPayload::PutBlob {
            retention,
            content_type,
            data,
        } => json!({
            "PutBlob": {
                "retention": retention,
                "content_type": content_type,
                "data": bytes_field(data, include_code),
            }
        }),
        TransactionPayload::BlobAnnounce {
            blob_id,
            size,
            retention,
            content_type,
        } => json!({
            "BlobAnnounce": {
                "blob_id": hex0x(blob_id),
                "size": size,
                "retention": retention,
                "content_type": content_type,
            }
        }),
        TransactionPayload::RenewMediaBlobPin {
            blob_id,
            size,
            extend_blocks,
        } => json!({
            "RenewMediaBlobPin": {
                "blob_id": hex0x(blob_id),
                "size": size,
                "extend_blocks": extend_blocks,
            }
        }),
        TransactionPayload::PayBlobSample {
            blob_id,
            chunk_index,
            chunk_count,
        } => json!({
            "PayBlobSample": {
                "blob_id": hex0x(blob_id),
                "chunk_index": chunk_index,
                "chunk_count": chunk_count,
            }
        }),
        TransactionPayload::RegisterBlobSampleOffer {
            blob_id,
            chunk_index,
            chunk_count,
            ask_fee,
            expires_at_height,
        } => json!({
            "RegisterBlobSampleOffer": {
                "blob_id": hex0x(blob_id),
                "chunk_index": chunk_index,
                "chunk_count": chunk_count,
                "ask_fee": u128_json(*ask_fee),
                "expires_at_height": expires_at_height,
            }
        }),
        TransactionPayload::SettleBlobSampleMatch {
            blob_id,
            chunk_index,
            chunk_count,
            provider,
        } => json!({
            "SettleBlobSampleMatch": {
                "blob_id": hex0x(blob_id),
                "chunk_index": chunk_index,
                "chunk_count": chunk_count,
                "provider": account_json(provider),
            }
        }),
        TransactionPayload::CancelBlobSampleOffer {
            blob_id,
            chunk_index,
        } => json!({
            "CancelBlobSampleOffer": {
                "blob_id": hex0x(blob_id),
                "chunk_index": chunk_index,
            }
        }),
        TransactionPayload::ChallengeBlobSampleServe {
            blob_id,
            chunk_index,
            chunk_count,
            provider,
        } => json!({
            "ChallengeBlobSampleServe": {
                "blob_id": hex0x(blob_id),
                "chunk_index": chunk_index,
                "chunk_count": chunk_count,
                "provider": account_json(provider),
            }
        }),
        TransactionPayload::RespondBlobSampleServe {
            blob_id,
            chunk_index,
            chunk_count,
            challenger,
            chunk_hash,
            chunks_root,
            siblings,
        } => json!({
            "RespondBlobSampleServe": {
                "blob_id": hex0x(blob_id),
                "chunk_index": chunk_index,
                "chunk_count": chunk_count,
                "challenger": account_json(challenger),
                "chunk_hash": bytes32_json(chunk_hash),
                "chunks_root": bytes32_json(chunks_root),
                "siblings": siblings.iter().map(bytes32_json).collect::<Vec<_>>(),
            }
        }),
        TransactionPayload::SubmitSlashAppeal { slash_id, evidence } => json!({
            "SubmitSlashAppeal": {
                "slash_id": slash_id,
                "evidence": bytes_field(evidence, include_code),
            }
        }),
        TransactionPayload::ResolveSlashAppeal {
            appeal_id,
            approved,
        } => json!({
            "ResolveSlashAppeal": { "appeal_id": appeal_id, "approved": approved }
        }),
        TransactionPayload::ResolveSlashAppealAttested {
            appeal_id,
            approved,
            ballot_signature,
        } => json!({
            "ResolveSlashAppealAttested": {
                "appeal_id": appeal_id,
                "approved": approved,
                // Opaque attested ballot — keep serde shape via hex of inner bytes when possible.
                "ballot_signature": serde_json::to_value(ballot_signature).unwrap_or(Value::Null),
            }
        }),
        TransactionPayload::ResolveSlashAppealBlsAggregate {
            appeal_id,
            approved,
            signer_bitset,
            aggregate_signature,
            signer_bls_pubkeys,
        } => json!({
            "ResolveSlashAppealBlsAggregate": {
                "appeal_id": appeal_id,
                "approved": approved,
                "signer_bitset": bytes_field(signer_bitset, include_code),
                "aggregate_signature": serde_json::to_value(aggregate_signature).unwrap_or(Value::Null),
                "signer_bls_pubkeys": serde_json::to_value(signer_bls_pubkeys).unwrap_or(Value::Null),
            }
        }),
        TransactionPayload::GrantBlobDownload { blob_id, grantee } => json!({
            "GrantBlobDownload": {
                "blob_id": hex0x(blob_id),
                "grantee": account_json(grantee),
            }
        }),
        TransactionPayload::RevokeBlobDownload { blob_id, grantee } => json!({
            "RevokeBlobDownload": {
                "blob_id": hex0x(blob_id),
                "grantee": account_json(grantee),
            }
        }),
    }
}

fn transaction_json(tx: &Transaction, include_code: bool) -> Value {
    json!({
        "nonce": tx.nonce,
        "sender": account_json(&tx.sender),
        "payload": payload_json(&tx.payload, include_code),
        "access_list": access_list_json(&tx.access_list),
    })
}

/// Build the JSON object for a block (without `hash` / `receipts` — caller adds those).
pub fn block_to_rpc_json(block: &Block, include_code: bool) -> Value {
    let header = &block.header;
    let mut header_obj = json!({
        "parent_hash": hash_json(&header.parent_hash),
        "height": header.height,
        "timestamp": header.timestamp,
        "proposer": account_json(&header.proposer),
        "tx_root": hash_json(&header.tx_root),
        "receipts_root": hash_json(&header.receipts_root),
        "state_root": hash_json(&header.state_root),
    });
    if let Some(br) = &header.blobs_root {
        header_obj
            .as_object_mut()
            .unwrap()
            .insert("blobs_root".into(), hash_json(br));
    }

    let mut obj = json!({
        "header": header_obj,
        "transactions": block
            .transactions
            .iter()
            .map(|tx| transaction_json(tx, include_code))
            .collect::<Vec<_>>(),
    });
    let map = obj.as_object_mut().unwrap();
    if !block.signatures.is_empty() {
        map.insert(
            "signatures".into(),
            Value::Array(block.signatures.iter().map(sig_json).collect()),
        );
    }
    if !block.priority_tips.is_empty() {
        map.insert(
            "priority_tips".into(),
            Value::Array(block.priority_tips.iter().copied().map(u128_json).collect()),
        );
    }
    obj
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, AccountId, BlockHeader, Hash, TransactionPayload};

    fn tiny_block(bytecode: Vec<u8>) -> Block {
        let tx = Transaction {
            nonce: 0,
            sender: AccountId([1u8; 32]),
            payload: TransactionPayload::ContractDeploy {
                bytecode,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        Block {
            header: BlockHeader {
                parent_hash: Hash([0u8; 32]),
                height: 1,
                timestamp: 1,
                proposer: AccountId([2u8; 32]),
                tx_root: Hash([0u8; 32]),
                receipts_root: Hash([0u8; 32]),
                state_root: Hash([0u8; 32]),
                blobs_root: None,
            },
            transactions: vec![tx],
            signatures: vec![],
            priority_tips: vec![],
        }
    }

    #[test]
    fn include_code_emits_hex_not_number_array() {
        let block = tiny_block(vec![0x60, 0x00]);
        let v = block_to_rpc_json(&block, true);
        let bc = &v["transactions"][0]["payload"]["ContractDeploy"]["bytecode"];
        assert_eq!(bc, &json!("0x6000"));
    }

    #[test]
    fn slim_mode_omits_bytecode_with_size_and_hash() {
        let bytes = vec![0xAAu8; 10_000];
        let block = tiny_block(bytes.clone());
        let v = block_to_rpc_json(&block, false);
        let bc = &v["transactions"][0]["payload"]["ContractDeploy"]["bytecode"];
        assert_eq!(bc["omitted"], json!(true));
        assert_eq!(bc["size"], json!(10_000));
        assert!(bc["hash"].as_str().unwrap().starts_with("0x"));
        let text = serde_json::to_string(&v).unwrap();
        assert!(text.len() < 2_000, "slim JSON must stay tiny, got {}", text.len());
        assert!(!text.contains("170,"), "must not emit decimal byte arrays");
    }
}
