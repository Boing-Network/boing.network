//! Core data structures for Boing blockchain.
//!
//! Account model with explicit access lists for parallel execution.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::hash::{hasher, Hash};

/// Account identifier (32 bytes, typically derived from pubkey).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct AccountId(pub [u8; 32]);

impl AccountId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        AccountId(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Some(AccountId(arr))
    }
}

impl std::fmt::Debug for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AccountId({})", hex::encode(&self.0[..8]))
    }
}

/// Nonce-based contract address (legacy Boing deploy): `BLAKE3(sender || deploy_tx_nonce_le)`.
pub fn nonce_derived_contract_address(sender: &AccountId, deploy_tx_nonce: u64) -> AccountId {
    let mut h = hasher();
    h.update(&sender.0);
    h.update(&deploy_tx_nonce.to_le_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    AccountId(out)
}

const CREATE2_DOMAIN: &[u8] = b"boing.create2.v1\0";

/// Salt-derived contract address: `BLAKE3(domain || deployer || salt || BLAKE3(bytecode))`.
/// Shares the same 32-byte [`AccountId`] space as Ed25519-derived accounts; accidental collision is negligible.
pub fn create2_contract_address(
    deployer: &AccountId,
    salt: &[u8; 32],
    bytecode: &[u8],
) -> AccountId {
    let mut ch = hasher();
    ch.update(bytecode);
    let code_hash = ch.finalize();
    let mut h = hasher();
    h.update(CREATE2_DOMAIN);
    h.update(&deployer.0);
    h.update(salt);
    h.update(code_hash.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    AccountId(out)
}

/// Access list — accounts this transaction reads/writes.
/// Enables parallel scheduling (transactions with disjoint access lists run in parallel).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessList {
    pub read: Vec<AccountId>,
    pub write: Vec<AccountId>,
}

fn suggested_access_list_for_contract_deploy(
    sender: AccountId,
    bytecode: &[u8],
    create2_salt: Option<[u8; 32]>,
) -> AccessList {
    let mut read = vec![sender];
    let mut write = vec![sender];
    if let Some(salt) = create2_salt {
        let addr = create2_contract_address(&sender, &salt, bytecode);
        read.push(addr);
        write.push(addr);
    }
    AccessList::new(read, write)
}

impl AccessList {
    pub fn new(read: Vec<AccountId>, write: Vec<AccountId>) -> Self {
        Self { read, write }
    }

    /// All accounts touched (read or write).
    pub fn all(&self) -> impl Iterator<Item = &AccountId> {
        self.read.iter().chain(self.write.iter())
    }

    /// True if this access list overlaps with another (shared account).
    pub fn conflicts_with(&self, other: &AccessList) -> bool {
        let a: std::collections::HashSet<_> = self.all().collect();
        let b: std::collections::HashSet<_> = other.all().collect();
        !a.is_disjoint(&b)
    }
}

/// Transaction — the unit of execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub nonce: u64,
    pub sender: AccountId,
    pub payload: TransactionPayload,
    pub access_list: AccessList,
}

