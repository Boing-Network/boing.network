//! Boing VM — deterministic execution engine.

use boing_primitives::{
    blob_sample_ask_escrow_account, blob_sample_serve_challenge_bond_escrow_account,
    contract_deploy_init_body, contract_deploy_uses_init_code, create2_contract_address,
    media_blob_gas, media_blob_max_bytes_at, media_blob_max_das_chunks_at, media_blob_meta_tx_gas,
    nonce_derived_contract_address, validate_batch_calls, validate_blob_announce_with_cap,
    validate_challenge_requires_settle, validate_pay_blob_sample,
    validate_pay_blob_sample_with_max_chunks, validate_put_blob_with_cap,
    validate_register_blob_sample_offer, validate_renew_media_blob_pin,
    validate_settle_blob_sample_match, AccountId, AccountState, BatchCall, BlobId, BlobRetention,
    ExecutionLog, Hash, MediaBlobDasSample, QaPoolVoteKind, Transaction, TransactionPayload,
    PHASE11_FAILED_SERVE_PENALTY, PHASE11_SERVE_CHALLENGE_BOND,
    PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
};
use boing_state::SettledSampleMatch;
use boing_qa::{
    check_contract_deploy_full, check_contract_deploy_full_with_metadata, QaPoolGovernanceConfig,
    QaQuorumDecision, QaResult, RuleRegistry,
};
use boing_state::{
    unpaid_counted_voters, EscrowedSampleOffer, QaPendingRecord, StateStore,
};

use super::interpreter::{Interpreter, VmExecutionContext};
use crate::gas::base;

/// Gas used by a transaction.
pub const GAS_PER_TRANSFER: u64 = 21_000;
pub const GAS_PER_CONTRACT_CALL: u64 = 40_000_000;
/// Gas budget for **`0xFD` init** that runs at deploy (SSTORE bootstrap + `MSTORE`/`RETURN` of large runtime).
pub const GAS_PER_CONTRACT_DEPLOY_INIT: u64 = 5_000_000;
pub const GAS_PER_CONTRACT_DEPLOY: u64 = 200_000;
/// Gas for an on-chain [`boing_primitives::TransactionPayload::QaPoolVote`].
pub const GAS_PER_QA_POOL_VOTE: u64 = base::QA_POOL_VOTE;
/// Fixed gas for a [`TransactionPayload::Batch`] envelope (signature, nonce, bookkeeping) — same as one transfer.
pub const GAS_BATCH_BASE: u64 = GAS_PER_TRANSFER;
/// Gas per native-transfer step inside a batch (cheaper than a standalone transfer: no extra
/// signature check or nonce bump). 16 transfers ≈ 133k gas vs 336k as separate txs.
pub const GAS_PER_BATCH_TRANSFER: u64 = 7_000;
/// Gas per contract-call step inside a batch, on top of the interpreter gas the call consumed.
pub const GAS_PER_BATCH_CONTRACT_CALL_STEP: u64 = 7_000;
/// Total interpreter gas shared by **all** contract-call steps of one batch (one call's worth),
/// so a batch can never consume more VM work than a single `ContractCall`.
pub const GAS_BATCH_CONTRACT_BUDGET: u64 = GAS_PER_CONTRACT_CALL;

/// Successful VM / tx application outcome (for receipts and simulation).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmExecutionResult {
    pub gas_used: u64,
    pub return_data: Vec<u8>,
    pub logs: Vec<ExecutionLog>,
}

/// Minimal state for Transfer-only execution (used by parallel path).
pub trait TransferState {
    fn get(&self, id: &AccountId) -> Option<AccountState>;
    fn get_mut(&mut self, id: &AccountId) -> Option<&mut AccountState>;
    fn insert(&mut self, account: boing_primitives::Account);
}

impl TransferState for StateStore {
    fn get(&self, id: &AccountId) -> Option<AccountState> {
        self.get(id).cloned()
    }
    fn get_mut(&mut self, id: &AccountId) -> Option<&mut AccountState> {
        self.get_mut(id)
    }
    fn insert(&mut self, account: boing_primitives::Account) {
        self.insert(account);
    }
}

impl TransferState for super::parallel::ExecutionView {
    fn get(&self, id: &AccountId) -> Option<AccountState> {
        self.get(id)
    }
    fn get_mut(&mut self, id: &AccountId) -> Option<&mut AccountState> {
        self.get_mut(id)
    }
    fn insert(&mut self, account: boing_primitives::Account) {
        self.insert(account);
    }
}

fn default_qa_registry() -> RuleRegistry {
    RuleRegistry::new()
}

/// Virtual machine for executing transactions.
pub struct Vm {
    /// Same rule set as mempool deploy QA (`check_contract_deploy_full_with_metadata`), with the
    /// native-value opcodes rejected (blocks below [`Self::native_value_from`]).
    qa_registry: RuleRegistry,
    /// The same registry accepting the native-value opcodes (blocks from the activation height).
    qa_registry_native: RuleRegistry,
    /// From this height contract calls may carry native value and the native-value opcodes execute
    /// (`BALANCE`, `CALLVALUE`, `SELFBALANCE`, `NATIVE_TRANSFER`). 0 = from genesis.
    native_value_from: u64,
    /// Pool membership / quorum / voter-reward knobs (execution-time public votes).
    qa_pool_config: QaPoolGovernanceConfig,
    /// Chain id for v2 validator registration proofs (security review #14); `None` = v1 only.
    registration_chain_id: Option<u64>,
    /// From this height only the v2 (chain-bound) registration proof is accepted.
    registration_v2_from: u64,
    /// From this height the 2026-10-17 security-review rules apply (#6, #10, #11); 0 = from genesis.
    security_v4_from: u64,
    /// From this height the ledger is in base units (10^18 per BOING); whole-BOING prices (bond, name
    /// rent, QA reward / min stake) are scaled by [`Denomination::scale`]. 0 = from genesis.
    base_units_from: u64,
    /// Media blob PutBlob activation: `None` = feature off; `Some(h)` = active at height ≥ `h`.
    media_blobs_from: Option<u64>,
    /// Phase 3: pin `blobs_root` into [`boing_primitives::BlockHeader`] from this height.
    /// `None` = omit field (historical encoding). Independent of PutBlob activation.
    blobs_root_header_from: Option<u64>,
    /// Phase 7: raise Put/Announce max toward 256 MiB from this height. `None` = stay at 16 MiB.
    media_blobs_phase7_cap_from: Option<u64>,
}

impl Vm {
    pub fn set_registration_proof_rule(&mut self, chain_id: Option<u64>, v2_from: u64) {
        self.registration_chain_id = chain_id;
        self.registration_v2_from = v2_from;
    }

    /// Activation height of the security-review v4 rules (sender-keyed name commitments, open-validator
    /// registry cap, no deregistering while jailed).
    pub fn set_security_v4_from(&mut self, from: u64) {
        self.security_v4_from = from;
    }

    pub fn security_v4_from(&self) -> u64 {
        self.security_v4_from
    }

    pub fn set_base_units_from(&mut self, from: u64) {
        self.base_units_from = from;
    }

    /// Enable L1 media blobs from `height` (`None` disables PutBlob).
    pub fn set_media_blobs_from(&mut self, from: Option<u64>) {
        self.media_blobs_from = from;
    }

    pub fn media_blobs_from(&self) -> Option<u64> {
        self.media_blobs_from
    }

    pub fn media_blobs_active(&self, height: u64) -> bool {
        self.media_blobs_from.is_some_and(|h| height >= h)
    }

    /// Phase 7 gated cap raise (`None` keeps the 16 MiB live max — Fly-safe default).
    pub fn set_media_blobs_phase7_cap_from(&mut self, from: Option<u64>) {
        self.media_blobs_phase7_cap_from = from;
    }

    pub fn media_blobs_phase7_cap_from(&self) -> Option<u64> {
        self.media_blobs_phase7_cap_from
    }

    pub fn media_blob_max_bytes(&self, height: u64) -> usize {
        media_blob_max_bytes_at(height, self.media_blobs_phase7_cap_from)
    }

    /// Enable header `blobs_root` from `height` (`None` keeps the field omitted).
    pub fn set_blobs_root_header_from(&mut self, from: Option<u64>) {
        self.blobs_root_header_from = from;
    }

    pub fn blobs_root_header_from(&self) -> Option<u64> {
        self.blobs_root_header_from
    }

    pub fn blobs_root_header_active(&self, height: u64) -> bool {
        self.blobs_root_header_from.is_some_and(|h| height >= h)
    }

    pub fn base_units_from(&self) -> u64 {
        self.base_units_from
    }

    /// Activation height of the native-value rules (value on contract calls, native opcodes).
    pub fn set_native_value_from(&mut self, from: u64) {
        self.native_value_from = from;
    }

    pub fn native_value_from(&self) -> u64 {
        self.native_value_from
    }

    pub fn native_value_active(&self, height: u64) -> bool {
        height >= self.native_value_from
    }

    /// `exec_ctx` with [`VmExecutionContext::native_value`] set from its block height.
    fn ctx_at(&self, exec_ctx: VmExecutionContext) -> VmExecutionContext {
        VmExecutionContext {
            native_value: self.native_value_active(exec_ctx.block_height),
            ..exec_ctx
        }
    }

