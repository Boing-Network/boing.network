//! In-memory account -> transaction index for `boing_getAccountTransactions`.
//!
//! Built lazily from committed blocks + receipts and caught up incrementally on every query, so it
//! needs no persistence (it is rebuilt from the chain after a restart). Paging is deterministic:
//! newest first, ordered by `(block_height, tx_index)`, with an opaque `"<height>:<index>"` cursor
//! meaning "strictly older than this position".

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use boing_primitives::{AccountId, BatchCall, Hash, Transaction, TransactionPayload};

use crate::node::BoingNode;

pub const DEFAULT_LIMIT: usize = 25;
pub const MAX_LIMIT: usize = 100;

#[derive(Default)]
pub struct AccountTxIndex {
    /// Highest indexed block height (`None` = nothing indexed yet).
    indexed_to: Option<u64>,
    /// Hash of the block at `indexed_to` (detects a reset chain).
    indexed_hash: Option<Hash>,
    /// Ascending `(height, tx_index)` positions per account.
    by_account: HashMap<AccountId, Vec<(u64, u32)>>,
}

/// Accounts a transaction touches (sender, recipients, called contracts, batch steps, log topics).
pub fn touched_accounts(tx: &Transaction, log_topics: &[[u8; 32]]) -> Vec<AccountId> {
    let mut out = vec![tx.sender];
    match &tx.payload {
        TransactionPayload::Transfer { to, .. } => out.push(*to),
        TransactionPayload::ContractCall { contract, .. }
        | TransactionPayload::ContractCallWithValue { contract, .. } => out.push(*contract),
        TransactionPayload::Batch { calls } => {
            for c in calls {
                match c {
                    BatchCall::Transfer { to, .. } => out.push(*to),
                    BatchCall::ContractCall { contract, .. }
                    | BatchCall::ContractCallWithValue { contract, .. } => out.push(*contract),
                }
            }
        }
        TransactionPayload::ValidatorRegister { node_id, .. }
        | TransactionPayload::ValidatorDeregister { node_id }
        | TransactionPayload::ValidatorUnjail { node_id } => out.push(*node_id),
        TransactionPayload::NameRegister { target, .. }
        | TransactionPayload::NameSetTarget { target, .. } => out.push(*target),
        TransactionPayload::NameTransfer { new_owner, .. } => out.push(*new_owner),
        _ => {}
    }
    for t in log_topics {
        out.push(AccountId(*t));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup();
    out
}

impl AccountTxIndex {
    pub fn catch_up(&mut self, node: &BoingNode) {
        let head = node.chain.height();
        if let (Some(h), Some(hash)) = (self.indexed_to, self.indexed_hash) {
            let same =
                h <= head && node.chain.get_block_by_height(h).map(|b| b.hash()) == Some(hash);
            if !same {
                *self = AccountTxIndex::default();
            }
        }
        let start = self.indexed_to.map(|h| h + 1).unwrap_or(0);
        if start > head {
            return;
        }
        for h in start..=head {
            let Some(block) = node.chain.get_block_by_height(h) else {
                continue;
            };
            for (i, tx) in block.transactions.iter().enumerate() {
                let topics: Vec<[u8; 32]> = node
                    .receipts
                    .get(&tx.id())
                    .map(|r| {
                        r.logs
                            .iter()
                            .flat_map(|l| l.topics.iter().copied())
                            .collect()
                    })
                    .unwrap_or_default();
                for acct in touched_accounts(tx, &topics) {
                    self.by_account.entry(acct).or_default().push((h, i as u32));
                }
            }
            self.indexed_to = Some(h);
            self.indexed_hash = Some(block.hash());
        }
    }

    /// Positions for `account`, newest first, strictly older than `before`.
    pub fn page(
        &self,
        account: &AccountId,
        before: Option<(u64, u32)>,
        limit: usize,
    ) -> (Vec<(u64, u32)>, bool) {
        let Some(list) = self.by_account.get(account) else {
            return (Vec::new(), false);
        };
        let end = match before {
            Some(pos) => list.partition_point(|p| *p < pos),
            None => list.len(),
        };
        let start = end.saturating_sub(limit);
        let mut v: Vec<(u64, u32)> = list[start..end].to_vec();
        v.reverse();
        (v, start > 0)
    }
}

fn registry() -> &'static Mutex<HashMap<usize, AccountTxIndex>> {
    static R: OnceLock<Mutex<HashMap<usize, AccountTxIndex>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn parse_cursor(s: &str) -> Option<(u64, u32)> {
    let (a, b) = s.split_once(':')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn hex0x(b: &[u8]) -> String {
    format!("0x{}", hex::encode(b))
}

fn payload_json(p: &TransactionPayload) -> serde_json::Value {
    use serde_json::json;
    match p {
        TransactionPayload::Transfer { to, amount } => {
            json!({"kind":"transfer","to":hex0x(&to.0),"amount":amount.to_string()})
        }
        TransactionPayload::ContractCall { contract, calldata } => {
            json!({"kind":"contract_call","contract":hex0x(&contract.0),"calldata_len":calldata.len()})
        }
        TransactionPayload::ContractCallWithValue { contract, calldata, value } => {
            json!({"kind":"contract_call","contract":hex0x(&contract.0),"calldata_len":calldata.len(),"value":value.to_string(),"value_boing":boing_primitives::format_boing(*value)})
        }
        TransactionPayload::ContractDeploy { .. }
        | TransactionPayload::ContractDeployWithPurpose { .. }
        | TransactionPayload::ContractDeployWithPurposeAndMetadata { .. } => {
            json!({"kind":"contract_deploy"})
        }
        TransactionPayload::Bond { amount } => json!({"kind":"bond","amount":amount.to_string()}),
        TransactionPayload::Unbond { amount } => {
            json!({"kind":"unbond","amount":amount.to_string()})
        }
        TransactionPayload::ClaimUnbond => json!({"kind":"claim_unbond"}),
        TransactionPayload::QaPoolVote { .. } => json!({"kind":"qa_pool_vote"}),
        TransactionPayload::Batch { calls } => {
            let steps: Vec<serde_json::Value> = calls
                .iter()
                .map(|c| match c {
                    BatchCall::Transfer { to, amount } => json!({"kind":"transfer","to":hex0x(&to.0),"amount":amount.to_string()}),
                    BatchCall::ContractCall { contract, calldata } => json!({"kind":"contract_call","contract":hex0x(&contract.0),"calldata_len":calldata.len()}),
                    BatchCall::ContractCallWithValue { contract, calldata, value } => json!({"kind":"contract_call","contract":hex0x(&contract.0),"calldata_len":calldata.len(),"value":value.to_string(),"value_boing":boing_primitives::format_boing(*value)}),
                })
                .collect();
            json!({"kind":"batch","steps":steps})
        }
        TransactionPayload::ValidatorRegister { node_id, .. } => {
            json!({"kind":"validator_register","node_id":hex0x(&node_id.0)})
        }
        TransactionPayload::ValidatorDeregister { node_id } => {
            json!({"kind":"validator_deregister","node_id":hex0x(&node_id.0)})
        }
        TransactionPayload::ValidatorUnjail { node_id } => {
            json!({"kind":"validator_unjail","node_id":hex0x(&node_id.0)})
        }
        TransactionPayload::NameCommit { .. } => json!({"kind":"name_commit"}),
        TransactionPayload::NameRegister {
            name,
            target,
            years,
            ..
        } => json!({"kind":"name_register","name":name,"target":hex0x(&target.0),"years":years}),
        TransactionPayload::NameRenew { name, years } => {
            json!({"kind":"name_renew","name":name,"years":years})
        }
        TransactionPayload::NameTransfer { name, new_owner } => {
            json!({"kind":"name_transfer","name":name,"new_owner":hex0x(&new_owner.0)})
        }
        TransactionPayload::NameSetTarget { name, target } => {
            json!({"kind":"name_set_target","name":name,"target":hex0x(&target.0)})
        }
        TransactionPayload::NameSetPrimary { name } => {
            json!({"kind":"name_set_primary","name":name})
        }
        TransactionPayload::NameRelease { name } => json!({"kind":"name_release","name":name}),
        TransactionPayload::PutBlob {
            retention,
            content_type,
            data,
        } => {
            let id = boing_primitives::blob_id_of(data);
            let retention = match retention {
                boing_primitives::BlobRetention::Forever => json!({"kind":"forever"}),
                boing_primitives::BlobRetention::Prunable { ttl_blocks } => {
                    json!({"kind":"prunable","ttl_blocks":ttl_blocks})
                }
            };
            json!({
                "kind":"put_blob",
                "blob_id":hex0x(&id),
                "size":data.len(),
                "content_type":content_type,
                "retention":retention,
            })
        }
        TransactionPayload::BlobAnnounce {
            blob_id,
            size,
            retention,
            content_type,
        } => {
            let retention = match retention {
                boing_primitives::BlobRetention::Forever => json!({"kind":"forever"}),
                boing_primitives::BlobRetention::Prunable { ttl_blocks } => {
                    json!({"kind":"prunable","ttl_blocks":ttl_blocks})
                }
            };
            json!({
                "kind":"blob_announce",
                "blob_id":hex0x(blob_id),
                "size":size,
                "content_type":content_type,
                "retention":retention,
            })
        }
        TransactionPayload::RenewMediaBlobPin {
            blob_id,
            size,
            extend_blocks,
        } => json!({
            "kind":"renew_media_blob_pin",
            "blob_id":hex0x(blob_id),
            "size":size,
            "extend_blocks":extend_blocks,
        }),
        TransactionPayload::PayBlobSample {
            blob_id,
            chunk_index,
            chunk_count,
        } => json!({
            "kind":"pay_blob_sample",
            "blob_id":hex0x(blob_id),
            "chunk_index":chunk_index,
            "chunk_count":chunk_count,
        }),
        TransactionPayload::RegisterBlobSampleOffer {
            blob_id,
            chunk_index,
            chunk_count,
            ask_fee,
            expires_at_height,
        } => json!({
            "kind":"register_blob_sample_offer",
            "blob_id":hex0x(blob_id),
            "chunk_index":chunk_index,
            "chunk_count":chunk_count,
            "ask_fee":ask_fee.to_string(),
            "expires_at_height":expires_at_height,
        }),
        TransactionPayload::SettleBlobSampleMatch {
            blob_id,
            chunk_index,
            chunk_count,
            provider,
        } => json!({
            "kind":"settle_blob_sample_match",
            "blob_id":hex0x(blob_id),
            "chunk_index":chunk_index,
            "chunk_count":chunk_count,
            "provider":hex0x(&provider.0),
        }),
        TransactionPayload::CancelBlobSampleOffer {
            blob_id,
            chunk_index,
        } => json!({
            "kind":"cancel_blob_sample_offer",
            "blob_id":hex0x(blob_id),
            "chunk_index":chunk_index,
        }),
        TransactionPayload::ChallengeBlobSampleServe {
            blob_id,
            chunk_index,
            chunk_count,
            provider,
        } => json!({
            "kind":"challenge_blob_sample_serve",
            "blob_id":hex0x(blob_id),
            "chunk_index":chunk_index,
            "chunk_count":chunk_count,
            "provider":hex0x(&provider.0),
        }),
        TransactionPayload::RespondBlobSampleServe {
            blob_id,
            chunk_index,
            chunk_count,
            challenger,
            ..
        } => json!({
            "kind":"respond_blob_sample_serve",
            "blob_id":hex0x(blob_id),
            "chunk_index":chunk_index,
            "chunk_count":chunk_count,
            "challenger":hex0x(&challenger.0),
        }),
        TransactionPayload::SubmitSlashAppeal { slash_id, evidence } => json!({
            "kind":"submit_slash_appeal",
            "slash_id":slash_id,
            "evidence_len":evidence.len(),
        }),
        TransactionPayload::ResolveSlashAppeal { appeal_id, approved } => json!({
            "kind":"resolve_slash_appeal",
            "appeal_id":appeal_id,
            "approved":approved,
        }),
        TransactionPayload::ResolveSlashAppealAttested {
            appeal_id,
            approved,
            ..
        } => json!({
            "kind":"resolve_slash_appeal_attested",
            "appeal_id":appeal_id,
            "approved":approved,
        }),
        TransactionPayload::ResolveSlashAppealBlsAggregate {
            appeal_id,
            approved,
            ..
        } => json!({
            "kind":"resolve_slash_appeal_bls_aggregate",
            "appeal_id":appeal_id,
            "approved":approved,
        }),
    }
}

/// Build the `boing_getAccountTransactions` result.
pub fn account_transactions_json(
    node: &BoingNode,
    node_key: usize,
    account: &AccountId,
    before: Option<(u64, u32)>,
    limit: usize,
) -> serde_json::Value {
    let limit = limit.clamp(1, MAX_LIMIT);
    let (positions, has_more) = {
        let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
        let idx = reg.entry(node_key).or_default();
        idx.catch_up(node);
        idx.page(account, before, limit)
    };
    let mut txs = Vec::with_capacity(positions.len());
    let mut block_cache: Option<(u64, boing_primitives::Block)> = None;
    for (h, i) in &positions {
        if block_cache.as_ref().map(|(bh, _)| *bh) != Some(*h) {
            block_cache = node.chain.get_block_by_height(*h).map(|b| (*h, b));
        }
        let Some((_, block)) = block_cache.as_ref() else {
            continue;
        };
        let Some(tx) = block.transactions.get(*i as usize) else {
            continue;
        };
        let tid = tx.id();
        let r = node.receipts.get(&tid);
        let roles: Vec<&str> = {
            let mut v = Vec::new();
            if tx.sender == *account {
                v.push("sender");
            } else {
                v.push("counterparty");
            }
            v
        };
        txs.push(serde_json::json!({
            "tx_id": hex0x(&tid.0),
            "block_height": h,
            "tx_index": i,
            "timestamp": block.header.timestamp,
            "sender": hex0x(&tx.sender.0),
            "nonce": tx.nonce,
            "direction": if tx.sender == *account { "out" } else { "in" },
            "roles": roles,
            "payload": payload_json(&tx.payload),
            "success": r.map(|r| r.success),
            "error": r.and_then(|r| r.error.clone()),
            "gas_used": r.map(|r| r.gas_used),
        }));
    }
    let next_cursor = if has_more {
        positions.last().map(|(h, i)| format!("{h}:{i}"))
    } else {
        None
    };
    serde_json::json!({
        "account": hex0x(&account.0),
        "head_height": node.chain.height(),
        "transactions": txs,
        "next_cursor": next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_is_newest_first_with_strict_cursor() {
        let mut idx = AccountTxIndex::default();
        let a = AccountId([7u8; 32]);
        idx.by_account
            .insert(a, vec![(1, 0), (2, 0), (2, 1), (5, 3)]);
        let (p, more) = idx.page(&a, None, 2);
        assert_eq!(p, vec![(5, 3), (2, 1)]);
        assert!(more);
        let (p, more) = idx.page(&a, Some((2, 1)), 2);
        assert_eq!(p, vec![(2, 0), (1, 0)]);
        assert!(!more);
        let (p, _) = idx.page(&AccountId([1u8; 32]), None, 5);
        assert!(p.is_empty());
    }

    #[test]
    fn cursor_parse() {
        assert_eq!(parse_cursor("12:3"), Some((12, 3)));
        assert_eq!(parse_cursor("x"), None);
    }
}