impl Transaction {
    /// Human-readable summary for signing display in wallets.
    pub fn display_for_signing(&self) -> String {
        let mut payload_str = match &self.payload {
            TransactionPayload::Transfer { to, amount } => {
                format!("Transfer {} to {}", amount, hex::encode(&to.0[..8]))
            }
            TransactionPayload::Bond { amount } => format!("Bond {} stake", amount),
            TransactionPayload::Unbond { amount } => format!("Unbond {} stake", amount),
            TransactionPayload::ClaimUnbond => "Claim unbonded stake".into(),
            TransactionPayload::ValidatorRegister { node_id, .. } => {
                format!("Register validator node {}", hex::encode(&node_id.0[..8]))
            }
            TransactionPayload::ValidatorDeregister { node_id } => {
                format!("Deregister validator node {}", hex::encode(&node_id.0[..8]))
            }
            TransactionPayload::ValidatorUnjail { node_id } => {
                format!("Unjail validator node {}", hex::encode(&node_id.0[..8]))
            }
            TransactionPayload::NameCommit { .. } => "Commit to a name registration".into(),
            TransactionPayload::NameRegister { name, years, .. } => {
                format!("Register name {}.boing for {} year(s)", name, years)
            }
            TransactionPayload::NameRenew { name, years } => {
                format!("Renew name {}.boing for {} year(s)", name, years)
            }
            TransactionPayload::NameTransfer { name, new_owner } => {
                format!(
                    "Transfer name {}.boing to {}",
                    name,
                    hex::encode(&new_owner.0[..8])
                )
            }
            TransactionPayload::NameSetTarget { name, target } => {
                format!(
                    "Point name {}.boing at {}",
                    name,
                    hex::encode(&target.0[..8])
                )
            }
            TransactionPayload::NameSetPrimary { name: Some(n) } => {
                format!("Set primary name {}.boing", n)
            }
            TransactionPayload::NameSetPrimary { name: None } => "Clear primary name".into(),
            TransactionPayload::NameRelease { name } => format!("Release name {}.boing", name),
            TransactionPayload::ContractCall { contract, .. } => {
                format!("Call contract {}", hex::encode(&contract.0[..8]))
            }
            TransactionPayload::ContractCallWithValue { contract, value, .. } => {
                format!(
                    "Call contract {} with {} BOING",
                    hex::encode(&contract.0[..8]),
                    crate::format_boing(*value)
                )
            }
            TransactionPayload::PutBlob {
                retention,
                content_type,
                data,
            } => {
                let id = crate::blob_id_of(data);
                let ret = match retention {
                    crate::BlobRetention::Forever => "forever".into(),
                    crate::BlobRetention::Prunable { ttl_blocks } => {
                        format!("prunable/{ttl_blocks}")
                    }
                };
                let ct = if content_type.is_empty() {
                    "unknown".into()
                } else {
                    content_type.clone()
                };
                format!(
                    "Put media blob {} ({} bytes, {}, {})",
                    hex::encode(&id[..8]),
                    data.len(),
                    ret,
                    ct
                )
            }
            TransactionPayload::BlobAnnounce {
                blob_id,
                size,
                retention,
                content_type,
            } => {
                let ret = match retention {
                    crate::BlobRetention::Forever => "forever".into(),
                    crate::BlobRetention::Prunable { ttl_blocks } => {
                        format!("prunable/{ttl_blocks}")
                    }
                };
                let ct = if content_type.is_empty() {
                    "unknown".into()
                } else {
                    content_type.clone()
                };
                format!(
                    "Announce media blob {} ({} bytes, {}, {})",
                    hex::encode(&blob_id[..8]),
                    size,
                    ret,
                    ct
                )
            }
            TransactionPayload::RenewMediaBlobPin {
                blob_id,
                size,
                extend_blocks,
            } => format!(
                "Renew media blob pin {} ({} bytes, +{} blocks)",
                hex::encode(&blob_id[..8]),
                size,
                extend_blocks
            ),
            TransactionPayload::PayBlobSample {
                blob_id,
                chunk_index,
                chunk_count,
            } => format!(
                "Pay media blob sample {} (chunk {}/{})",
                hex::encode(&blob_id[..8]),
                chunk_index,
                chunk_count
            ),
            TransactionPayload::RegisterBlobSampleOffer {
                blob_id,
                chunk_index,
                chunk_count,
                ask_fee,
                ..
            } => format!(
                "Register media blob sample offer {} (chunk {}/{}, ask {})",
                hex::encode(&blob_id[..8]),
                chunk_index,
                chunk_count,
                ask_fee
            ),
            TransactionPayload::SettleBlobSampleMatch {
                blob_id,
                chunk_index,
                chunk_count,
                provider,
            } => format!(
                "Settle media blob sample {} (chunk {}/{}, provider {})",
                hex::encode(&blob_id[..8]),
                chunk_index,
                chunk_count,
                hex::encode(&provider.0[..8])
            ),
            TransactionPayload::CancelBlobSampleOffer {
                blob_id,
                chunk_index,
            } => format!(
                "Cancel media blob sample offer {} (chunk {})",
                hex::encode(&blob_id[..8]),
                chunk_index
            ),
            TransactionPayload::ChallengeBlobSampleServe {
                blob_id,
                chunk_index,
                chunk_count,
                provider,
            } => format!(
                "Challenge media blob sample serve {} (chunk {}/{}, provider {})",
                hex::encode(&blob_id[..8]),
                chunk_index,
                chunk_count,
                hex::encode(&provider.0[..8])
            ),
            TransactionPayload::RespondBlobSampleServe {
                blob_id,
                chunk_index,
                chunk_count,
                challenger,
                ..
            } => format!(
                "Respond media blob sample serve {} (chunk {}/{}, challenger {})",
                hex::encode(&blob_id[..8]),
                chunk_index,
                chunk_count,
                hex::encode(&challenger.0[..8])
            ),
            TransactionPayload::SubmitSlashAppeal { slash_id, evidence } => format!(
                "Submit slash appeal for slash_id {} ({} evidence bytes)",
                slash_id,
                evidence.len()
            ),
            TransactionPayload::ResolveSlashAppeal {
                appeal_id,
                approved,
            } => format!(
                "Resolve slash appeal {} ({})",
                appeal_id,
                if *approved { "approve" } else { "reject" }
            ),
            TransactionPayload::ContractDeploy { .. }
            | TransactionPayload::ContractDeployWithPurpose { .. }
            | TransactionPayload::ContractDeployWithPurposeAndMetadata { .. } => {
                "Deploy contract".into()
            }
            TransactionPayload::QaPoolVote { subject, vote } => {
                format!(
                    "QA pool vote {:?} on {}",
                    vote,
                    hex::encode(&subject.0[..8])
                )
            }
            TransactionPayload::Batch { calls } => {
                let steps: Vec<String> = calls
                    .iter()
                    .map(|c| match c {
                        BatchCall::Transfer { to, amount } => {
                            format!("Transfer {} to {}", amount, hex::encode(&to.0[..8]))
                        }
                        BatchCall::ContractCall { contract, .. } => {
                            format!("Call contract {}", hex::encode(&contract.0[..8]))
                        }
                        BatchCall::ContractCallWithValue { contract, value, .. } => format!(
                            "Call contract {} with {} BOING",
                            hex::encode(&contract.0[..8]),
                            crate::format_boing(*value)
                        ),
                    })
                    .collect();
                format!("Batch of {} (atomic): {}", calls.len(), steps.join("; "))
            }
        };
        if self.payload.deploy_create2_salt().is_some() {
            payload_str = format!("{payload_str} (salt-derived)");
        }
        format!(
            "From: {} | Nonce: {} | {}",
            hex::encode(&self.sender.0[..8]),
            self.nonce,
            payload_str
        )
    }

    pub fn id(&self) -> Hash {
        let mut h = hasher();
        h.update(&bincode::serialize(self).unwrap_or_default());
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
    }