    /// Deploy QA registry for a block: native-value opcodes accepted only once active.
    fn qa_at(&self, exec_ctx: VmExecutionContext) -> &RuleRegistry {
        if exec_ctx.native_value {
            &self.qa_registry_native
        } else {
            &self.qa_registry
        }
    }

    /// Ledger unit at `height`.
    pub fn denomination_at(&self, height: u64) -> boing_primitives::Denomination {
        if height >= self.base_units_from {
            boing_primitives::Denomination::BaseUnits
        } else {
            boing_primitives::Denomination::WholeBoing
        }
    }

    /// `(chain id, v2-only-from height)` of the validator registration proof rule.
    pub fn registration_proof_rule(&self) -> (Option<u64>, u64) {
        (self.registration_chain_id, self.registration_v2_from)
    }

    fn registration_proof_ok(&self, height: u64, owner: &AccountId, node_id: &AccountId, sig: &[u8]) -> bool {
        let v2 = self
            .registration_chain_id
            .is_some_and(|c| boing_primitives::verify_validator_registration_v2(owner, node_id, sig, c));
        match self.registration_chain_id {
            Some(_) if height >= self.registration_v2_from => v2,
            _ => v2 || boing_primitives::verify_validator_registration(owner, node_id, sig),
        }
    }

    pub fn new() -> Self {
        Self {
            qa_registry_native: default_qa_registry().with_native_value_opcodes(true),
            qa_registry: default_qa_registry(),
            native_value_from: 0,
            qa_pool_config: QaPoolGovernanceConfig::development_default(),
            registration_chain_id: None,
            registration_v2_from: 0,
            security_v4_from: 0,
            base_units_from: 0,
            media_blobs_from: None,
            blobs_root_header_from: None,
            media_blobs_phase7_cap_from: None,
        }
    }

    /// Use the same [RuleRegistry] as the node mempool so execution rejects the same deploys as admission.
    pub fn with_qa_registry(qa_registry: RuleRegistry) -> Self {
        Self {
            qa_registry_native: qa_registry.clone().with_native_value_opcodes(true),
            qa_registry: qa_registry.with_native_value_opcodes(false),
            native_value_from: 0,
            qa_pool_config: QaPoolGovernanceConfig::development_default(),
            registration_chain_id: None,
            registration_v2_from: 0,
            security_v4_from: 0,
            base_units_from: 0,
            media_blobs_from: None,
            blobs_root_header_from: None,
            media_blobs_phase7_cap_from: None,
        }
    }

    /// Registry + pool governance (public membership, voter rewards).
    pub fn with_qa_policies(
        qa_registry: RuleRegistry,
        qa_pool_config: QaPoolGovernanceConfig,
    ) -> Self {
        Self {
            qa_registry_native: qa_registry.clone().with_native_value_opcodes(true),
            qa_registry: qa_registry.with_native_value_opcodes(false),
            native_value_from: 0,
            qa_pool_config,
            registration_chain_id: None,
            registration_v2_from: 0,
            security_v4_from: 0,
            base_units_from: 0,
            media_blobs_from: None,
            blobs_root_header_from: None,
            media_blobs_phase7_cap_from: None,
        }
    }

    /// Reference to the QA registry used for contract deploy execution checks.
    pub fn qa_registry(&self) -> &RuleRegistry {
        &self.qa_registry
    }

    pub fn qa_pool_config(&self) -> &QaPoolGovernanceConfig {
        &self.qa_pool_config
    }

    /// Execute Transfer tx against any TransferState (for parallel path).
    pub fn execute_transfer<S: TransferState>(
        &self,
        tx: &Transaction,
        state: &mut S,
    ) -> Result<VmExecutionResult, VmError> {
        let sender_state = state.get(&tx.sender).ok_or(VmError::AccountNotFound)?;
        if sender_state.nonce != tx.nonce {
            return Err(VmError::InvalidNonce {
                expected: sender_state.nonce,
                got: tx.nonce,
            });
        }
        let TransactionPayload::Transfer { to, amount } = &tx.payload else {
            return Err(VmError::NotImplemented("Not a Transfer"));
        };
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.balance = sender_state
            .balance
            .checked_sub(*amount)
            .ok_or(VmError::InsufficientBalance)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        let to_state = state.get_mut(to);
        match to_state {
            Some(s) => s.balance = s.balance.saturating_add(*amount),
            None => {
                state.insert(boing_primitives::Account {
                    id: *to,
                    state: boing_primitives::AccountState {
                        balance: *amount,
                        nonce: 0,
                        stake: 0,
                        ..Default::default()
                    },
                });
            }
        }
        Ok(VmExecutionResult {
            gas_used: GAS_PER_TRANSFER,
            return_data: Vec::new(),
            logs: Vec::new(),
        })
    }

    /// Execute a single transaction against the state (block height / timestamp default to zero).
    pub fn execute(
        &self,
        tx: &Transaction,
        state: &mut StateStore,
    ) -> Result<VmExecutionResult, VmError> {
        self.execute_with_context(tx, state, VmExecutionContext::default())
    }