    /// Heuristic minimum [`AccessList`] for parallel scheduling (see `TECHNICAL-SPECIFICATION.md` §4.2).
    /// Used by RPC simulation hints — does not reflect every storage key a contract may touch.
    pub fn suggested_parallel_access_list(&self) -> AccessList {
        match &self.payload {
            TransactionPayload::Transfer { to, .. } => {
                AccessList::new(vec![self.sender, *to], vec![self.sender, *to])
            }
            TransactionPayload::ContractCall { contract, .. }
            | TransactionPayload::ContractCallWithValue { contract, .. } => {
                AccessList::new(vec![self.sender, *contract], vec![self.sender, *contract])
            }
            TransactionPayload::ContractDeploy {
                bytecode,
                create2_salt,
            } => suggested_access_list_for_contract_deploy(self.sender, bytecode, *create2_salt),
            TransactionPayload::ContractDeployWithPurpose {
                bytecode,
                create2_salt,
                ..
            } => suggested_access_list_for_contract_deploy(self.sender, bytecode, *create2_salt),
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                create2_salt,
                ..
            } => suggested_access_list_for_contract_deploy(self.sender, bytecode, *create2_salt),
            TransactionPayload::Bond { .. }
            | TransactionPayload::Unbond { .. }
            | TransactionPayload::ClaimUnbond
            | TransactionPayload::ValidatorRegister { .. }
            | TransactionPayload::ValidatorDeregister { .. }
            | TransactionPayload::ValidatorUnjail { .. }
            | TransactionPayload::NameCommit { .. }
            | TransactionPayload::NameRegister { .. }
            | TransactionPayload::NameRenew { .. }
            | TransactionPayload::NameTransfer { .. }
            | TransactionPayload::NameSetTarget { .. }
            | TransactionPayload::NameSetPrimary { .. }
            | TransactionPayload::NameRelease { .. }
            | TransactionPayload::PutBlob { .. }
            | TransactionPayload::BlobAnnounce { .. }
            | TransactionPayload::RenewMediaBlobPin { .. }
            | TransactionPayload::PayBlobSample { .. }
            | TransactionPayload::RegisterBlobSampleOffer { .. }
            | TransactionPayload::SettleBlobSampleMatch { .. }
            | TransactionPayload::CancelBlobSampleOffer { .. }
            | TransactionPayload::ChallengeBlobSampleServe { .. }
            | TransactionPayload::RespondBlobSampleServe { .. }
            | TransactionPayload::SubmitSlashAppeal { .. }
            | TransactionPayload::ResolveSlashAppeal { .. }
            | TransactionPayload::QaPoolVote { .. } => {
                AccessList::new(vec![self.sender], vec![self.sender])
            }
            TransactionPayload::Batch { calls } => {
                let mut ids = vec![self.sender];
                for c in calls {
                    let id = match c {
                        BatchCall::Transfer { to, .. } => *to,
                        BatchCall::ContractCall { contract, .. }
                        | BatchCall::ContractCallWithValue { contract, .. } => *contract,
                    };
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
                AccessList::new(ids.clone(), ids)
            }
        }
    }

    /// `true` if every account in [`suggested_parallel_access_list`] appears in `access_list` (read or write).
    pub fn access_list_covers_parallel_suggestion(&self) -> bool {
        let sug = self.suggested_parallel_access_list();
        let have: HashSet<AccountId> = self.access_list.all().copied().collect();
        let need: HashSet<AccountId> = sug.all().copied().collect();
        need.is_subset(&have)
    }
}

/// Transaction payload (extensible).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionPayload {
    Transfer {
        to: AccountId,
        amount: u128,
    },
    ContractCall {
        contract: AccountId,
        calldata: Vec<u8>,
    },
    /// Contract deploy (legacy; no purpose declaration).
    ContractDeploy {
        bytecode: Vec<u8>,
        /// When `Some`, contract address is [`create2_contract_address`] instead of [`nonce_derived_contract_address`].
        #[serde(default)]
        create2_salt: Option<[u8; 32]>,
    },
    /// Contract deploy with optional QA purpose declaration (no deploy-time metadata).
    ContractDeployWithPurpose {
        bytecode: Vec<u8>,
        purpose_category: String,
        description_hash: Option<Vec<u8>>,
        #[serde(default)]
        create2_salt: Option<[u8; 32]>,
    },
    /// Contract deploy with purpose and optional asset metadata (name/symbol) for content-policy checks.
    ContractDeployWithPurposeAndMetadata {
        bytecode: Vec<u8>,
        purpose_category: String,
        description_hash: Option<Vec<u8>>,
        /// Optional asset name (e.g. token name). Checked against governance content blocklist. Max 256 bytes UTF-8.
        asset_name: Option<String>,
        /// Optional asset symbol (e.g. ticker). Checked against governance content blocklist. Max 32 bytes UTF-8.
        asset_symbol: Option<String>,
        #[serde(default)]
        create2_salt: Option<[u8; 32]>,
    },
    /// Bond stake to become/remain a validator.
    Bond {
        amount: u128,
    },
    /// Begin unbonding: move `amount` from `stake` into `pending_unbond` (unlock after delay).
    Unbond {
        amount: u128,
    },
    /// Claim matured `pending_unbond` into `balance` once `unbond_unlock_height` is reached.
    ClaimUnbond,
    /// Signed QA pool vote on a pending Unsure deploy (`subject` = that deploy's `tx.id()`).
    /// Appended last so existing bincode discriminants stay stable.
    QaPoolVote {
        subject: Hash,
        vote: QaPoolVoteKind,
    },
    /// Atomic batch: up to [`MAX_BATCH_CALLS`] native transfers and/or contract calls applied in
    /// order under **one** signature and **one** nonce. If any step fails, the whole batch is
    /// reverted (the nonce is still consumed, like any failed tx). Appended last so existing
    /// bincode discriminants stay stable (variant index 9).
    Batch {
        calls: Vec<BatchCall>,
    },
    /// Open validator registration (variant index 10). The tx signer (`sender`) is the **owner** and
    /// pays the configured bond; `node_signature` is the node key's signature over
    /// [`crate::validator_registration_message`]`(sender, node_id)`. Rejected unless the network has
    /// activated the `open` validator-set mode.
    ValidatorRegister {
        node_id: AccountId,
        node_signature: Vec<u8>,
    },
    /// Leave the open validator set (index 11). Owner only. The bond moves to `pending_unbond` and is
    /// claimable with [`TransactionPayload::ClaimUnbond`] after the deregistration delay.
    ValidatorDeregister {
        node_id: AccountId,
    },
    /// Request re-admission after a liveness jail once the cooldown has passed (index 12). Owner only.
    ValidatorUnjail {
        node_id: AccountId,
    },
    /// Name registry (index 13): commit to `name_commitment(label, sender, salt)` before registering,
    /// so the label is not visible in the mempool. See `crate::names`.
    NameCommit {
        commitment: [u8; 32],
    },
    /// Register `name` (index 14) for `years`, revealing the salt of an earlier matured `NameCommit` by
    /// the same sender. The sender becomes the owner; the name resolves to `target`. Rent is burned.
    NameRegister {
        name: String,
        salt: [u8; 32],
        target: AccountId,
        years: u8,
    },
    /// Extend a name (index 15) by `years` (owner only; allowed during the grace period). Rent is burned.
    NameRenew {
        name: String,
        years: u8,
    },
    /// Give a name to `new_owner` (index 16). Owner only; the target is kept.
    NameTransfer {
        name: String,
        new_owner: AccountId,
    },
    /// Point a name at another address (index 17). Owner only.
    NameSetTarget {
        name: String,
        target: AccountId,
    },
    /// Set (or clear with `None`) the sender's primary / reverse name (index 18). The name must be
    /// active and resolve to the sender.
    NameSetPrimary {
        name: Option<String>,
    },
    /// Release a name early (index 19). Owner only; no refund.
    NameRelease {
        name: String,
    },
    /// Contract call carrying native BOING (index 20; native-value rules, testnet height 1150).
    /// `value` (base units) is debited from the sender and credited to `contract` atomically before
    /// the contract runs; the code reads it with `CALLVALUE`. If the call (or its fee) fails, the whole
    /// tx reverts, so the value is refunded and only the failed-tx fee is charged. Signed like every
    /// other payload (v2 signatures bind the chain id), so `value` is covered by the signature.
    ContractCallWithValue {
        contract: AccountId,
        calldata: Vec<u8>,
        value: u128,
    },
    /// Publish a content-addressed media blob (index **21**). Payload carries bytes (cap
    /// [`crate::MAX_MEDIA_BLOB_BYTES`], Phase 2 = 1 MiB); id is `BLAKE3(data)`. Bytes are stored in a
    /// node sidecar; consensus keeps metadata only. Separate fee schedule via size-scaled blob gas —
    /// not SSTORE. See `docs/BOING-L1-MEDIA-BLOBS.md`.
    PutBlob {
        retention: crate::BlobRetention,
        /// Optional MIME hint (max [`crate::MAX_MEDIA_BLOB_CONTENT_TYPE_LEN`] UTF-8 bytes).
        content_type: String,
        data: Vec<u8>,
    },
    /// Announce a media blob without embedding bytes (index **22**). Pays size-scaled blob fees;
    /// payload is expected via P2P (`boing/media-blobs`) or prior sidecar put. Phase 2 data plane.
    BlobAnnounce {
        blob_id: crate::BlobId,
        size: u32,
        retention: crate::BlobRetention,
        content_type: String,
    },
    /// Extend a media blob pin / retention horizon (index **23**). Phase 5 Forever renewals +
    /// economics path; `size` is the fee meter (must match sidecar). See `BOING-L1-MEDIA-BLOBS.md`.
    RenewMediaBlobPin {
        blob_id: crate::BlobId,
        size: u32,
        extend_blocks: u64,
    },
    /// Pay for one DAS chunk sample (index **24**). Phase 5 paid-sampling market scaffold —
    /// charges `blob_sample_fee_per_chunk`; does not yet settle a full market.
    PayBlobSample {
        blob_id: crate::BlobId,
        chunk_index: u32,
        chunk_count: u32,
    },
    /// Register an on-chain DAS sample offer (index **25**). Provider = tx sender. Phase 7.
    RegisterBlobSampleOffer {
        blob_id: crate::BlobId,
        chunk_index: u32,
        chunk_count: u32,
        ask_fee: u128,
        expires_at_height: u64,
    },
    /// Settle a sample match against a registered provider (index **26**). Payer = tx sender;
    /// charges `blob_sample_fee_per_chunk` like [`PayBlobSample`]. Phase 7 on-chain matcher path.
    /// Phase 9: also releases SMT-escrowed `ask_fee` to `provider`.
    SettleBlobSampleMatch {
        blob_id: crate::BlobId,
        chunk_index: u32,
        chunk_count: u32,
        provider: AccountId,
    },
    /// Cancel a registered sample offer and refund escrowed ask (index **27**). Provider = tx sender.
    CancelBlobSampleOffer {
        blob_id: crate::BlobId,
        chunk_index: u32,
    },
    /// Challenge that a provider failed to serve a settled sample (index **28**). Challenger = sender.
    ChallengeBlobSampleServe {
        blob_id: crate::BlobId,
        chunk_index: u32,
        chunk_count: u32,
        provider: AccountId,
    },
    /// Provider responds to a serve challenge with a DAS merkle sample (index **29**).
    RespondBlobSampleServe {
        blob_id: crate::BlobId,
        chunk_index: u32,
        chunk_count: u32,
        challenger: AccountId,
        chunk_hash: [u8; 32],
        chunks_root: [u8; 32],
        siblings: Vec<[u8; 32]>,
    },
    /// Submit a consensus slash appeal (index **30**). Phase 14 — evidence opaque, size-capped.
    SubmitSlashAppeal {
        slash_id: u64,
        evidence: Vec<u8>,
    },
    /// Resolve a pending consensus slash appeal (index **31**). Sender must be a consensus validator.
    /// Approve restores FailedServe **balance** (or stake for other reasons); not a SNARK verifier.
    ResolveSlashAppeal {
        appeal_id: u64,
        approved: bool,
    },
}

/// One step inside [`TransactionPayload::Batch`]. Contract calls run with `CALLER` = tx sender,
/// so token / NFT `transfer` calldata works exactly as in a standalone `ContractCall`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchCall {
    Transfer {
        to: AccountId,
        amount: u128,
    },
    ContractCall {
        contract: AccountId,
        calldata: Vec<u8>,
    },
    /// Contract call step carrying native BOING (index 2; native-value rules). `value` moves from the
    /// tx sender to `contract` right before the step runs (`CALLVALUE` = `value`).
    ContractCallWithValue {
        contract: AccountId,
        calldata: Vec<u8>,
        value: u128,
    },
}

/// Maximum steps in one [`TransactionPayload::Batch`].
pub const MAX_BATCH_CALLS: usize = 16;
/// Maximum summed calldata bytes across all contract-call steps of one batch.
pub const MAX_BATCH_CALLDATA_BYTES: usize = 64 * 1024;

/// Structural problems with a [`TransactionPayload::Batch`] (checked at admission and execution).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BatchValidationError {
    #[error("batch has no calls")]
    Empty,
    #[error("batch has {got} calls (max {max})")]
    TooManyCalls { got: usize, max: usize },
    #[error("batch calldata is {got} bytes (max {max})")]
    CalldataTooLarge { got: usize, max: usize },
    #[error("batch transfer total overflows u128")]
    AmountOverflow,
}