    /// Execute with block context for **`BlockHeight` (`0x40`)** / **`Timestamp` (`0x41`)** opcodes inside contract code.
    pub fn execute_with_context(
        &self,
        tx: &Transaction,
        state: &mut StateStore,
        exec_ctx: VmExecutionContext,
    ) -> Result<VmExecutionResult, VmError> {
        let exec_ctx = self.ctx_at(exec_ctx);
        // Nonce validation
        let sender_state = state.get(&tx.sender).ok_or(VmError::AccountNotFound)?;
        if sender_state.nonce != tx.nonce {
            return Err(VmError::InvalidNonce {
                expected: sender_state.nonce,
                got: tx.nonce,
            });
        }

        let gas_used = match &tx.payload {
            TransactionPayload::Bond { amount } => {
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                sender_state.balance = sender_state
                    .balance
                    .checked_sub(*amount)
                    .ok_or(VmError::InsufficientBalance)?;
                sender_state.stake = sender_state.stake.saturating_add(*amount);
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;
                base::BOND
            }
            TransactionPayload::Unbond { amount } => {
                let height = exec_ctx.block_height;
                let delay = boing_tokenomics::UNBONDING_DELAY_BLOCKS;
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                sender_state.stake = sender_state
                    .stake
                    .checked_sub(*amount)
                    .ok_or(VmError::InsufficientBalance)?;
                // Restart the unlock clock for the combined pending amount.
                sender_state.pending_unbond = sender_state.pending_unbond.saturating_add(*amount);
                sender_state.unbond_unlock_height = height.saturating_add(delay);
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;
                base::UNBOND
            }
            TransactionPayload::ClaimUnbond => {
                let height = exec_ctx.block_height;
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                if sender_state.pending_unbond == 0 {
                    return Err(VmError::InsufficientBalance);
                }
                if height < sender_state.unbond_unlock_height {
                    return Err(VmError::UnbondNotMature {
                        unlock_height: sender_state.unbond_unlock_height,
                        current_height: height,
                    });
                }
                let amount = sender_state.pending_unbond;
                sender_state.pending_unbond = 0;
                sender_state.unbond_unlock_height = 0;
                sender_state.balance = sender_state.balance.saturating_add(amount);
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;
                base::CLAIM_UNBOND
            }
            TransactionPayload::Transfer { to, amount } => {
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                sender_state.balance = sender_state
                    .balance
                    .checked_sub(*amount)
                    .ok_or(VmError::InsufficientBalance)?;
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;

                let to_state = state.get_mut(to);
                match to_state {
                    Some(s) => {
                        s.balance = s.balance.saturating_add(*amount);
                    }
                    None => {
                        state.insert(boing_primitives::Account {
                            id: *to,
                            state: boing_primitives::AccountState {
                                balance: *amount,
                                nonce: 0,
                                stake: 0,
                                ..Default::default()
                            },
                        });
                    }
                }
                GAS_PER_TRANSFER
            }
            TransactionPayload::ContractCall { contract, calldata } => {
                return self.execute_contract_call(state, tx, contract, calldata, 0, exec_ctx);
            }
            TransactionPayload::ContractCallWithValue {
                contract,
                calldata,
                value,
            } => {
                if !exec_ctx.native_value {
                    return Err(VmError::NativeValueNotActive);
                }
                return self.execute_contract_call(state, tx, contract, calldata, *value, exec_ctx);
            }
            TransactionPayload::ContractDeploy { .. }
            | TransactionPayload::ContractDeployWithPurpose { .. }
            | TransactionPayload::ContractDeployWithPurposeAndMetadata { .. } => {
                return self.execute_contract_deploy(state, tx, exec_ctx);
            }
            TransactionPayload::QaPoolVote { subject, vote } => {
                return self.execute_qa_pool_vote(state, tx, *subject, *vote, exec_ctx);
            }
            TransactionPayload::Batch { calls } => {
                return self.execute_batch(state, tx, calls, exec_ctx);
            }
            TransactionPayload::ValidatorRegister {
                node_id,
                node_signature,
            } => {
                let mut params = open_validator_params_at(state, exec_ctx.block_height)?;
                // The configured bond is whole BOING; the record and the debit are ledger units.
                params.bond = self.denomination_at(exec_ctx.block_height).scale(params.bond);
                if !self.registration_proof_ok(exec_ctx.block_height, &tx.sender, node_id, node_signature) {
                    return Err(VmError::OpenValidator(
                        boing_state::OpenValidatorError::InvalidNodeSignature.to_string(),
                    ));
                }
                let balance = state.get(&tx.sender).map(|s| s.balance).unwrap_or(0);
                if balance < params.bond {
                    return Err(VmError::InsufficientBalance);
                }
                let v4 = exec_ctx.block_height >= self.security_v4_from;
                state
                    .open_validators_mut()
                    .register_with_cap(&params, exec_ctx.block_height, tx.sender, *node_id, v4)
                    .map_err(|e| VmError::OpenValidator(e.to_string()))?;
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                sender_state.balance -= params.bond;
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;
                base::BOND
            }
            TransactionPayload::ValidatorDeregister { node_id } => {
                let params = open_validator_params_at(state, exec_ctx.block_height)?;
                let v4 = exec_ctx.block_height >= self.security_v4_from;
                let refund = state
                    .open_validators_mut()
                    .deregister_at(tx.sender, *node_id, exec_ctx.block_height, v4)
                    .map_err(|e| VmError::OpenValidator(e.to_string()))?;
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                if refund > 0 {
                    sender_state.pending_unbond =
                        sender_state.pending_unbond.saturating_add(refund);
                    sender_state.unbond_unlock_height = sender_state.unbond_unlock_height.max(
                        exec_ctx
                            .block_height
                            .saturating_add(params.deregister_delay),
                    );
                }
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;
                base::UNBOND
            }
            TransactionPayload::ValidatorUnjail { node_id } => {
                let _ = open_validator_params_at(state, exec_ctx.block_height)?;
                state
                    .open_validators_mut()
                    .unjail(tx.sender, *node_id, exec_ctx.block_height)
                    .map_err(|e| VmError::OpenValidator(e.to_string()))?;
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;
                base::UNBOND
            }
            TransactionPayload::NameCommit { .. }
            | TransactionPayload::NameRegister { .. }
            | TransactionPayload::NameRenew { .. }
            | TransactionPayload::NameTransfer { .. }
            | TransactionPayload::NameSetTarget { .. }
            | TransactionPayload::NameSetPrimary { .. }
            | TransactionPayload::NameRelease { .. } => {
                execute_name_tx(
                    state,
                    tx,
                    exec_ctx.block_height,
                    exec_ctx.block_height >= self.security_v4_from,
                    self.denomination_at(exec_ctx.block_height).unit(),
                )?
            }
            TransactionPayload::PutBlob {
                retention,
                content_type,
                data,
            } => {
                return self.execute_put_blob(
                    state,
                    tx,
                    *retention,
                    content_type,
                    data,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::BlobAnnounce {
                blob_id,
                size,
                retention,
                content_type,
            } => {
                return self.execute_blob_announce(
                    state,
                    tx,
                    blob_id,
                    *size,
                    *retention,
                    content_type,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::RenewMediaBlobPin {
                blob_id,
                size,
                extend_blocks,
            } => {
                return self.execute_renew_media_blob_pin(
                    state,
                    tx,
                    blob_id,
                    *size,
                    *extend_blocks,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::PayBlobSample {
                blob_id,
                chunk_index,
                chunk_count,
            } => {
                return self.execute_pay_blob_sample(
                    state,
                    tx,
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::RegisterBlobSampleOffer {
                blob_id,
                chunk_index,
                chunk_count,
                ask_fee,
                expires_at_height,
            } => {
                return self.execute_register_blob_sample_offer(
                    state,
                    tx,
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    *ask_fee,
                    *expires_at_height,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::SettleBlobSampleMatch {
                blob_id,
                chunk_index,
                chunk_count,
                provider,
            } => {
                return self.execute_settle_blob_sample_match(
                    state,
                    tx,
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    provider,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::CancelBlobSampleOffer {
                blob_id,
                chunk_index,
            } => {
                return self.execute_cancel_blob_sample_offer(
                    state,
                    tx,
                    blob_id,
                    *chunk_index,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::ChallengeBlobSampleServe {
                blob_id,
                chunk_index,
                chunk_count,
                provider,
            } => {
                return self.execute_challenge_blob_sample_serve(
                    state,
                    tx,
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    provider,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::RespondBlobSampleServe {
                blob_id,
                chunk_index,
                chunk_count,
                challenger,
                chunk_hash,
                chunks_root,
                siblings,
            } => {
                return self.execute_respond_blob_sample_serve(
                    state,
                    tx,
                    blob_id,
                    *chunk_index,
                    *chunk_count,
                    challenger,
                    *chunk_hash,
                    *chunks_root,
                    siblings,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::SubmitSlashAppeal { slash_id, evidence } => {
                return self.execute_submit_slash_appeal(
                    state,
                    tx,
                    *slash_id,
                    evidence,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::ResolveSlashAppeal {
                appeal_id,
                approved,
            } => {
                return self.execute_resolve_slash_appeal(
                    state,
                    tx,
                    *appeal_id,
                    *approved,
                    exec_ctx.block_height,
                );
            }
        };
        Ok(VmExecutionResult {
            gas_used,
            return_data: Vec::new(),
            logs: Vec::new(),
        })
    }

    /// Validate + meter a media PutBlob; return_data is the 32-byte [`boing_primitives::BlobId`].
    /// Bytes are not written into account SMT — the node sidecar persists them from the block tx.
    fn execute_put_blob(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        retention: BlobRetention,
        content_type: &str,
        data: &[u8],
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        let max = self.media_blob_max_bytes(height);
        let blob_id = validate_put_blob_with_cap(retention, content_type, data, max)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        let gas_used = media_blob_gas(data.len(), retention);
        Ok(VmExecutionResult {
            gas_used,
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    /// Commit to a blob id + size without inline bytes (Phase 2 data plane).
    fn execute_blob_announce(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        size: u32,
        retention: BlobRetention,
        content_type: &str,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        let max = self.media_blob_max_bytes(height);
        validate_blob_announce_with_cap(blob_id, size, retention, content_type, max)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        let gas_used = media_blob_gas(size as usize, retention);
        Ok(VmExecutionResult {
            gas_used,
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_renew_media_blob_pin(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        size: u32,
        extend_blocks: u64,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        validate_renew_media_blob_pin(blob_id, size, extend_blocks)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_pay_blob_sample(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        chunk_index: u32,
        chunk_count: u32,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        validate_pay_blob_sample(blob_id, chunk_index, chunk_count)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_register_blob_sample_offer(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        chunk_index: u32,
        chunk_count: u32,
        ask_fee: u128,
        expires_at_height: u64,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        let max_chunks = media_blob_max_das_chunks_at(height, self.media_blobs_phase7_cap_from);
        validate_pay_blob_sample_with_max_chunks(blob_id, chunk_index, chunk_count, max_chunks)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        validate_register_blob_sample_offer(
            blob_id,
            chunk_index,
            chunk_count,
            ask_fee,
            expires_at_height,
        )
        .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        // Refund any expired offers' escrow before admitting a new one.
        self.refund_expired_sample_offers(state, height)?;

        let escrow = blob_sample_ask_escrow_account();
        let offer = EscrowedSampleOffer {
            blob_id: *blob_id,
            chunk_index,
            chunk_count,
            provider: tx.sender,
            ask_fee,
            expires_at_height,
            registered_at_height: height,
        };
        let prev = state
            .sample_offers_mut()
            .register(offer)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        if let Some(old) = prev {
            if old.ask_fee > 0 {
                move_native(state, escrow, old.provider, old.ask_fee)?;
            }
        }
        if ask_fee > 0 {
            move_native(state, tx.sender, escrow, ask_fee)?;
        }

        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_settle_blob_sample_match(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        chunk_index: u32,
        chunk_count: u32,
        provider: &AccountId,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        let max_chunks = media_blob_max_das_chunks_at(height, self.media_blobs_phase7_cap_from);
        validate_pay_blob_sample_with_max_chunks(blob_id, chunk_index, chunk_count, max_chunks)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        self.refund_expired_sample_offers(state, height)?;

        let offer = state
            .sample_offers_mut()
            .settle(blob_id, chunk_index, provider, height)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        if offer.chunk_count != chunk_count {
            return Err(VmError::MediaBlob(
                "settle chunk_count does not match registered offer".into(),
            ));
        }
        validate_settle_blob_sample_match(&offer.to_on_chain(), &tx.sender, height, provider)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        let escrow = blob_sample_ask_escrow_account();
        if offer.ask_fee > 0 {
            // Release prepaid ask to provider (already held in escrow from Register).
            move_native(state, escrow, *provider, offer.ask_fee)?;
        }

        // Phase 12: retain settle receipt so payer may Challenge this provider.
        state
            .sample_offers_mut()
            .record_settle_receipt(SettledSampleMatch {
                blob_id: *blob_id,
                chunk_index,
                chunk_count: offer.chunk_count,
                provider: *provider,
                payer: tx.sender,
                settled_at_height: height,
            })
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_cancel_blob_sample_offer(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        chunk_index: u32,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        self.refund_expired_sample_offers(state, height)?;
        let offer = state
            .sample_offers_mut()
            .cancel(blob_id, chunk_index, &tx.sender)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let escrow = blob_sample_ask_escrow_account();
        if offer.ask_fee > 0 {
            move_native(state, escrow, tx.sender, offer.ask_fee)?;
        }
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn refund_expired_sample_offers(
        &self,
        state: &mut StateStore,
        height: u64,
    ) -> Result<(), VmError> {
        let expired = state.sample_offers_mut().take_expired(height);
        let escrow = blob_sample_ask_escrow_account();
        for o in expired {
            if o.ask_fee > 0 {
                move_native(state, escrow, o.provider, o.ask_fee)?;
            }
        }
        Ok(())
    }

    fn execute_challenge_blob_sample_serve(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        chunk_index: u32,
        chunk_count: u32,
        provider: &AccountId,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        let max_chunks = media_blob_max_das_chunks_at(height, self.media_blobs_phase7_cap_from);
        validate_pay_blob_sample_with_max_chunks(blob_id, chunk_index, chunk_count, max_chunks)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        // Settle any expired challenges + prune stale settle receipts before admitting a new one.
        apply_media_blob_block_housekeeping(state, height)?;

        // Phase 12: challenger must have previously settled a match with this provider.
        let has_receipt = state.sample_offers().has_settle_receipt(
            blob_id,
            chunk_index,
            provider,
            &tx.sender,
        );
        validate_challenge_requires_settle(has_receipt)
            .map_err(|e| VmError::MediaBlob(format!(
                "challenge requires prior SettleBlobSampleMatch: {e}"
            )))?;

        let bond = PHASE11_SERVE_CHALLENGE_BOND;
        let penalty = PHASE11_FAILED_SERVE_PENALTY;
        let challenger_bal = state.get(&tx.sender).map(|s| s.balance).unwrap_or(0);
        if challenger_bal < bond {
            return Err(VmError::InsufficientBalance);
        }
        let provider_bal = state.get(provider).map(|s| s.balance).unwrap_or(0);
        if provider_bal < penalty {
            return Err(VmError::MediaBlob(
                "provider balance below FailedServe penalty; cannot open challenge".into(),
            ));
        }

        state
            .sample_serve_mut()
            .challenge(
                *blob_id,
                chunk_index,
                chunk_count,
                *provider,
                tx.sender,
                height,
                bond,
                penalty,
            )
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        let escrow = blob_sample_serve_challenge_bond_escrow_account();
        // Escrow challenger bond + provider penalty up front so expiry settle cannot fail.
        if bond > 0 {
            move_native(state, tx.sender, escrow, bond)?;
        }
        if penalty > 0 {
            move_native(state, *provider, escrow, penalty)?;
        }

        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_respond_blob_sample_serve(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        chunk_index: u32,
        chunk_count: u32,
        challenger: &AccountId,
        chunk_hash: [u8; 32],
        chunks_root: [u8; 32],
        siblings: &[ [u8; 32] ],
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        apply_media_blob_block_housekeeping(state, height)?;

        let sample = MediaBlobDasSample {
            blob_id: *blob_id,
            chunk_index,
            chunk_count,
            chunk_hash,
            chunks_root: Hash(chunks_root),
            siblings: siblings.to_vec(),
        };
        let challenge = state
            .sample_serve_mut()
            .respond(
                blob_id,
                chunk_index,
                &tx.sender,
                challenger,
                height,
                &sample,
            )
            .map_err(|e| VmError::MediaBlob(e.to_string()))?
            .clone();

        // Valid respond: challenger bond → provider; escrowed provider penalty refunded.
        if !challenge.bond_settled {
            let escrow = blob_sample_serve_challenge_bond_escrow_account();
            if challenge.bond_amount > 0 {
                move_native(state, escrow, tx.sender, challenge.bond_amount)?;
            }
            if challenge.provider_penalty > 0 {
                move_native(state, escrow, tx.sender, challenge.provider_penalty)?;
            }
            state.sample_serve_mut().mark_bond_settled(
                blob_id,
                chunk_index,
                &tx.sender,
                challenger,
            );
        }
        // Phase 13: consume settle receipt so the challenge gate cannot be reused forever.
        let _ = state.sample_offers_mut().remove_settle_receipt(
            blob_id,
            chunk_index,
            &tx.sender,
            challenger,
        );

        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: blob_id.to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_submit_slash_appeal(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        slash_id: u64,
        evidence: &[u8],
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        state
            .slash_appeals_mut()
            .submit_appeal(
                slash_id,
                evidence.to_vec(),
                height,
                PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
            )
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: slash_id.to_le_bytes().to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_resolve_slash_appeal(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        appeal_id: u64,
        approved: bool,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
        }
        if !sender_may_resolve_slash_appeal(state, &tx.sender) {
            return Err(VmError::MediaBlob(
                "ResolveSlashAppeal requires a consensus validator".into(),
            ));
        }
        let slash = state
            .slash_appeals_mut()
            .resolve_appeal(appeal_id, approved)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?
            .clone();
        if approved {
            let restored = match slash.reason {
                boing_state::ConsensusSlashReason::BlobFailedServe => {
                    boing_tokenomics::restore_failed_serve_penalty(
                        state,
                        &slash.account,
                        slash.amount,
                    )
                }
                _ => boing_tokenomics::restore_equivocation_slash(
                    state,
                    &slash.account,
                    slash.amount,
                ),
            };
            let _ = restored;
        }
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: media_blob_meta_tx_gas(),
            return_data: appeal_id.to_le_bytes().to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_contract_deploy(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        exec_ctx: VmExecutionContext,
    ) -> Result<VmExecutionResult, VmError> {
        // Defense in depth: same full QA path as mempool (`Mempool::insert`), using payload fields.
        // Malicious blocks must not apply deploys that honest mempools would reject.
        let Some((bytecode, purpose, desc_hash, asset_name, asset_symbol)) =
            tx.payload.as_contract_deploy()
        else {
            return Err(VmError::NotImplemented("Not a contract deploy"));
        };

        match check_contract_deploy_full_with_metadata(
            bytecode,
            purpose,
            desc_hash,
            asset_name,
            asset_symbol,
            self.qa_at(exec_ctx),
        ) {
            QaResult::Allow => {}
            QaResult::Reject(r) => {
                return Err(VmError::QaRejected {
                    rule_id: r.rule_id.0,
                    message: r.message,
                });
            }
            QaResult::Unsure => {
                if self.qa_pool_config.public_membership {
                    return self.register_unsure_deploy(state, tx, exec_ctx.block_height);
                }
                // Legacy / admin-pool path: Unsure is not a hard reject. Community QA pool admits
                // some Unsure txs; at execution we only enforce Allow vs Reject.
                boing_telemetry::component_debug(
                    "boing_execution::vm",
                    "execution",
                    "deploy_qa_unsure_proceeding",
                    "contract deploy QA Unsure at execution — proceeding with deploy",
                );
            }
        }

        let contract_addr = if let Some(salt) = tx.payload.deploy_create2_salt() {
            create2_contract_address(&tx.sender, &salt, bytecode)
        } else {
            nonce_derived_contract_address(&tx.sender, tx.nonce)
        };
        if state.get(&contract_addr).is_some() || state.get_contract_code(&contract_addr).is_some()
        {
            return Err(VmError::DeploymentAddressInUse);
        }

        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;

        state.insert(boing_primitives::Account {
            id: contract_addr,
            state: boing_primitives::AccountState {
                balance: 0,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });

        let uses_init = contract_deploy_uses_init_code(bytecode);
        let init_body = contract_deploy_init_body(bytecode);
        let (gas_used, logs, stored_code) = if uses_init {
            let mut interpreter =
                Interpreter::new(init_body.to_vec(), GAS_PER_CONTRACT_DEPLOY_INIT);
            let g_init = interpreter.run_with_qa(
                tx.sender,
                contract_addr,
                &[],
                state,
                self.qa_at(exec_ctx),
                exec_ctx,
            )?;
            let runtime = interpreter.return_data.take().unwrap_or_default();
            let logs = std::mem::take(&mut interpreter.logs);
            let gas = GAS_PER_CONTRACT_DEPLOY.saturating_add(g_init);
            (gas, logs, runtime)
        } else {
            (GAS_PER_CONTRACT_DEPLOY, Vec::new(), bytecode.to_vec())
        };

        state.set_contract_code(contract_addr, stored_code);
        Ok(VmExecutionResult {
            gas_used,
            return_data: Vec::new(),
            logs,
        })
    }

    fn register_unsure_deploy(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        let subject = tx.id();
        if state.qa_pending(&subject).is_some() {
            return Err(VmError::QaPendingAlreadyRegistered);
        }
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        state.insert_qa_pending(subject, QaPendingRecord::new(tx.clone(), height));
        Ok(VmExecutionResult {
            gas_used: GAS_PER_CONTRACT_DEPLOY,
            return_data: b"qa_pending".to_vec(),
            logs: Vec::new(),
        })
    }

    fn execute_qa_pool_vote(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        subject: Hash,
        vote: QaPoolVoteKind,
        exec_ctx: VmExecutionContext,
    ) -> Result<VmExecutionResult, VmError> {
        let voter = tx.sender;
        let deployer = {
            let record = state
                .qa_pending(&subject)
                .ok_or(VmError::QaPendingNotFound)?;
            record.deployer
        };
        // QA pool amounts are configured in whole BOING: compare the stake in the same unit.
        let unit = self.denomination_at(exec_ctx.block_height).unit();
        let voter_stake = state.get(&voter).map(|s| s.stake).unwrap_or(0) / unit;
        self.qa_pool_config
            .voter_eligibility(voter, Some(deployer), Some(voter_stake))
            .map_err(|e| match e {
                boing_qa::QaVoterIneligible::DeployerConflict => {
                    VmError::QaVoterIneligible("deployer cannot vote on their own Unsure deploy")
                }
                boing_qa::QaVoterIneligible::InsufficientStake => {
                    VmError::QaVoterIneligible("voter stake below min_voter_stake")
                }
                boing_qa::QaVoterIneligible::NotMember => {
                    VmError::QaVoterIneligible("voter is not eligible under qa_pool_config")
                }
            })?;

        let sender_state = state.get_mut(&voter).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;

        let pay_abstain = self.qa_pool_config.pay_abstain;
        let reward_each = self.qa_pool_config.reward_per_counted_vote.saturating_mul(unit);
        let (decision, unpaid) = {
            let record = state
                .qa_pending_mut(&subject)
                .ok_or(VmError::QaPendingNotFound)?;
            record.votes.insert(voter, vote);

            let mut allow = 0usize;
            let mut reject = 0usize;
            for kind in record.votes.values() {
                match kind {
                    QaPoolVoteKind::Allow => allow += 1,
                    QaPoolVoteKind::Reject => reject += 1,
                    QaPoolVoteKind::Abstain => {}
                }
            }
            let decision = self.qa_pool_config.quorum_decision(allow, reject);
            if matches!(decision, QaQuorumDecision::Pending) {
                return Ok(VmExecutionResult {
                    gas_used: GAS_PER_QA_POOL_VOTE,
                    return_data: b"qa_vote_pending".to_vec(),
                    logs: Vec::new(),
                });
            }
            let unpaid = unpaid_counted_voters(record, pay_abstain);
            for id in &unpaid {
                record.rewarded.insert(*id);
            }
            (decision, unpaid)
        };
        let _paid = boing_tokenomics::pay_qa_counted_voters(state, &unpaid, reward_each);

        match decision {
            QaQuorumDecision::Allow => {
                let rec = state
                    .remove_qa_pending(&subject)
                    .ok_or(VmError::QaPendingNotFound)?;
                self.install_contract_from_deploy_tx(state, &rec.tx, exec_ctx)?;
                Ok(VmExecutionResult {
                    gas_used: GAS_PER_QA_POOL_VOTE,
                    return_data: b"qa_vote_allow".to_vec(),
                    logs: Vec::new(),
                })
            }
            QaQuorumDecision::Reject => {
                let _ = state.remove_qa_pending(&subject);
                Ok(VmExecutionResult {
                    gas_used: GAS_PER_QA_POOL_VOTE,
                    return_data: b"qa_vote_reject".to_vec(),
                    logs: Vec::new(),
                })
            }
            QaQuorumDecision::Pending => unreachable!(),
        }
    }

    /// Create the contract for a previously registered Unsure deploy (nonce already consumed).
    fn install_contract_from_deploy_tx(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        exec_ctx: VmExecutionContext,
    ) -> Result<(), VmError> {
        let Some((bytecode, _, _, _, _)) = tx.payload.as_contract_deploy() else {
            return Err(VmError::NotImplemented("QA allow of non-deploy"));
        };
        let contract_addr = if let Some(salt) = tx.payload.deploy_create2_salt() {
            create2_contract_address(&tx.sender, &salt, bytecode)
        } else {
            nonce_derived_contract_address(&tx.sender, tx.nonce)
        };
        if state.get(&contract_addr).is_some() || state.get_contract_code(&contract_addr).is_some()
        {
            return Err(VmError::DeploymentAddressInUse);
        }
        state.insert(boing_primitives::Account {
            id: contract_addr,
            state: boing_primitives::AccountState {
                balance: 0,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let uses_init = contract_deploy_uses_init_code(bytecode);
        let init_body = contract_deploy_init_body(bytecode);
        let stored_code = if uses_init {
            let mut interpreter =
                Interpreter::new(init_body.to_vec(), GAS_PER_CONTRACT_DEPLOY_INIT);
            let _g_init = interpreter.run_with_qa(
                tx.sender,
                contract_addr,
                &[],
                state,
                self.qa_at(exec_ctx),
                exec_ctx,
            )?;
            interpreter.return_data.take().unwrap_or_default()
        } else {
            bytecode.to_vec()
        };
        state.set_contract_code(contract_addr, stored_code);
        Ok(())
    }

    /// Apply every batch step in order. Any error aborts with `Err`; the block executor reverts the
    /// whole tx to its checkpoint (all-or-nothing) and consumes the nonce.
    fn execute_batch(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        calls: &[BatchCall],
        exec_ctx: VmExecutionContext,
    ) -> Result<VmExecutionResult, VmError> {
        validate_batch_calls(calls).map_err(|e| VmError::InvalidBatch(e.to_string()))?;
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;

        let mut gas_used = GAS_BATCH_BASE;
        let mut vm_budget = GAS_BATCH_CONTRACT_BUDGET;
        let mut logs = Vec::new();
        let mut return_data = Vec::new();
        for (index, call) in calls.iter().enumerate() {
            let step_err = |e: VmError| VmError::BatchStepFailed {
                index,
                reason: e.to_string(),
            };
            match call {
                BatchCall::Transfer { to, amount } => {
                    let s = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                    s.balance = s
                        .balance
                        .checked_sub(*amount)
                        .ok_or_else(|| step_err(VmError::InsufficientBalance))?;
                    match state.get_mut(to) {
                        Some(r) => r.balance = r.balance.saturating_add(*amount),
                        None => state.insert(boing_primitives::Account {
                            id: *to,
                            state: AccountState {
                                balance: *amount,
                                ..Default::default()
                            },
                        }),
                    }
                    gas_used = gas_used.saturating_add(GAS_PER_BATCH_TRANSFER);
                }
                BatchCall::ContractCall { contract, calldata } => {
                    let code = state
                        .get_contract_code(contract)
                        .ok_or_else(|| step_err(VmError::AccountNotFound))?
                        .clone();
                    let mut interpreter = Interpreter::new(code, vm_budget);
                    let g = interpreter
                        .run_with_qa(
                            tx.sender,
                            *contract,
                            calldata,
                            state,
                            self.qa_at(exec_ctx),
                            exec_ctx,
                        )
                        .map_err(step_err)?;
                    vm_budget = vm_budget.saturating_sub(g);
                    logs.append(&mut interpreter.logs);
                    return_data = interpreter.return_data.take().unwrap_or_default();
                    gas_used = gas_used
                        .saturating_add(GAS_PER_BATCH_CONTRACT_CALL_STEP)
                        .saturating_add(g);
                }
                BatchCall::ContractCallWithValue {
                    contract,
                    calldata,
                    value,
                } => {
                    if !exec_ctx.native_value {
                        return Err(step_err(VmError::NativeValueNotActive));
                    }
                    let code = state
                        .get_contract_code(contract)
                        .ok_or_else(|| step_err(VmError::AccountNotFound))?
                        .clone();
                    move_native(state, tx.sender, *contract, *value).map_err(step_err)?;
                    let mut interpreter = Interpreter::new(code, vm_budget);
                    interpreter.call_value = *value;
                    let g = interpreter
                        .run_with_qa(
                            tx.sender,
                            *contract,
                            calldata,
                            state,
                            self.qa_at(exec_ctx),
                            exec_ctx,
                        )
                        .map_err(step_err)?;
                    vm_budget = vm_budget.saturating_sub(g);
                    logs.append(&mut interpreter.logs);
                    return_data = interpreter.return_data.take().unwrap_or_default();
                    gas_used = gas_used
                        .saturating_add(GAS_PER_BATCH_CONTRACT_CALL_STEP)
                        .saturating_add(g);
                }
            }
        }
        Ok(VmExecutionResult {
            gas_used,
            return_data,
            logs,
        })
    }

    fn execute_contract_call(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        contract: &AccountId,
        calldata: &[u8],
        value: u128,
        exec_ctx: VmExecutionContext,
    ) -> Result<VmExecutionResult, VmError> {
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;

        let code = state
            .get_contract_code(contract)
            .ok_or(VmError::AccountNotFound)?
            .clone();
        // Value moves before the code runs (so `SELFBALANCE` includes it). Any later failure reverts
        // the whole tx to its checkpoint, refunding the value.
        if value > 0 {
            move_native(state, tx.sender, *contract, value)?;
        }
        let mut interpreter = Interpreter::new(code, GAS_PER_CONTRACT_CALL);
        interpreter.call_value = value;
        let gas_used = interpreter.run_with_qa(
            tx.sender,
            *contract,
            calldata,
            state,
            self.qa_at(exec_ctx),
            exec_ctx,
        )?;
        let return_data = interpreter.return_data.take().unwrap_or_default();
        let logs = std::mem::take(&mut interpreter.logs);
        Ok(VmExecutionResult {
            gas_used,
            return_data,
            logs,
        })
    }
}

/// Settle expired serve challenges: mark FailedServe, refund challenger bond, burn provider penalty,
/// assign a consensus `slash_id` (Phase 12), and seed the consensus slash-appeal book (Phase 14).
///
/// Called at the start of Challenge/Respond and once per block (producer + importer) so FailedServe
/// settles automatically without waiting for another sample tx.
pub fn settle_expired_serve_challenges(
    state: &mut StateStore,
    height: u64,
) -> Result<usize, VmError> {
    let failed = state.sample_serve_mut().take_newly_failed(height);
    if failed.is_empty() {
        return Ok(0);
    }
    let escrow = blob_sample_serve_challenge_bond_escrow_account();
    let burn = boing_tokenomics::FEE_BURN_SINK;
    let mut n = 0usize;
    for c in failed {
        if c.bond_settled {
            continue;
        }
        // Bond was escrowed from challenger at open → refund challenger.
        if c.bond_amount > 0 {
            move_native(state, escrow, c.challenger, c.bond_amount)?;
        }
        // Penalty was escrowed from provider at open → burn.
        if c.provider_penalty > 0 {
            move_native(state, escrow, burn, c.provider_penalty)?;
        }
        state.sample_serve_mut().mark_bond_settled(
            &c.blob_id,
            c.chunk_index,
            &c.provider,
            &c.challenger,
        );
        let slash_id = state.sample_serve_mut().assign_failed_serve_slash_id(
            &c.blob_id,
            c.chunk_index,
            &c.provider,
            &c.challenger,
        );
        // Phase 14: seed consensus slash book so Submit/Resolve txs can unburn without RPC.
        if let Some(id) = slash_id {
            let _ = state.slash_appeals_mut().record_slash_with_id(
                id,
                c.provider,
                c.provider_penalty,
                boing_state::ConsensusSlashReason::BlobFailedServe,
                height,
                PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
            );
        }
        // Phase 13: consume settle receipt when FailedServe closes the challenge.
        let _ = state.sample_offers_mut().remove_settle_receipt(
            &c.blob_id,
            c.chunk_index,
            &c.provider,
            &c.challenger,
        );
        n = n.saturating_add(1);
    }
    Ok(n)
}

/// Thin MVP: host-pinned consensus validator set, else open Active / bonded stake.
fn sender_may_resolve_slash_appeal(state: &StateStore, sender: &AccountId) -> bool {
    if let Some(set) = state.slash_appeal_resolvers() {
        return set.iter().any(|a| a == sender);
    }
    if let Some(rec) = state.open_validators().records.get(sender) {
        if matches!(
            rec.status,
            boing_state::ValidatorStatus::Active | boing_state::ValidatorStatus::Pending
        ) {
            return true;
        }
    }
    state.get(sender).is_some_and(|s| s.stake > 0)
}

/// Housekeeping: settle FailedServe + prune stale settle receipts (end-of-block / tx entry).
pub fn apply_media_blob_block_housekeeping(
    state: &mut StateStore,
    height: u64,
) -> Result<usize, VmError> {
    let settled = settle_expired_serve_challenges(state, height)?;
    let _ = state.sample_offers_mut().prune_settle_receipts(height);
    Ok(settled)
}

/// Move native BOING `from -> to` (journaled through [`StateStore::get_mut`] / `insert`, so a tx
/// revert undoes it). Creates `to` when absent; `from == to` and `amount == 0` are no-ops (after the
/// balance check).
pub(crate) fn move_native(
    state: &mut StateStore,
    from: AccountId,
    to: AccountId,
    amount: u128,
) -> Result<(), VmError> {
    let have = state.get(&from).map(|s| s.balance).unwrap_or(0);
    if have < amount {
        return Err(VmError::InsufficientBalance);
    }
    if amount == 0 || from == to {
        return Ok(());
    }
    let to_balance = state.get(&to).map(|s| s.balance).unwrap_or(0);
    let credited = to_balance
        .checked_add(amount)
        .ok_or(VmError::InsufficientBalance)?;
    let f = state.get_mut(&from).ok_or(VmError::AccountNotFound)?;
    f.balance -= amount;
    match state.get_mut(&to) {
        Some(t) => t.balance = credited,
        None => state.insert(boing_primitives::Account {
            id: to,
            state: AccountState {
                balance: amount,
                ..Default::default()
            },
        }),
    }
    Ok(())
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, Account, AccountState, CONTRACT_DEPLOY_INIT_CODE_MARKER};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn full_qa_reject_empty_bytecode() {
        let vm = Vm::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let tx = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: vec![],
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        assert!(matches!(
            vm.execute(&tx, &mut state),
            Err(VmError::QaRejected { .. })
        ));
    }

    #[test]
    fn full_qa_unsure_deploy_still_executes() {
        let reg = RuleRegistry::new().with_always_review_categories(vec!["meme".to_string()]);
        let vm = Vm::with_qa_registry(reg);
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let tx = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: vec![0x00],
                purpose_category: "meme".to_string(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        assert!(vm.execute(&tx, &mut state).is_ok());
        assert_eq!(state.get(&sender).unwrap().nonce, 1);
    }

    #[test]
    fn deploy_init_code_emits_log_and_installs_returned_runtime() {
        let vm = Vm::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        // Init: LOG0 (empty data), MSTORE zero word at 0, RETURN 1 byte → runtime STOP (0x00).
        let init = vec![
            0x60, 0x00, 0x60, 0x00, 0xa0, // LOG0
            0x60, 0x00, 0x60, 0x00, 0x52, // MSTORE
            0x60, 0x01, 0x60, 0x00, 0xf3, // RETURN offset 0 size 1
        ];
        let mut bytecode = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
        bytecode.extend(init);
        let deploy = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        let out = vm.execute(&deploy, &mut state).unwrap();
        assert_eq!(out.logs.len(), 1);
        assert!(out.logs[0].topics.is_empty());
        assert!(out.logs[0].data.is_empty());
        let contract = nonce_derived_contract_address(&sender, 0);
        assert_eq!(
            state.get_contract_code(&contract).unwrap().as_slice(),
            &[0x00]
        );
    }

    #[test]
    fn contract_call_smoke_emits_log_and_returns_caller() {
        let vm = Vm::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let code = crate::reference_token::smoke_contract_bytecode();
        let deploy = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: code,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        let out = vm.execute(&deploy, &mut state).unwrap();
        assert!(out.logs.is_empty());
        let contract = nonce_derived_contract_address(&sender, 0);
        let call = Transaction {
            nonce: 1,
            sender,
            payload: TransactionPayload::ContractCall {
                contract,
                calldata: b"ping".to_vec(),
            },
            access_list: AccessList::default(),
        };
        let out2 = vm.execute(&call, &mut state).unwrap();
        assert_eq!(out2.return_data.as_slice(), sender.0.as_slice());
        assert_eq!(out2.logs.len(), 1);
        assert_eq!(out2.logs[0].data.as_slice(), b"ping");
    }

    #[test]
    fn nested_call_opcode_merges_child_logs_and_caller_is_parent_contract() {
        use crate::bytecode::Opcode;

        let vm = Vm::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });

        let smoke_code = crate::reference_token::smoke_contract_bytecode();
        let deploy_smoke = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: smoke_code,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        vm.execute(&deploy_smoke, &mut state).unwrap();
        let smoke_addr = nonce_derived_contract_address(&sender, 0);

        let mut ping_word = [0u8; 32];
        ping_word[0..4].copy_from_slice(b"ping");
        let mut router_code = Vec::new();
        router_code.push(Opcode::Push32 as u8);
        router_code.extend_from_slice(&ping_word);
        router_code.extend([
            Opcode::Push1 as u8,
            0x00,
            Opcode::MStore as u8,
            Opcode::Push32 as u8,
        ]);
        router_code.extend_from_slice(&smoke_addr.0);
        router_code.extend([
            Opcode::Push1 as u8,
            0x00,
            Opcode::Push1 as u8,
            0x04,
            Opcode::Push1 as u8,
            0x20,
            Opcode::Push1 as u8,
            0x20,
            Opcode::Call as u8,
            Opcode::Stop as u8,
        ]);

        let deploy_router = Transaction {
            nonce: 1,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: router_code,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        vm.execute(&deploy_router, &mut state).unwrap();
        let router_addr = nonce_derived_contract_address(&sender, 1);

        let call = Transaction {
            nonce: 2,
            sender,
            payload: TransactionPayload::ContractCall {
                contract: router_addr,
                calldata: Vec::new(),
            },
            access_list: AccessList::default(),
        };
        let out = vm.execute(&call, &mut state).unwrap();
        assert_eq!(out.logs.len(), 1);
        assert_eq!(out.logs[0].data.as_slice(), b"ping");

        let smoke_caller_key = [1u8; 32];
        let stored = state.get_contract_storage(&smoke_addr, &smoke_caller_key);
        assert_eq!(stored, router_addr.0);
    }

    #[test]
    fn deploy_create2_address_matches_prediction() {
        let vm = Vm::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let code = vec![0x00, 0x00];
        let salt = [7u8; 32];
        let expected = create2_contract_address(&sender, &salt, &code);
        let deploy = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: code.clone(),
                create2_salt: Some(salt),
            },
            access_list: AccessList::default(),
        };
        assert!(vm.execute(&deploy, &mut state).is_ok());
        assert!(state.get_contract_code(&expected).is_some());
        assert_eq!(state.get(&sender).unwrap().nonce, 1);
    }

    #[test]
    fn deploy_create2_second_time_fails_address_in_use() {
        let vm = Vm::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let code = vec![0x00];
        let salt = [9u8; 32];
        let deploy1 = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: code.clone(),
                create2_salt: Some(salt),
            },
            access_list: AccessList::default(),
        };
        assert!(vm.execute(&deploy1, &mut state).is_ok());
        let deploy2 = Transaction {
            nonce: 1,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: code,
                create2_salt: Some(salt),
            },
            access_list: AccessList::default(),
        };
        assert!(matches!(
            vm.execute(&deploy2, &mut state),
            Err(VmError::DeploymentAddressInUse)
        ));
        assert_eq!(state.get(&sender).unwrap().nonce, 1);
    }

    #[test]
    fn in_contract_create2_deploys_deterministic_child() {
        use crate::bytecode::Opcode;

        let vm = Vm::new();
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });

        let salt = [1u8; 32];
        // Factory: MSTORE 0,0 (runtime byte 0x00 at mem[0]); CREATE2 with offset 0 size 1.
        let mut factory = Vec::new();
        factory.extend([
            Opcode::Push1 as u8,
            0x00,
            Opcode::Push1 as u8,
            0x00,
            Opcode::MStore as u8,
        ]);
        // Stack before CREATE2: offset (bottom), size, salt (top) — pops salt, size, offset.
        factory.extend([Opcode::Push1 as u8, 0x00, Opcode::Push1 as u8, 0x01]);
        factory.push(Opcode::Push32 as u8);
        factory.extend_from_slice(&salt);
        factory.extend([Opcode::Create2 as u8, Opcode::Stop as u8]);

        let deploy_factory = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: factory,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        vm.execute(&deploy_factory, &mut state).unwrap();
        let factory_addr = nonce_derived_contract_address(&sender, 0);
        let expected_child = create2_contract_address(&factory_addr, &salt, &[0x00]);

        let call = Transaction {
            nonce: 1,
            sender,
            payload: TransactionPayload::ContractCall {
                contract: factory_addr,
                calldata: Vec::new(),
            },
            access_list: AccessList::default(),
        };
        vm.execute(&call, &mut state).unwrap();
        assert_eq!(
            state.get_contract_code(&expected_child).unwrap().as_slice(),
            &[0x00]
        );
        assert_eq!(
            state.get(&factory_addr).unwrap().nonce,
            0,
            "factory nonce unchanged"
        );
    }
}

/// Name registry params when the registry is configured and `height` is at/after activation.
fn name_params_at(
    state: &StateStore,
    height: u64,
) -> Result<boing_state::NameRegistryParams, VmError> {
    let params = state
        .name_registry_params()
        .cloned()
        .ok_or_else(|| VmError::Name(boing_state::NameRegistryError::Disabled.to_string()))?;
    if height < params.activation_height {
        return Err(VmError::Name(
            boing_state::NameRegistryError::NotYetActive {
                activation_height: params.activation_height,
            }
            .to_string(),
        ));
    }
    Ok(params)
}

/// Name registry transactions (`Name*` payloads). Mutates the registry, burns rent from the sender and
/// bumps the nonce. All failures leave the state untouched (the caller reverts via checkpoint anyway).
/// `sender_keyed`: security review #6 (v4) — commitments are stored per sender, so copying a pending
/// `NameCommit` hash no longer blocks the victim's commit.
fn execute_name_tx(
    state: &mut StateStore,
    tx: &Transaction,
    height: u64,
    sender_keyed: bool,
    unit: u128,
) -> Result<u64, VmError> {
    let params = name_params_at(state, height)?;
    let me = tx.sender;
    let name_err = |e: boing_state::NameRegistryError| VmError::Name(e.to_string());
    let label = |raw: &str| -> Result<String, VmError> {
        boing_primitives::normalize_name(raw)
            .map_err(|e| name_err(boing_state::NameRegistryError::InvalidName(e.to_string())))
    };
    let balance = state.get(&me).map(|s| s.balance).unwrap_or(0);
    let rent: u128 = match &tx.payload {
        TransactionPayload::NameCommit { commitment } => {
            state
                .names_mut()
                .commit_keyed(&params, height, me, *commitment, sender_keyed)
                .map_err(name_err)?;
            0
        }
        TransactionPayload::NameRegister {
            name,
            salt,
            target,
            years,
        } => {
            let l = label(name)?;
            let (rent, _) = state
                .names()
                .check_register_keyed(&params, height, me, &l, salt, *years, sender_keyed)
                .map_err(name_err)?;
            // Rent tables are whole BOING; charge in ledger units.
            let rent = rent.saturating_mul(unit);
            if balance < rent {
                return Err(name_err(
                    boing_state::NameRegistryError::InsufficientBalance { rent },
                ));
            }
            state
                .names_mut()
                .register_keyed(&params, height, me, &l, salt, *target, *years, sender_keyed)
                .map_err(name_err)?
                .saturating_mul(unit)
        }
        TransactionPayload::NameRenew { name, years } => {
            let l = label(name)?;
            // Security review #5: no full registry clone per renewal. Renew in place; on insufficient
            // balance the error makes the executor revert the tx through its (journaled) checkpoint.
            let rent = state
                .names_mut()
                .renew(&params, height, me, &l, *years)
                .map_err(name_err)?
                .saturating_mul(unit);
            if balance < rent {
                return Err(name_err(
                    boing_state::NameRegistryError::InsufficientBalance { rent },
                ));
            }
            rent
        }
        TransactionPayload::NameTransfer { name, new_owner } => {
            let l = label(name)?;
            state
                .names_mut()
                .transfer(&params, height, me, &l, *new_owner)
                .map_err(name_err)?;
            0
        }
        TransactionPayload::NameSetTarget { name, target } => {
            let l = label(name)?;
            state
                .names_mut()
                .set_target(&params, height, me, &l, *target)
                .map_err(name_err)?;
            0
        }
        TransactionPayload::NameSetPrimary { name } => {
            let l = match name {
                Some(n) => Some(label(n)?),
                None => None,
            };
            state
                .names_mut()
                .set_primary(&params, height, me, l.as_deref())
                .map_err(name_err)?;
            0
        }
        TransactionPayload::NameRelease { name } => {
            let l = label(name)?;
            state
                .names_mut()
                .release(&params, height, me, &l)
                .map_err(name_err)?;
            0
        }
        _ => unreachable!("execute_name_tx called with a non-name payload"),
    };
    let sender_state = state.get_mut(&me).ok_or(VmError::AccountNotFound)?;
    // Rent is burned: removed from the payer, credited to nobody.
    sender_state.balance = sender_state
        .balance
        .checked_sub(rent)
        .ok_or(VmError::InsufficientBalance)?;
    sender_state.nonce = sender_state
        .nonce
        .checked_add(1)
        .ok_or(VmError::NonceOverflow)?;
    Ok(base::NAME)
}

/// Open validator params when the mode is configured and `height` is at/after activation.
fn open_validator_params_at(
    state: &StateStore,
    height: u64,
) -> Result<boing_state::OpenValidatorParams, VmError> {
    let params = state.open_validator_params().cloned().ok_or_else(|| {
        VmError::OpenValidator(boing_state::OpenValidatorError::Disabled.to_string())
    })?;
    if height < params.activation_height {
        return Err(VmError::OpenValidator(
            boing_state::OpenValidatorError::NotYetActive {
                activation_height: params.activation_height,
            }
            .to_string(),
        ));
    }
    Ok(params)
}

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("Account not found")]
    AccountNotFound,
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Unbond not mature: unlock at height {unlock_height}, current {current_height}")]
    UnbondNotMature {
        unlock_height: u64,
        current_height: u64,
    },
    #[error("Nonce overflow")]
    NonceOverflow,
    #[error("Invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },
    #[error("Not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("Out of gas")]
    OutOfGas,
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Invalid bytecode")]
    InvalidBytecode,
    #[error("Invalid jump destination")]
    InvalidJump,
    #[error("Invalid log event: {0}")]
    InvalidLog(&'static str),
    #[error("Deployment address already has an account or code")]
    DeploymentAddressInUse,
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Contract call depth limit exceeded")]
    CallDepthExceeded,
    #[error("CALL input or output buffer exceeds protocol limit")]
    CallBufferTooLarge,
    #[error("QA rejected: {rule_id} — {message}")]
    QaRejected { rule_id: String, message: String },
    #[error("QA pool vote ineligible: {0}")]
    QaVoterIneligible(&'static str),
    #[error("QA pool item not registered on-chain")]
    QaPendingNotFound,
    #[error("QA pool item already registered")]
    QaPendingAlreadyRegistered,
    #[error("In-contract CREATE2 is not available for this storage backend")]
    Create2NotSupported,
    #[error("CREATE2 init code size is zero")]
    Create2InitCodeEmpty,
    #[error("Invalid batch: {0}")]
    InvalidBatch(String),
    #[error("Batch step {index} failed: {reason} (whole batch reverted)")]
    BatchStepFailed { index: usize, reason: String },
    #[error("Open validator: {0}")]
    OpenValidator(String),
    #[error("Name registry: {0}")]
    Name(String),
    #[error("Native value on contract calls is not active at this height")]
    NativeValueNotActive,
    #[error("Native balance / transfer is not available for this storage backend")]
    NativeValueNotSupported,
    #[error("Media blobs (PutBlob) are not active at this height (activation={from:?})")]
    MediaBlobsNotActive { from: Option<u64> },
    #[error("Media blob: {0}")]
    MediaBlob(String),
}

/// Apply CREATE2-style deploy from inside a contract: QA, address derivation, account insert, optional init, runtime install.
/// `deployer` is the **current contract** (`ADDRESS`); init runs with `CALLER` = `deployer` (factory semantics).
/// `parent_call_depth` is the depth of the frame that executed `CREATE2` (constructor runs at `+1`).
pub(crate) fn apply_in_tx_create2(
    state: &mut StateStore,
    qa_registry: &RuleRegistry,
    deployer: AccountId,
    salt: [u8; 32],
    bytecode: Vec<u8>,
    init_gas_limit: u64,
    parent_logs: &mut Vec<ExecutionLog>,
    parent_call_depth: u8,
    exec_ctx: VmExecutionContext,
) -> Result<(AccountId, u64), VmError> {
    if bytecode.is_empty() {
        return Err(VmError::Create2InitCodeEmpty);
    }

    match check_contract_deploy_full(&bytecode, Some("dapp"), None, qa_registry) {
        QaResult::Allow => {}
        QaResult::Reject(r) => {
            return Err(VmError::QaRejected {
                rule_id: r.rule_id.0,
                message: r.message,
            });
        }
        QaResult::Unsure => {
            boing_telemetry::component_debug(
                "boing_execution::vm",
                "execution",
                "create2_qa_unsure_proceeding",
                "in-tx CREATE2 QA Unsure at execution — proceeding",
            );
        }
    }

    let contract_addr = create2_contract_address(&deployer, &salt, &bytecode);
    if state.get(&contract_addr).is_some() || state.get_contract_code(&contract_addr).is_some() {
        return Err(VmError::DeploymentAddressInUse);
    }

    state.insert(boing_primitives::Account {
        id: contract_addr,
        state: boing_primitives::AccountState {
            balance: 0,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });

    let uses_init = contract_deploy_uses_init_code(&bytecode);
    let init_body = contract_deploy_init_body(&bytecode);
    let (stored_code, init_gas) = if uses_init {
        let mut interpreter = Interpreter::new(init_body.to_vec(), init_gas_limit);
        let init_depth = parent_call_depth.saturating_add(1);
        if init_depth >= crate::interpreter::MAX_CALL_DEPTH {
            return Err(VmError::CallDepthExceeded);
        }
        let g = interpreter.run_nested(
            deployer,
            contract_addr,
            &[],
            state,
            qa_registry,
            init_depth,
            exec_ctx,
        )?;
        Interpreter::merge_child_logs(parent_logs, &interpreter)?;
        let runtime = interpreter.return_data.take().unwrap_or_default();
        (runtime, g)
    } else {
        (bytecode, 0)
    };

    state.set_contract_code(contract_addr, stored_code);
    Ok((contract_addr, init_gas))
}

#[cfg(test)]
mod registration_proof_rule_tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    /// Security review #14: before the activation height the v1 proof (and v2 for the right chain)
    /// is accepted; from it only the v2 proof for this chain.
    #[test]
    fn registration_proof_is_height_gated_and_chain_bound() {
        let node = SigningKey::from_bytes(&[7u8; 32]);
        let node_id = AccountId(node.verifying_key().to_bytes());
        let owner = AccountId([9u8; 32]);
        let v1 = boing_primitives::sign_validator_registration(&owner, &node);
        let v2 = boing_primitives::sign_validator_registration_v2(&owner, &node, 6913);
        let other = boing_primitives::sign_validator_registration_v2(&owner, &node, 6914);
        let mut vm = Vm::new();
        assert!(vm.registration_proof_ok(5, &owner, &node_id, &v1), "no chain id: v1");
        assert!(!vm.registration_proof_ok(5, &owner, &node_id, &v2));
        vm.set_registration_proof_rule(Some(6913), 10);
        assert!(vm.registration_proof_ok(9, &owner, &node_id, &v1), "v1 before activation");
        assert!(vm.registration_proof_ok(9, &owner, &node_id, &v2), "v2 before activation");
        assert!(!vm.registration_proof_ok(9, &owner, &node_id, &other));
        assert!(!vm.registration_proof_ok(10, &owner, &node_id, &v1), "v1 rejected from activation");
        assert!(vm.registration_proof_ok(10, &owner, &node_id, &v2));
        assert!(!vm.registration_proof_ok(10, &owner, &node_id, &other), "other chain rejected");
        assert!(!vm.registration_proof_ok(10, &AccountId([8u8; 32]), &node_id, &v2), "bound to owner");
    }
}

#[cfg(test)]
mod slash_appeal_phase14_tests {
    use super::*;
    use boing_primitives::{Account, AccessList, PHASE14_MAX_APPEAL_EVIDENCE_BYTES};
    use boing_state::ConsensusSlashReason;

    fn fund(state: &mut StateStore, id: AccountId, balance: u128, stake: u128, nonce: u64) {
        state.insert(Account {
            id,
            state: AccountState {
                balance,
                nonce,
                stake,
                ..Default::default()
            },
        });
    }

    fn ctx(h: u64) -> VmExecutionContext {
        VmExecutionContext {
            block_height: h,
            block_timestamp: 0,
            native_value: false,
        }
    }

    #[test]
    fn submit_resolve_approved_restores_failed_serve_balance() {
        let provider = AccountId([2u8; 32]);
        let validator = AccountId([3u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 50, 0);
        fund(&mut state, validator, 1_000_000, 0, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                7,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                10,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![validator]));

        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));

        let submit = Transaction {
            nonce: 0,
            sender: provider,
            payload: TransactionPayload::SubmitSlashAppeal {
                slash_id: 7,
                evidence: b"ok".to_vec(),
            },
            access_list: AccessList::new(vec![provider], vec![provider]),
        };
        vm.execute_with_context(&submit, &mut state, ctx(50)).unwrap();
        assert_eq!(state.slash_appeals().list_appeals().len(), 1);

        let resolve = Transaction {
            nonce: 0,
            sender: validator,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id: 1,
                approved: true,
            },
            access_list: AccessList::new(vec![validator], vec![validator]),
        };
        vm.execute_with_context(&resolve, &mut state, ctx(51)).unwrap();
        assert!(state.slash_appeals().is_reversed(7));
        assert_eq!(state.get(&provider).unwrap().balance, 1_100);
        assert_eq!(state.get(&provider).unwrap().stake, 50);
    }

    #[test]
    fn reject_resolve_leaves_balance_unchanged() {
        let provider = AccountId([4u8; 32]);
        let validator = AccountId([5u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, validator, 1_000_000, 0, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 500, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                1,
                provider,
                500,
                ConsensusSlashReason::BlobFailedServe,
                1,
                100,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![validator]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        let submit = Transaction {
            nonce: 0,
            sender: provider,
            payload: TransactionPayload::SubmitSlashAppeal {
                slash_id: 1,
                evidence: vec![],
            },
            access_list: AccessList::new(vec![provider], vec![provider]),
        };
        vm.execute_with_context(&submit, &mut state, ctx(2)).unwrap();
        let resolve = Transaction {
            nonce: 0,
            sender: validator,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id: 1,
                approved: false,
            },
            access_list: AccessList::new(vec![validator], vec![validator]),
        };
        vm.execute_with_context(&resolve, &mut state, ctx(3)).unwrap();
        assert!(!state.slash_appeals().is_reversed(1));
        assert_eq!(state.get(&provider).unwrap().balance, 100);
    }

    #[test]
    fn evidence_oversize_and_non_validator_rejected() {
        let provider = AccountId([6u8; 32]);
        let validator = AccountId([7u8; 32]);
        let outsider = AccountId([8u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, validator, 1_000_000, 0, 0);
        fund(&mut state, outsider, 1_000_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                1,
                provider,
                10,
                ConsensusSlashReason::BlobFailedServe,
                1,
                100,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![validator]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));

        let oversize = Transaction {
            nonce: 0,
            sender: provider,
            payload: TransactionPayload::SubmitSlashAppeal {
                slash_id: 1,
                evidence: vec![0u8; PHASE14_MAX_APPEAL_EVIDENCE_BYTES + 1],
            },
            access_list: AccessList::new(vec![provider], vec![provider]),
        };
        assert!(vm.execute_with_context(&oversize, &mut state, ctx(2)).is_err());

        let submit = Transaction {
            nonce: 0,
            sender: provider,
            payload: TransactionPayload::SubmitSlashAppeal {
                slash_id: 1,
                evidence: vec![],
            },
            access_list: AccessList::new(vec![provider], vec![provider]),
        };
        vm.execute_with_context(&submit, &mut state, ctx(2)).unwrap();

        let bad = Transaction {
            nonce: 0,
            sender: outsider,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id: 1,
                approved: true,
            },
            access_list: AccessList::new(vec![outsider], vec![outsider]),
        };
        let err = vm.execute_with_context(&bad, &mut state, ctx(3)).unwrap_err();
        assert!(format!("{err}").contains("consensus validator"));
    }

    #[test]
    fn double_resolve_rejected() {
        let provider = AccountId([9u8; 32]);
        let validator = AccountId([10u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 0, 0, 0);
        fund(&mut state, validator, 1_000_000, 0, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 50, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                3,
                provider,
                50,
                ConsensusSlashReason::BlobFailedServe,
                1,
                100,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![validator]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 3,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        let resolve = Transaction {
            nonce: 0,
            sender: validator,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id: 1,
                approved: true,
            },
            access_list: AccessList::new(vec![validator], vec![validator]),
        };
        vm.execute_with_context(&resolve, &mut state, ctx(3)).unwrap();
        let mut resolve2 = resolve.clone();
        resolve2.nonce = 1;
        assert!(vm.execute_with_context(&resolve2, &mut state, ctx(4)).is_err());
    }
}