/// Validate batch shape: non-empty, at most [`MAX_BATCH_CALLS`], bounded calldata, no u128 overflow.
pub fn validate_batch_calls(calls: &[BatchCall]) -> Result<(), BatchValidationError> {
    if calls.is_empty() {
        return Err(BatchValidationError::Empty);
    }
    if calls.len() > MAX_BATCH_CALLS {
        return Err(BatchValidationError::TooManyCalls {
            got: calls.len(),
            max: MAX_BATCH_CALLS,
        });
    }
    let mut calldata = 0usize;
    let mut total: u128 = 0;
    for c in calls {
        match c {
            BatchCall::Transfer { amount, .. } => {
                total = total
                    .checked_add(*amount)
                    .ok_or(BatchValidationError::AmountOverflow)?;
            }
            BatchCall::ContractCall { calldata: d, .. } => {
                calldata = calldata.saturating_add(d.len());
            }
            BatchCall::ContractCallWithValue {
                calldata: d, value, ..
            } => {
                calldata = calldata.saturating_add(d.len());
                total = total
                    .checked_add(*value)
                    .ok_or(BatchValidationError::AmountOverflow)?;
            }
        }
    }
    if calldata > MAX_BATCH_CALLDATA_BYTES {
        return Err(BatchValidationError::CalldataTooLarge {
            got: calldata,
            max: MAX_BATCH_CALLDATA_BYTES,
        });
    }
    Ok(())
}

/// Allow / Reject / Abstain for [`TransactionPayload::QaPoolVote`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QaPoolVoteKind {
    Allow,
    Reject,
    Abstain,
}

/// Borrowed fields exposed by [`TransactionPayload::as_contract_deploy`].
pub type ContractDeployFields<'a> = (
    &'a [u8],
    Option<&'a str>,
    Option<&'a [u8]>,
    Option<&'a str>,
    Option<&'a str>,
);

impl TransactionPayload {
    /// Salt for salt-derived deploy, if this payload is a contract deploy variant.
    pub fn deploy_create2_salt(&self) -> Option<[u8; 32]> {
        match self {
            TransactionPayload::ContractDeploy { create2_salt, .. } => *create2_salt,
            TransactionPayload::ContractDeployWithPurpose { create2_salt, .. } => *create2_salt,
            TransactionPayload::ContractDeployWithPurposeAndMetadata { create2_salt, .. } => {
                *create2_salt
            }
            _ => None,
        }
    }

    /// Structural check for [`TransactionPayload::Batch`]; `Ok(())` for every other variant.
    pub fn validate_batch(&self) -> Result<(), BatchValidationError> {
        match self {
            TransactionPayload::Batch { calls } => validate_batch_calls(calls),
            _ => Ok(()),
        }
    }

    /// Native BOING moved by this payload (Transfer amount, or sum of batch transfer steps).
    /// Used by the optional transfer-amount fee so batching cannot dodge it.
    pub fn native_transfer_total(&self) -> u128 {
        match self {
            TransactionPayload::Transfer { amount, .. } => *amount,
            TransactionPayload::Batch { calls } => calls
                .iter()
                .map(|c| match c {
                    BatchCall::Transfer { amount, .. } => *amount,
                    BatchCall::ContractCall { .. } | BatchCall::ContractCallWithValue { .. } => 0,
                })
                .fold(0u128, |a, b| a.saturating_add(b)),
            _ => 0,
        }
    }

    /// Max length for asset_name in ContractDeployWithPurpose (UTF-8 bytes).
    pub const MAX_ASSET_NAME_LEN: usize = 256;
    /// Max length for asset_symbol in ContractDeployWithPurpose (UTF-8 bytes).
    pub const MAX_ASSET_SYMBOL_LEN: usize = 32;

    /// Returns (bytecode, purpose_category, description_hash, asset_name, asset_symbol) if this is a contract deploy payload.
    pub fn as_contract_deploy(&self) -> Option<ContractDeployFields<'_>> {
        match self {
            TransactionPayload::ContractDeploy { bytecode, .. } => {
                Some((bytecode.as_slice(), None, None, None, None))
            }
            TransactionPayload::ContractDeployWithPurpose {
                bytecode,
                purpose_category,
                description_hash,
                ..
            } => Some((
                bytecode.as_slice(),
                Some(purpose_category.as_str()),
                description_hash.as_deref(),
                None,
                None,
            )),
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                purpose_category,
                description_hash,
                asset_name,
                asset_symbol,
                ..
            } => Some((
                bytecode.as_slice(),
                Some(purpose_category.as_str()),
                description_hash.as_deref(),
                asset_name.as_deref(),
                asset_symbol.as_deref(),
            )),
            _ => None,
        }
    }
}

/// Account state — balance, nonce, and staked amount.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    pub balance: u128,
    pub nonce: u64,
    /// Staked balance (bonded for validation / active in the stake set).
    pub stake: u128,
    /// Stake waiting for the unbonding delay before returning to `balance`.
    #[serde(default)]
    pub pending_unbond: u128,
    /// Block height at which `pending_unbond` may be claimed (0 = none).
    #[serde(default)]
    pub unbond_unlock_height: u64,
}

/// Full account (id + state).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub state: AccountState,
}

/// Compute Merkle root of transaction IDs (simple binary tree).
pub fn tx_root(txs: &[Transaction]) -> Hash {
    if txs.is_empty() {
        return Hash::ZERO;
    }
    let leaves: Vec<Hash> = txs.iter().map(|tx| tx.id()).collect();
    merkle_root_impl(&leaves)
}

/// BLAKE3 hash of `bincode(receipt)` — one leaf in [`receipts_root`].
pub fn receipt_leaf_hash(receipt: &ExecutionReceipt) -> Hash {
    let mut h = crate::hash::hasher();
    h.update(&bincode::serialize(receipt).unwrap_or_default());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    Hash(out)
}

/// Merkle root of execution receipts (same tree shape as [`tx_root`]); empty → [`Hash::ZERO`].
pub fn receipts_root(receipts: &[ExecutionReceipt]) -> Hash {
    if receipts.is_empty() {
        return Hash::ZERO;
    }
    let leaves: Vec<Hash> = receipts.iter().map(receipt_leaf_hash).collect();
    merkle_root_impl(&leaves)
}

fn merkle_root_impl(hashes: &[Hash]) -> Hash {
    if hashes.len() == 1 {
        return hashes[0];
    }
    let mut next = Vec::new();
    for chunk in hashes.chunks(2) {
        let h = if chunk.len() == 2 {
            hash_pair(&chunk[0], &chunk[1])
        } else {
            hash_pair(&chunk[0], &chunk[0])
        };
        next.push(h);
    }
    merkle_root_impl(&next)
}

fn hash_pair(a: &Hash, b: &Hash) -> Hash {
    let mut h = crate::hash::hasher();
    h.update(a.as_bytes());
    h.update(b.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    Hash(out)
}

/// Block header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub parent_hash: Hash,
    pub height: u64,
    pub timestamp: u64,
    pub proposer: AccountId,
    pub tx_root: Hash,
    /// Merkle root over receipt leaves ([`receipt_leaf_hash`]) in transaction order.
    pub receipts_root: Hash,
    pub state_root: Hash,
    /// Media commitment ([`crate::blobs_root_of_transactions`]).
    ///
    /// **Phase 3+:** when `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT` is active, producers set
    /// `Some(root)` (incl. [`Hash::ZERO`] when the block has no media txs) and importers verify.
    /// Before activation the field is omitted (`None`) so historical header encodings and
    /// [`Block::hash`] stay unchanged. Decode historical bytes via [`Block::from_bincode`]
    /// (exact-length layout attempts — do not rely on bincode `Option` defaults mid-header).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blobs_root: Option<Hash>,
}

/// Block — header + transactions (+ the transactions' signatures from the block-signature activation).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    /// Sender signature of each transaction, same order as `transactions` (security review #16).
    /// Required (one per tx, verified on import) from the network's block-signature activation
    /// height; empty before it. Not part of the header or the block hash, and omitted when empty so
    /// historical blocks keep their exact encoding (bincode, JSON and CBOR).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<crate::signature::Signature>,
    /// Optional priority tips (ledger units), same order as `transactions`.
    /// Empty means every tip is `0` (historical / untipped blocks). When non-empty must match
    /// `transactions.len()`. Omitted when empty so historical encodings stay byte-identical.
    /// Not part of the header or block hash; verified with signatures on import.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_tips: Vec<u128>,
}

#[derive(Deserialize)]
struct LegacyHeaderNoBlobsRoot {
    parent_hash: Hash,
    height: u64,
    timestamp: u64,
    proposer: AccountId,
    tx_root: Hash,
    receipts_root: Hash,
    state_root: Hash,
}

impl From<LegacyHeaderNoBlobsRoot> for BlockHeader {
    fn from(h: LegacyHeaderNoBlobsRoot) -> Self {
        BlockHeader {
            parent_hash: h.parent_hash,
            height: h.height,
            timestamp: h.timestamp,
            proposer: h.proposer,
            tx_root: h.tx_root,
            receipts_root: h.receipts_root,
            state_root: h.state_root,
            blobs_root: None,
        }
    }
}

#[derive(Deserialize)]
struct LegacyBlockEncoding {
    header: LegacyHeaderNoBlobsRoot,
    transactions: Vec<Transaction>,
}

#[derive(Deserialize)]
struct LegacyBlockWithSigs {
    header: LegacyHeaderNoBlobsRoot,
    transactions: Vec<Transaction>,
    signatures: Vec<crate::signature::Signature>,
}

/// Current header (incl. `blobs_root`) + txs, signatures omitted (common pre-activation layout).
#[derive(Deserialize)]
struct BlockNoSigs {
    header: BlockHeader,
    transactions: Vec<Transaction>,
}

#[derive(Deserialize)]
struct BlockWithSignaturesOnly {
    header: BlockHeader,
    transactions: Vec<Transaction>,
    #[serde(default)]
    signatures: Vec<crate::signature::Signature>,
}

impl Block {
    /// Tip at `index`, or `0` when the block has no tip vector.
    pub fn priority_tip_at(&self, index: usize) -> u128 {
        self.priority_tips.get(index).copied().unwrap_or(0)
    }

    fn decode_exact<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Option<T> {
        let mut cur = std::io::Cursor::new(bytes);
        let v = bincode::deserialize_from::<_, T>(&mut cur).ok()?;
        if cur.position() as usize == bytes.len() {
            Some(v)
        } else {
            None
        }
    }

    /// Decode a bincode-encoded block across layout eras:
    /// - current header (`blobs_root`) + optional signatures + optional priority tips
    /// - current header + signatures only / no signatures
    /// - historical header without `blobs_root` (+ optional signatures)
    pub fn from_bincode(bytes: &[u8]) -> Result<Block, bincode::Error> {
        // Prefer exact-length layout attempts. Relying on `#[serde(default)]` trailing fields with
        // bincode is unreliable when prior fields used `skip_serializing_if`.
        if let Some(b) = Self::decode_exact::<Block>(bytes) {
            if b.priority_tips.is_empty() || b.priority_tips.len() == b.transactions.len() {
                return Ok(b);
            }
        }
        if let Some(w) = Self::decode_exact::<BlockWithSignaturesOnly>(bytes) {
            return Ok(Block {
                header: w.header,
                transactions: w.transactions,
                signatures: w.signatures,
                priority_tips: Vec::new(),
            });
        }
        if let Some(b) = Self::decode_exact::<BlockNoSigs>(bytes) {
            return Ok(Block {
                header: b.header,
                transactions: b.transactions,
                signatures: Vec::new(),
                priority_tips: Vec::new(),
            });
        }
        if let Some(l) = Self::decode_exact::<LegacyBlockWithSigs>(bytes) {
            return Ok(Block {
                header: l.header.into(),
                transactions: l.transactions,
                signatures: l.signatures,
                priority_tips: Vec::new(),
            });
        }
        if let Some(l) = Self::decode_exact::<LegacyBlockEncoding>(bytes) {
            return Ok(Block {
                header: l.header.into(),
                transactions: l.transactions,
                signatures: Vec::new(),
                priority_tips: Vec::new(),
            });
        }
        // Fall back to best-effort derived decode for a clearer error.
        bincode::deserialize::<Block>(bytes)
    }

    /// The transactions paired with their signatures (`None` when the block carries none or the
    /// counts differ). Tips default to 0 when the tip vector is empty.
    pub fn signed_transactions(&self) -> Option<Vec<crate::signature::SignedTransaction>> {
        if self.signatures.len() != self.transactions.len() {
            return None;
        }
        if !self.priority_tips.is_empty() && self.priority_tips.len() != self.transactions.len() {
            return None;
        }
        Some(
            self.transactions
                .iter()
                .zip(&self.signatures)
                .enumerate()
                .map(|(i, (tx, sig))| crate::signature::SignedTransaction {
                    tx: tx.clone(),
                    signature: *sig,
                    priority_tip: self.priority_tip_at(i),
                })
                .collect(),
        )
    }

    pub fn hash(&self) -> Hash {
        let mut h = hasher();
        h.update(&bincode::serialize(&self.header).unwrap_or_default());
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
    }
}

/// Max `return_data` bytes stored per receipt (indexer / RPC bound).
pub const MAX_RECEIPT_RETURN_DATA_BYTES: usize = 24 * 1024;
/// Max UTF-8 bytes for optional `error` on failed receipts.
pub const MAX_RECEIPT_ERROR_STRING_BYTES: usize = 2048;
/// Max indexed topics per log (Boing VM `LOG0`..`LOG4`).
pub const MAX_EXECUTION_LOG_TOPICS: usize = 4;
/// Max payload bytes per log entry.
pub const MAX_EXECUTION_LOG_DATA_BYTES: usize = 1024;
/// Max log entries per transaction (receipt bound).
pub const MAX_EXECUTION_LOGS_PER_TX: usize = 24;

/// Leading byte of `ContractDeploy` (all variants) **payload**: the remainder is **init code**.
///
/// The node runs init once at deploy (`CALLER` = tx sender, `ADDRESS` = new contract). Logs are
/// recorded on the deploy transaction receipt. The memory slice passed to `RETURN` becomes the
/// **runtime bytecode** stored for the contract (empty slice if execution ends via `STOP` without
/// `RETURN`). Payloads **without** this prefix keep legacy behavior: bytes are stored verbatim and
/// no VM execution runs at deploy.
///
/// `0xFD` is not a valid Boing VM opcode; it is stripped before QA bytecode walks and init execution.
pub const CONTRACT_DEPLOY_INIT_CODE_MARKER: u8 = 0xFD;

/// `true` if deploy bytecode uses [`CONTRACT_DEPLOY_INIT_CODE_MARKER`] (init + `RETURN` → runtime).
#[inline]
pub fn contract_deploy_uses_init_code(bytecode: &[u8]) -> bool {
    bytecode.first().copied() == Some(CONTRACT_DEPLOY_INIT_CODE_MARKER)
}

/// Bytecode validated at deploy and passed to the interpreter for init (marker stripped when present).
#[inline]
pub fn contract_deploy_init_body(bytecode: &[u8]) -> &[u8] {
    if contract_deploy_uses_init_code(bytecode) {
        bytecode.get(1..).unwrap_or(&[])
    } else {
        bytecode
    }
}

/// One log entry emitted during contract execution (indexer-facing; no bloom in header).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

fn sanitize_logs(mut logs: Vec<ExecutionLog>) -> Vec<ExecutionLog> {
    logs.truncate(MAX_EXECUTION_LOGS_PER_TX);
    for log in logs.iter_mut() {
        log.topics.truncate(MAX_EXECUTION_LOG_TOPICS);
        if log.data.len() > MAX_EXECUTION_LOG_DATA_BYTES {
            log.data.truncate(MAX_EXECUTION_LOG_DATA_BYTES);
        }
    }
    logs
}

/// Result of applying one transaction in a block (execution summary for indexers; no log bloom yet).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub tx_id: Hash,
    pub block_height: u64,
    pub tx_index: u32,
    pub success: bool,
    pub gas_used: u64,
    pub return_data: Vec<u8>,
    /// VM logs from `ContractCall`, or from `ContractDeploy` when init-code mode is used
    /// ([`CONTRACT_DEPLOY_INIT_CODE_MARKER`]); otherwise empty (e.g. transfers, legacy deploy).
    pub logs: Vec<ExecutionLog>,
    pub error: Option<String>,
}

impl ExecutionReceipt {
    pub fn from_tx_outcome(
        tx: &Transaction,
        block_height: u64,
        tx_index: u32,
        success: bool,
        gas_used: u64,
        return_data: Vec<u8>,
        logs: Vec<ExecutionLog>,
        error: Option<String>,
    ) -> Self {
        let mut return_data = return_data;
        if return_data.len() > MAX_RECEIPT_RETURN_DATA_BYTES {
            return_data.truncate(MAX_RECEIPT_RETURN_DATA_BYTES);
        }
        let error = error.and_then(|mut s| {
            if s.len() > MAX_RECEIPT_ERROR_STRING_BYTES {
                s.truncate(MAX_RECEIPT_ERROR_STRING_BYTES);
            }
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        });
        Self {
            tx_id: tx.id(),
            block_height,
            tx_index,
            success,
            gas_used,
            return_data,
            logs: sanitize_logs(logs),
            error,
        }
    }
}

#[cfg(test)]
mod block_encoding_tests {
    use super::*;
    use crate::signature::Signature;

    fn header() -> BlockHeader {
        BlockHeader {
            parent_hash: Hash::ZERO,
            height: 7,
            timestamp: 1,
            proposer: AccountId([1; 32]),
            tx_root: Hash::ZERO,
            receipts_root: Hash::ZERO,
            state_root: Hash::ZERO,
            blobs_root: None,
        }
    }

    #[test]
    fn legacy_header_without_blobs_root_decodes_as_none() {
        #[derive(Serialize)]
        struct Old {
            parent_hash: Hash,
            height: u64,
            timestamp: u64,
            proposer: AccountId,
            tx_root: Hash,
            receipts_root: Hash,
            state_root: Hash,
        }
        let h = header();
        let old = bincode::serialize(&Old {
            parent_hash: h.parent_hash,
            height: h.height,
            timestamp: h.timestamp,
            proposer: h.proposer,
            tx_root: h.tx_root,
            receipts_root: h.receipts_root,
            state_root: h.state_root,
        })
        .unwrap();
        // Inactive headers omit blobs_root — same bincode as pre-Phase-3.
        assert_eq!(bincode::serialize(&h).unwrap().len(), old.len());
        let with = BlockHeader {
            blobs_root: Some(Hash([9; 32])),
            ..h.clone()
        };
        assert!(bincode::serialize(&with).unwrap().len() > old.len());
        let block_old = {
            #[derive(Serialize)]
            struct OldBlock<'a> {
                header: &'a Old,
                transactions: &'a Vec<Transaction>,
            }
            let txs = Vec::<Transaction>::new();
            let oh = Old {
                parent_hash: h.parent_hash,
                height: h.height,
                timestamp: h.timestamp,
                proposer: h.proposer,
                tx_root: h.tx_root,
                receipts_root: h.receipts_root,
                state_root: h.state_root,
            };
            bincode::serialize(&OldBlock {
                header: &oh,
                transactions: &txs,
            })
            .unwrap()
        };
        let decoded = Block::from_bincode(&block_old).unwrap();
        assert_eq!(decoded.header.blobs_root, None);
        assert_eq!(decoded.header.height, 7);
        // Hash must match the historical (no blobs_root) id.
        let mut hh = hasher();
        hh.update(&old);
        let mut out = [0u8; 32];
        out.copy_from_slice(hh.finalize().as_bytes());
        assert_eq!(
            Block {
                header: h,
                transactions: vec![],
                signatures: vec![],
                priority_tips: vec![],
            }
            .hash(),
            Hash(out)
        );
    }

    #[test]
    fn unsigned_block_roundtrips_and_legacy_decodes() {
        let b = Block {
            header: header(),
            transactions: vec![],
            signatures: vec![],
            priority_tips: vec![],
        };
        let enc = bincode::serialize(&b).unwrap();
        assert_eq!(Block::from_bincode(&enc).unwrap(), b);
        // Historical (header without blobs_root) still decodes.
        #[derive(Serialize)]
        struct OldHeader {
            parent_hash: Hash,
            height: u64,
            timestamp: u64,
            proposer: AccountId,
            tx_root: Hash,
            receipts_root: Hash,
            state_root: Hash,
        }
        #[derive(Serialize)]
        struct OldBlock {
            header: OldHeader,
            transactions: Vec<Transaction>,
        }
        let legacy = bincode::serialize(&OldBlock {
            header: OldHeader {
                parent_hash: Hash::ZERO,
                height: 7,
                timestamp: 1,
                proposer: AccountId([1; 32]),
                tx_root: Hash::ZERO,
                receipts_root: Hash::ZERO,
                state_root: Hash::ZERO,
            },
            transactions: vec![],
        })
        .unwrap();
        let decoded = Block::from_bincode(&legacy).unwrap();
        assert_eq!(decoded.header.blobs_root, None);
        let j = serde_json::to_value(&b).unwrap();
        assert!(j.get("signatures").is_none());
        let back: Block = serde_json::from_value(j).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn fly_testnet_sample_blocks_decode_and_chain() {
        // Samples pulled from boing-testnet-1 volume (pre–blobs_root header era).
        let dir = std::path::Path::new("/tmp/boing-chain-sample");
        if !dir.join("0.bin").is_file() {
            return;
        }
        let g = Block::from_bincode(&std::fs::read(dir.join("0.bin")).unwrap()).unwrap();
        let b1 = Block::from_bincode(&std::fs::read(dir.join("1.bin")).unwrap()).unwrap();
        assert_eq!(g.header.height, 0);
        assert_eq!(b1.header.height, 1);
        assert_eq!(g.header.blobs_root, None);
        assert_eq!(b1.header.blobs_root, None);
        assert_eq!(
            g.hash(),
            b1.header.parent_hash,
            "historical parent link must survive Option blobs_root + exact decode"
        );
        for name in ["919.bin", "920.bin", "1050.bin", "1150.bin", "1187.bin"] {
            let b = Block::from_bincode(&std::fs::read(dir.join(name)).unwrap()).unwrap();
            assert!(b.header.blobs_root.is_none());
        }
    }

    #[test]
    fn signed_block_roundtrips() {
        let tx = Transaction {
            nonce: 0,
            sender: AccountId([2; 32]),
            payload: TransactionPayload::Transfer { to: AccountId([3; 32]), amount: 1 },
            access_list: AccessList::default(),
        };
        let b = Block { header: header(), transactions: vec![tx], signatures: vec![Signature([5; 64])], priority_tips: vec![] };
        let enc = bincode::serialize(&b).unwrap();
        assert_eq!(Block::from_bincode(&enc).unwrap(), b);
        let back: Block = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        assert_eq!(back, b);
        assert_eq!(b.hash(), Block { signatures: vec![], ..b.clone() }.hash(), "signatures are not in the hash");
    }
}

#[cfg(test)]
mod receipt_root_tests {
    use super::*;

    #[test]
    fn receipts_root_empty_is_zero() {
        assert_eq!(receipts_root(&[]), Hash::ZERO);
    }

    #[test]
    fn receipts_root_order_matters() {
        let id = AccountId([9u8; 32]);
        let tx1 = Transaction {
            nonce: 0,
            sender: id,
            payload: TransactionPayload::Transfer { to: id, amount: 1 },
            access_list: AccessList::default(),
        };
        let tx2 = Transaction {
            nonce: 1,
            sender: id,
            payload: TransactionPayload::Transfer { to: id, amount: 2 },
            access_list: AccessList::default(),
        };
        let r1 = ExecutionReceipt::from_tx_outcome(&tx1, 1, 0, true, 100, vec![], vec![], None);
        let r2 = ExecutionReceipt::from_tx_outcome(&tx2, 1, 1, true, 200, vec![1], vec![], None);
        let a = receipts_root(&[r1.clone(), r2.clone()]);
        let b = receipts_root(&[r2, r1]);
        assert_ne!(a, b);
    }
}
