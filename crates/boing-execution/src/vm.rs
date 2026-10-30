//! Boing VM — deterministic execution engine.

use boing_primitives::{
    blob_sample_ask_escrow_account, blob_sample_serve_challenge_bond_escrow_account,
    contract_deploy_init_body, contract_deploy_uses_init_code, create2_contract_address,
    default_signing_chain_id, media_blob_gas, media_blob_max_bytes_at,
    media_blob_max_das_chunks_at, media_blob_meta_tx_gas, nonce_derived_contract_address,
    phase20_is_legacy_electorate_snapshot, phase20_pin_set_commitment,
    phase20_snapshot_electorate_w, phase32_admit_bls_aggregate_resolve_tx,
    phase32_committee_requires_attested, phase32_committee_requires_bls_aggregate,
    phase32_decode_signer_bitset, phase32_verify_aggregate_resolve_self_contained,
    validate_batch_calls, validate_blob_announce_with_cap, validate_challenge_requires_settle,
    validate_pay_blob_sample, validate_pay_blob_sample_with_max_chunks, validate_put_blob_with_cap,
    validate_register_blob_sample_offer, validate_renew_media_blob_pin,
    validate_settle_blob_sample_match, verify_slash_appeal_resolve_ballot_dual, AccountId,
    AccountState, BatchCall, BlobId, BlobRetention, BlsAggregateResolveSelfContainedPayload,
    ExecutionLog, Hash, MediaBlobDasSample, QaPoolVoteKind, Transaction, TransactionPayload,
    PHASE11_FAILED_SERVE_PENALTY, PHASE11_SERVE_CHALLENGE_BOND,
    PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS, PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
    PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED,
};
use boing_qa::{
    check_contract_deploy_full, check_contract_deploy_full_with_metadata, QaPoolGovernanceConfig,
    QaQuorumDecision, QaResult, RuleRegistry,
};
use boing_state::SettledSampleMatch;
use boing_state::{unpaid_counted_voters, EscrowedSampleOffer, QaPendingRecord, StateStore};

use super::interpreter::{Interpreter, VmExecutionContext};
use crate::gas::base;

/// Gas used by a transaction.
pub const GAS_PER_TRANSFER: u64 = 21_000;
/// Per-call **gas limit** for contract execution (EVM-style ceiling, not a flat charge).
/// Interpreter usage is what gets billed; this constant caps CPU work per call (issue #61 item 25).
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
    /// From this height the VM hardening rules apply (audit #61 items 3, 22, 23). 0 = from genesis.
    vm_hardening_from: u64,
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
    /// Phase 32: flip multi-pin committee from Ed25519 tx 32 → BLS aggregate tx 33.
    /// `None` = Phase 31 policy (multi-pin default stays 32).
    bls_committee_flip_from: Option<u64>,
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

    /// Phase 32 BLS multi-pin committee flip (`None` keeps Ed25519 tx 32 default).
    pub fn set_bls_committee_flip_from(&mut self, from: Option<u64>) {
        self.bls_committee_flip_from = from;
    }

    pub fn bls_committee_flip_from(&self) -> Option<u64> {
        self.bls_committee_flip_from
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

    /// Activation height of the VM hardening rules (init-runtime QA, memory gas, stack cap).
    pub fn set_vm_hardening_from(&mut self, from: u64) {
        self.vm_hardening_from = from;
    }

    pub fn vm_hardening_from(&self) -> u64 {
        self.vm_hardening_from
    }

    pub fn vm_hardening_active(&self, height: u64) -> bool {
        height >= self.vm_hardening_from
    }

    /// `exec_ctx` with [`VmExecutionContext::native_value`] / [`VmExecutionContext::vm_hardening`]
    /// set from its block height.
    fn ctx_at(&self, exec_ctx: VmExecutionContext) -> VmExecutionContext {
        VmExecutionContext {
            native_value: self.native_value_active(exec_ctx.block_height),
            vm_hardening: self.vm_hardening_active(exec_ctx.block_height),
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

    fn registration_proof_ok(
        &self,
        height: u64,
        owner: &AccountId,
        node_id: &AccountId,
        sig: &[u8],
    ) -> bool {
        let v2 = self.registration_chain_id.is_some_and(|c| {
            boing_primitives::verify_validator_registration_v2(owner, node_id, sig, c)
        });
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
            vm_hardening_from: 0,
            qa_pool_config: QaPoolGovernanceConfig::development_default(),
            registration_chain_id: None,
            registration_v2_from: 0,
            security_v4_from: 0,
            base_units_from: 0,
            media_blobs_from: None,
            blobs_root_header_from: None,
            media_blobs_phase7_cap_from: None,
            bls_committee_flip_from: None,
        }
    }

    /// Use the same [RuleRegistry] as the node mempool so execution rejects the same deploys as admission.
    pub fn with_qa_registry(qa_registry: RuleRegistry) -> Self {
        Self {
            qa_registry_native: qa_registry.clone().with_native_value_opcodes(true),
            qa_registry: qa_registry.with_native_value_opcodes(false),
            native_value_from: 0,
            vm_hardening_from: 0,
            qa_pool_config: QaPoolGovernanceConfig::development_default(),
            registration_chain_id: None,
            registration_v2_from: 0,
            security_v4_from: 0,
            base_units_from: 0,
            media_blobs_from: None,
            blobs_root_header_from: None,
            media_blobs_phase7_cap_from: None,
            bls_committee_flip_from: None,
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
            vm_hardening_from: 0,
            qa_pool_config,
            registration_chain_id: None,
            registration_v2_from: 0,
            security_v4_from: 0,
            base_units_from: 0,
            media_blobs_from: None,
            blobs_root_header_from: None,
            media_blobs_phase7_cap_from: None,
            bls_committee_flip_from: None,
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
                params.bond = self
                    .denomination_at(exec_ctx.block_height)
                    .scale(params.bond);
                if !self.registration_proof_ok(
                    exec_ctx.block_height,
                    &tx.sender,
                    node_id,
                    node_signature,
                ) {
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
            | TransactionPayload::NameRelease { .. } => execute_name_tx(
                state,
                tx,
                exec_ctx.block_height,
                exec_ctx.block_height >= self.security_v4_from,
                self.denomination_at(exec_ctx.block_height).unit(),
            )?,
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
                    None,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::ResolveSlashAppealAttested {
                appeal_id,
                approved,
                ballot_signature,
            } => {
                return self.execute_resolve_slash_appeal(
                    state,
                    tx,
                    *appeal_id,
                    *approved,
                    Some(*ballot_signature),
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::ResolveSlashAppealBlsAggregate {
                appeal_id,
                approved,
                signer_bitset,
                aggregate_signature,
                signer_bls_pubkeys,
            } => {
                return self.execute_resolve_slash_appeal_bls_aggregate(
                    state,
                    tx,
                    *appeal_id,
                    *approved,
                    signer_bitset,
                    *aggregate_signature,
                    signer_bls_pubkeys,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::GrantBlobDownload { blob_id, .. } => {
                return self.execute_blob_download_acl_tx(
                    state,
                    tx,
                    blob_id,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::RevokeBlobDownload { blob_id, .. } => {
                return self.execute_blob_download_acl_tx(
                    state,
                    tx,
                    blob_id,
                    exec_ctx.block_height,
                );
            }
            TransactionPayload::ReportEquivocation { evidence } => {
                return self.execute_report_equivocation(
                    state,
                    tx,
                    evidence,
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

    /// Phase 34: Grant/RevokeBlobDownload — meter + nonce; ACL lives in the node sidecar.
    fn execute_blob_download_acl_tx(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        blob_id: &BlobId,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        if !self.media_blobs_active(height) {
            return Err(VmError::MediaBlobsNotActive {
                from: self.media_blobs_from,
            });
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
        let has_receipt =
            state
                .sample_offers()
                .has_settle_receipt(blob_id, chunk_index, provider, &tx.sender);
        validate_challenge_requires_settle(has_receipt).map_err(|e| {
            VmError::MediaBlob(format!(
                "challenge requires prior SettleBlobSampleMatch: {e}"
            ))
        })?;

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
        siblings: &[[u8; 32]],
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

    /// Apply an equivocation slash inside block execution (consensus-ordered).
    ///
    /// Verifies the two conflicting votes, burns
    /// [`boing_tokenomics::EQUIVOCATION_SLASH_BPS`] of the offender's active stake, and seeds the
    /// consensus slash-appeal book. Idempotent for `(validator, round)`.
    fn execute_report_equivocation(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        evidence: &boing_primitives::EquivocationEvidence,
        _height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        evidence
            .verify()
            .map_err(|e| VmError::MediaBlob(format!("invalid equivocation evidence: {e}")))?;
        let offender = evidence.validator();
        let round = evidence.round();
        if let Some(existing_id) = state
            .slash_appeals()
            .find_equivocation_at_round(&offender, round)
            .map(|s| s.id)
        {
            // Already applied in a prior block (or earlier tx) — succeed without re-burning.
            let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
            sender_state.nonce = sender_state
                .nonce
                .checked_add(1)
                .ok_or(VmError::NonceOverflow)?;
            return Ok(VmExecutionResult {
                gas_used: GAS_PER_TRANSFER,
                return_data: existing_id.to_le_bytes().to_vec(),
                logs: Vec::new(),
            });
        }
        let burned = boing_tokenomics::slash_equivocation_stake(state, &offender);
        let slash_id = state
            .slash_appeals_mut()
            .record_slash(
                offender,
                burned,
                boing_state::ConsensusSlashReason::Equivocation,
                round,
                boing_tokenomics::EQUIVOCATION_APPEAL_WINDOW_BLOCKS,
            )
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;
        Ok(VmExecutionResult {
            gas_used: GAS_PER_TRANSFER,
            return_data: slash_id.to_le_bytes().to_vec(),
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
        // Phase 15: always-on — equivocation/liveness appeals must work when media is inactive.
        // Phase 20: snapshot electorate W (+ pin-set commitment) at Submit for frozen ⌈2W/3⌉.
        let pinned: Option<Vec<AccountId>> = state.slash_appeal_resolvers().map(|s| s.to_vec());
        let (electorate_w_snapshot, pin_set_commitment) = match pinned.as_deref() {
            Some(set) if !set.is_empty() => {
                let stakes: Vec<u128> = set
                    .iter()
                    .map(|id| state.get(id).map(|a| a.stake).unwrap_or(0))
                    .collect();
                (
                    phase20_snapshot_electorate_w(&stakes),
                    phase20_pin_set_commitment(set),
                )
            }
            _ => (
                phase20_snapshot_electorate_w(&[]),
                phase20_pin_set_commitment(&[]),
            ),
        };
        state
            .slash_appeals_mut()
            .submit_appeal(
                slash_id,
                evidence.to_vec(),
                height,
                PHASE14_MAX_APPEAL_EVIDENCE_BYTES,
                electorate_w_snapshot,
                pin_set_commitment,
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
        ballot_signature: Option<boing_primitives::SlashAppealBallotSignature>,
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        // Phase 15/16/18: always-on — not gated on media-blobs activation.
        // Phase 21: tip vs resolve_deadline — expire Pending and reject after window.
        if !sender_may_resolve_slash_appeal(state, &tx.sender) {
            return Err(VmError::MediaBlob(
                "ResolveSlashAppeal requires a consensus validator".into(),
            ));
        }
        state
            .slash_appeals_mut()
            .gate_resolve_window(appeal_id, height)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        // Phase 18/22 attested path: look up pending appeal slash_id + pin_set_commitment
        // and dual-verify ballot (V2 preferred, then V1) against tx.sender + chain_id.
        // Phase 19/31: when |pinned| > 1 and flip unset, unattested 31 is rejected
        // (committee nets must use 32). Phase 32: when flip active and |pinned| > 1,
        // reject both 31 and 32 — use ResolveSlashAppealBlsAggregate (33).
        // Historical unattested votes already recorded keep their weight mid-tally —
        // this gate is admission-only. Phase 22: new casts should sign V2 (pin-bound).
        let pinned_len = state.slash_appeal_resolvers().map(|s| s.len()).unwrap_or(0);
        if phase32_committee_requires_bls_aggregate(
            pinned_len,
            height,
            self.bls_committee_flip_from,
        ) {
            return Err(VmError::MediaBlob(
                "ResolveSlashAppeal / ResolveSlashAppealAttested (31/32) rejected when BLS committee flip active and |slash_appeal_resolvers| > 1; use ResolveSlashAppealBlsAggregate (33)"
                    .into(),
            ));
        }
        let ballot_to_store = if let Some(sig) = ballot_signature {
            let (slash_id, pin_set_commitment) = state
                .slash_appeals()
                .get_appeal(appeal_id)
                .filter(|a| a.status == boing_state::ConsensusAppealStatus::Pending)
                .map(|a| (a.slash_id, a.pin_set_commitment))
                .ok_or_else(|| {
                    VmError::MediaBlob(
                        "appeal not found or not pending for attested resolve".into(),
                    )
                })?;
            let chain_id = self
                .registration_chain_id
                .or_else(default_signing_chain_id)
                .unwrap_or(0);
            verify_slash_appeal_resolve_ballot_dual(
                chain_id,
                appeal_id,
                slash_id,
                approved,
                &tx.sender,
                &pin_set_commitment,
                &sig,
            )
            .map_err(|e| VmError::MediaBlob(format!("invalid resolve ballot: {e}")))?;
            Some(sig)
        } else {
            if phase32_committee_requires_attested(pinned_len, height, self.bls_committee_flip_from)
            {
                return Err(VmError::MediaBlob(
                    "ResolveSlashAppeal (31) rejected when |slash_appeal_resolvers| > 1; use ResolveSlashAppealAttested (32)"
                        .into(),
                ));
            }
            None
        };
        // Phase 17/20: voter weight = sender stake at cast. Electorate W is the
        // Submit-height snapshot when present; pre-V4 legacy rows (sentinel 0 + zero
        // pin commit) recompute live W each cast so mid-upgrade tallies do not strand.
        // Unpinned / empty pin → effective W=1 (solo-dev one-shot). W=0 never finalizes.
        let pinned: Option<Vec<AccountId>> = state.slash_appeal_resolvers().map(|s| s.to_vec());
        let appeal_snap = state
            .slash_appeals()
            .get_appeal(appeal_id)
            .map(|a| (a.electorate_w_snapshot, a.pin_set_commitment))
            .ok_or_else(|| VmError::MediaBlob("appeal not found".into()))?;
        let (electorate_w_snapshot, pin_set_commitment) = appeal_snap;
        let current_pin_commit = match pinned.as_deref() {
            Some(set) if !set.is_empty() => phase20_pin_set_commitment(set),
            _ => phase20_pin_set_commitment(&[]),
        };
        let (voter_weight, electorate_w) =
            if phase20_is_legacy_electorate_snapshot(electorate_w_snapshot, &pin_set_commitment) {
                // Legacy pre-V4 Pending: live recompute (Phase 17 behavior).
                match pinned.as_deref() {
                    Some(set) if !set.is_empty() => {
                        let mut w = 0u128;
                        for id in set {
                            w = w.saturating_add(state.get(id).map(|a| a.stake).unwrap_or(0));
                        }
                        let voter_w = state.get(&tx.sender).map(|a| a.stake).unwrap_or(0);
                        (voter_w, w)
                    }
                    _ => (1u128, 1u128),
                }
            } else {
                if current_pin_commit != pin_set_commitment {
                    return Err(VmError::MediaBlob(
                        "slash appeal pin set changed since submit".into(),
                    ));
                }
                let voter_w = match pinned.as_deref() {
                    Some(set) if !set.is_empty() => {
                        state.get(&tx.sender).map(|a| a.stake).unwrap_or(0)
                    }
                    _ => 1u128,
                };
                (voter_w, electorate_w_snapshot)
            };
        let outcome = state
            .slash_appeals_mut()
            .cast_resolve_vote(
                appeal_id,
                tx.sender,
                approved,
                voter_weight,
                electorate_w,
                ballot_to_store,
            )
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        if matches!(
            outcome,
            boing_state::CastResolveOutcome::Finalized { approved: true }
        ) {
            let slash_id = state
                .slash_appeals()
                .get_appeal(appeal_id)
                .map(|a| a.slash_id)
                .ok_or_else(|| VmError::MediaBlob("appeal not found after finalize".into()))?;
            let slash = state
                .slash_appeals()
                .get_slash(slash_id)
                .cloned()
                .ok_or_else(|| VmError::MediaBlob("slash not found after finalize".into()))?;
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

    /// Phase 31/32: BLS aggregate resolve (tx 33). Verifies self-contained aggregate,
    /// then stake-weighted-casts each bitset signer. Optional until Phase 32 flip; required
    /// for multi-pin when `bls_committee_flip_from` is active.
    fn execute_resolve_slash_appeal_bls_aggregate(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        appeal_id: u64,
        approved: bool,
        signer_bitset: &[u8],
        aggregate_signature: boing_primitives::BlsSignatureG2,
        signer_bls_pubkeys: &[boing_primitives::BlsPublicKeyG1],
        height: u64,
    ) -> Result<VmExecutionResult, VmError> {
        phase32_admit_bls_aggregate_resolve_tx().map_err(|e| VmError::MediaBlob(e.to_string()))?;
        if !PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED {
            return Err(VmError::MediaBlob(
                "ResolveSlashAppealBlsAggregate (33) consensus admit not wired; use ResolveSlashAppealAttested (32)"
                    .into(),
            ));
        }
        state
            .slash_appeals_mut()
            .gate_resolve_window(appeal_id, height)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        let (slash_id, pin_set_commitment, electorate_w_snapshot) = state
            .slash_appeals()
            .get_appeal(appeal_id)
            .filter(|a| a.status == boing_state::ConsensusAppealStatus::Pending)
            .map(|a| (a.slash_id, a.pin_set_commitment, a.electorate_w_snapshot))
            .ok_or_else(|| {
                VmError::MediaBlob(
                    "appeal not found or not pending for BLS aggregate resolve".into(),
                )
            })?;

        let pinned: Vec<AccountId> = state
            .slash_appeal_resolvers()
            .map(|s| s.to_vec())
            .unwrap_or_default();
        if pinned.is_empty() {
            return Err(VmError::MediaBlob(
                "empty slash_appeal_resolvers pin set — cannot execute BLS aggregate resolve"
                    .into(),
            ));
        }
        let current_pin_commit = phase20_pin_set_commitment(&pinned);
        if !phase20_is_legacy_electorate_snapshot(electorate_w_snapshot, &pin_set_commitment)
            && current_pin_commit != pin_set_commitment
        {
            return Err(VmError::MediaBlob(
                "slash appeal pin set changed since submit".into(),
            ));
        }

        let chain_id = self
            .registration_chain_id
            .or_else(default_signing_chain_id)
            .unwrap_or(0);
        let payload = BlsAggregateResolveSelfContainedPayload {
            appeal_id,
            approved,
            signer_bitset: signer_bitset.to_vec(),
            aggregate_signature,
            signer_bls_pubkeys: signer_bls_pubkeys.to_vec(),
        };
        phase32_verify_aggregate_resolve_self_contained(
            chain_id,
            slash_id,
            &pin_set_commitment,
            &pinned,
            &payload,
        )
        .map_err(|e| VmError::MediaBlob(format!("invalid BLS aggregate resolve: {e}")))?;

        let signers = phase32_decode_signer_bitset(&pinned, signer_bitset)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        let electorate_w =
            if phase20_is_legacy_electorate_snapshot(electorate_w_snapshot, &pin_set_commitment) {
                let mut w = 0u128;
                for id in &pinned {
                    w = w.saturating_add(state.get(id).map(|a| a.stake).unwrap_or(0));
                }
                w
            } else {
                electorate_w_snapshot
            };

        let mut batch: Vec<(
            AccountId,
            bool,
            u128,
            Option<boing_primitives::SlashAppealBallotSignature>,
        )> = Vec::with_capacity(signers.len());
        for voter in &signers {
            let voter_w = state.get(voter).map(|a| a.stake).unwrap_or(0);
            batch.push((*voter, approved, voter_w, None));
        }

        let outcome = state
            .slash_appeals_mut()
            .cast_resolve_votes_batch(appeal_id, &batch, electorate_w)
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;

        if matches!(
            outcome,
            boing_state::CastResolveOutcome::Finalized { approved: true }
        ) {
            let slash = state
                .slash_appeals()
                .get_slash(slash_id)
                .cloned()
                .ok_or_else(|| VmError::MediaBlob("slash not found after finalize".into()))?;
            let _ = match slash.reason {
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
            if exec_ctx.vm_hardening {
                qa_check_init_runtime(
                    &runtime,
                    purpose,
                    desc_hash,
                    asset_name,
                    asset_symbol,
                    self.qa_at(exec_ctx),
                )?;
            }
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
        let reward_each = self
            .qa_pool_config
            .reward_per_counted_vote
            .saturating_mul(unit);
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
                // Charge init-code gas to the voter's tx (issue #61 item 26); previously discarded.
                let init_gas = self.install_contract_from_deploy_tx(state, &rec.tx, exec_ctx)?;
                Ok(VmExecutionResult {
                    gas_used: GAS_PER_QA_POOL_VOTE.saturating_add(init_gas),
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
    /// Returns gas used by init-code execution (0 when the deploy has no init section).
    fn install_contract_from_deploy_tx(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        exec_ctx: VmExecutionContext,
    ) -> Result<u64, VmError> {
        let Some((bytecode, purpose, desc_hash, asset_name, asset_symbol)) =
            tx.payload.as_contract_deploy()
        else {
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
        let (stored_code, init_gas) = if uses_init {
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
            if exec_ctx.vm_hardening {
                qa_check_init_runtime(
                    &runtime,
                    purpose,
                    desc_hash,
                    asset_name,
                    asset_symbol,
                    self.qa_at(exec_ctx),
                )?;
            }
            (runtime, g_init)
        } else {
            (bytecode.to_vec(), 0)
        };
        state.set_contract_code(contract_addr, stored_code);
        Ok(init_gas)
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
        // Phase 15: allocate from the shared consensus slash-id allocator, then pin onto the
        // challenge so FailedServe ids cannot collide with equivocation/liveness rows.
        let slash_id = state
            .slash_appeals_mut()
            .record_slash(
                c.provider,
                c.provider_penalty,
                boing_state::ConsensusSlashReason::BlobFailedServe,
                height,
                PHASE14_FAILED_SERVE_APPEAL_WINDOW_BLOCKS,
            )
            .map_err(|e| VmError::MediaBlob(e.to_string()))?;
        let _ = state.sample_serve_mut().set_failed_serve_slash_id(
            &c.blob_id,
            c.chunk_index,
            &c.provider,
            &c.challenger,
            slash_id,
        );
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

/// Housekeeping: settle FailedServe + prune stale settle receipts + expire overdue
/// appeals + prune Expired / ancient Approved·Rejected appeals + prune terminal
/// serve challenges past retention.
pub fn apply_media_blob_block_housekeeping(
    state: &mut StateStore,
    height: u64,
) -> Result<usize, VmError> {
    let settled = settle_expired_serve_challenges(state, height)?;
    let _ = state.sample_offers_mut().prune_settle_receipts(height);
    // Phase 21: stuck Pending (W=0 / incomplete tallies) → Expired; no restore.
    let _ = state.slash_appeals_mut().expire_pending_appeals(height);
    // Phase 23: drop Expired past resolve_deadline + 4096; slash rows stay.
    let _ = state.slash_appeals_mut().prune_expired_appeals(height);
    // Phase 25: drop Approved / Rejected past resolve_deadline + 4096; slash rows stay.
    let _ = state.slash_appeals_mut().prune_resolved_appeals(height);
    // Phase 24: drop Responded / settled FailedServe past deadline + 4096.
    let _ = state.sample_serve_mut().prune_terminal_challenges(height);
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

    fn init_deploy_tx(sender: AccountId, init: Vec<u8>) -> Transaction {
        let mut bytecode = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
        bytecode.extend(init);
        Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        }
    }

    fn funded_sender(state: &mut StateStore) -> AccountId {
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId(key.verifying_key().to_bytes());
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 1_000_000,
                ..Default::default()
            },
        });
        sender
    }

    /// Build init bytecode that `RETURN`s exactly `runtime` (≤32 bytes) via a single `MSTORE`.
    fn init_returning(runtime: &[u8]) -> Vec<u8> {
        assert!(!runtime.is_empty() && runtime.len() <= 32);
        let mut word = [0u8; 32];
        word[..runtime.len()].copy_from_slice(runtime);
        let mut init = vec![0x7f]; // PUSH32
        init.extend_from_slice(&word);
        init.extend([0x60, 0x00, 0x52]); // PUSH1 0; MSTORE
        init.extend([0x60, runtime.len() as u8, 0x60, 0x00, 0xf3]); // RETURN size, offset 0
        init
    }

    /// Audit #61 item 3: the runtime returned by init code is QA-checked from the VM hardening
    /// height (empty runtime, runtime carrying the init marker); before it, stored as-is.
    #[test]
    fn init_code_runtime_qa_is_gated() {
        // RETURN offset 0 size 0 → empty runtime.
        let empty = vec![0x60, 0x00, 0x60, 0x00, 0xf3];
        // MSTORE marker in byte 31, RETURN offset 31 size 1 → runtime = [marker].
        let marker = vec![
            0x60,
            CONTRACT_DEPLOY_INIT_CODE_MARKER,
            0x60,
            0x00,
            0x52,
            0x60,
            0x01,
            0x60,
            0x1f,
            0xf3,
        ];
        for init in [empty, marker] {
            // Gate active (from genesis).
            let vm = Vm::new();
            let mut state = StateStore::new();
            let sender = funded_sender(&mut state);
            let r = vm.execute(&init_deploy_tx(sender, init.clone()), &mut state);
            assert!(matches!(r, Err(VmError::QaRejected { .. })), "{r:?}");
            assert!(state
                .get_contract_code(&nonce_derived_contract_address(&sender, 0))
                .is_none());

            // Before the gate: historical behaviour, runtime stored unchecked.
            let mut vm = Vm::new();
            vm.set_vm_hardening_from(u64::MAX);
            let mut state = StateStore::new();
            let sender = funded_sender(&mut state);
            vm.execute(&init_deploy_tx(sender, init.clone()), &mut state)
                .unwrap();
            assert!(state
                .get_contract_code(&nonce_derived_contract_address(&sender, 0))
                .is_some());
        }
    }

    /// Audit #61 item 3 (exact finding): a well-formed init stub can RETURN runtime that would be
    /// Rejected if deployed directly (invalid opcode / blocklisted hash). With hardening the deploy
    /// fails; without it the bad runtime is stored. Covers top-level deploy and in-contract CREATE2.
    #[test]
    fn init_code_cannot_smuggle_rejected_runtime() {
        use crate::bytecode::Opcode;
        use boing_primitives::hasher;

        let bad_opcode_runtime = vec![0xff];
        // STOP is normally Allow; blocklisting its hash models a known-bad runtime whose bytes do
        // not appear as a contiguous scam pattern inside the init PUSH immediate (which the outer
        // deploy QA would already catch).
        let blocklisted_runtime = vec![0x00];
        let mut h = hasher();
        h.update(&blocklisted_runtime);
        let blocked_hash = *h.finalize().as_bytes();
        let block_reg = RuleRegistry::new().with_blocklist(vec![blocked_hash]);

        assert!(matches!(
            check_contract_deploy_full(
                &bad_opcode_runtime,
                Some("dapp"),
                None,
                &RuleRegistry::new()
            ),
            QaResult::Reject(_)
        ));
        assert!(matches!(
            check_contract_deploy_full(&blocklisted_runtime, Some("dapp"), None, &block_reg),
            QaResult::Reject(_)
        ));

        for (runtime, registry) in [
            (bad_opcode_runtime.as_slice(), RuleRegistry::new()),
            (blocklisted_runtime.as_slice(), block_reg.clone()),
        ] {
            let init = init_returning(runtime);
            let mut deploy_probe = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
            deploy_probe.extend_from_slice(&init);
            let probe = check_contract_deploy_full(&deploy_probe, Some("dapp"), None, &registry);
            assert!(
                matches!(probe, QaResult::Allow),
                "init stub alone must pass QA so the bypass is real: {probe:?}"
            );

            // Hardening on → Reject, no code installed.
            let vm = Vm::with_qa_registry(registry.clone());
            let mut state = StateStore::new();
            let sender = funded_sender(&mut state);
            let r = vm.execute(&init_deploy_tx(sender, init.clone()), &mut state);
            assert!(
                matches!(
                    &r,
                    Err(VmError::QaRejected { message, .. }) if message.contains("init-code runtime")
                ),
                "{r:?}"
            );
            assert!(state
                .get_contract_code(&nonce_derived_contract_address(&sender, 0))
                .is_none());

            // Hardening off → historical bypass: bad runtime stored.
            let mut vm = Vm::with_qa_registry(registry.clone());
            vm.set_vm_hardening_from(u64::MAX);
            let mut state = StateStore::new();
            let sender = funded_sender(&mut state);
            vm.execute(&init_deploy_tx(sender, init.clone()), &mut state)
                .unwrap();
            assert_eq!(
                state
                    .get_contract_code(&nonce_derived_contract_address(&sender, 0))
                    .unwrap()
                    .as_slice(),
                runtime
            );
        }

        // CREATE2 path: factory deploys a compact `0xFD || init` that RETURNs `[0xff]`.
        // Short init (fits in one MSTORE word with the marker): PUSH1 0xff; PUSH1 0; MSTORE;
        // RETURN offset 31 size 1.
        let init = vec![0x60, 0xff, 0x60, 0x00, 0x52, 0x60, 0x01, 0x60, 0x1f, 0xf3];
        let mut deploy_bytes = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
        deploy_bytes.extend_from_slice(&init);
        let salt = [7u8; 32];

        let mut factory = Vec::new();
        assert!(deploy_bytes.len() <= 32);
        let mut word = [0u8; 32];
        word[..deploy_bytes.len()].copy_from_slice(&deploy_bytes);
        factory.push(Opcode::Push32 as u8);
        factory.extend_from_slice(&word);
        factory.extend([Opcode::Push1 as u8, 0x00, Opcode::MStore as u8]);
        factory.extend([
            Opcode::Push1 as u8,
            0x00, // offset
            Opcode::Push1 as u8,
            deploy_bytes.len() as u8, // size
        ]);
        factory.push(Opcode::Push32 as u8);
        factory.extend_from_slice(&salt);
        factory.extend([Opcode::Create2 as u8, Opcode::Stop as u8]);

        let vm = Vm::new(); // hardening from genesis
        let mut state = StateStore::new();
        let sender = funded_sender(&mut state);
        vm.execute(
            &Transaction {
                nonce: 0,
                sender,
                payload: TransactionPayload::ContractDeploy {
                    bytecode: factory.clone(),
                    create2_salt: None,
                },
                access_list: AccessList::default(),
            },
            &mut state,
        )
        .unwrap();
        let factory_addr = nonce_derived_contract_address(&sender, 0);
        let call = Transaction {
            nonce: 1,
            sender,
            payload: TransactionPayload::ContractCall {
                contract: factory_addr,
                calldata: Vec::new(),
            },
            access_list: AccessList::default(),
        };
        let r = vm.execute(&call, &mut state);
        assert!(
            matches!(
                &r,
                Err(VmError::QaRejected { message, .. }) if message.contains("init-code runtime")
            ),
            "CREATE2 must QA-check returned runtime: {r:?}"
        );
        let expected_child = create2_contract_address(&factory_addr, &salt, &deploy_bytes);
        assert!(
            state.get_contract_code(&expected_child).is_none(),
            "smuggled CREATE2 child must not be installed"
        );

        // Without hardening, the same CREATE2 installs the bad runtime.
        let mut vm = Vm::new();
        vm.set_vm_hardening_from(u64::MAX);
        let mut state = StateStore::new();
        let sender = funded_sender(&mut state);
        vm.execute(
            &Transaction {
                nonce: 0,
                sender,
                payload: TransactionPayload::ContractDeploy {
                    bytecode: factory,
                    create2_salt: None,
                },
                access_list: AccessList::default(),
            },
            &mut state,
        )
        .unwrap();
        let factory_addr = nonce_derived_contract_address(&sender, 0);
        vm.execute(
            &Transaction {
                nonce: 1,
                sender,
                payload: TransactionPayload::ContractCall {
                    contract: factory_addr,
                    calldata: Vec::new(),
                },
                access_list: AccessList::default(),
            },
            &mut state,
        )
        .unwrap();
        let expected_child = create2_contract_address(&factory_addr, &salt, &deploy_bytes);
        assert_eq!(
            state.get_contract_code(&expected_child).unwrap().as_slice(),
            bad_opcode_runtime.as_slice()
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

/// Hard cap on one VM frame's linear memory. Memory growth is not separately metered, so without a
/// cap a tiny contract (`PUSH <2^40> MSTORE`) makes the node try to allocate terabytes (abort) or hit
/// `Vec` capacity-overflow panics. Real contracts use kilobytes; calldata/return buffers are already
/// bounded far below this.
pub const MAX_VM_MEMORY_BYTES: usize = 8 * 1024 * 1024;

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
    #[error(
        "VM stack depth exceeded (max {} items)",
        crate::interpreter::MAX_STACK_DEPTH
    )]
    StackOverflow,
    #[error("VM memory limit exceeded (max {MAX_VM_MEMORY_BYTES} bytes per frame)")]
    MemoryLimitExceeded,
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
/// Audit #61 item 3 (from the VM hardening height): the runtime bytecode an init program returns
/// is what gets stored and executed, so it must pass the same deploy QA as a direct deploy. It may
/// not itself carry the init-code marker (that would smuggle an unchecked body past QA again).
/// `Unsure` is not escalated here: the deploy as a whole already went through the Unsure path.
fn qa_check_init_runtime(
    runtime: &[u8],
    purpose: Option<&str>,
    desc_hash: Option<&[u8]>,
    asset_name: Option<&str>,
    asset_symbol: Option<&str>,
    registry: &RuleRegistry,
) -> Result<(), VmError> {
    if contract_deploy_uses_init_code(runtime) {
        return Err(VmError::QaRejected {
            rule_id: boing_qa::RuleId::MALFORMED_BYTECODE.to_string(),
            message:
                "runtime bytecode returned by init code must not start with the init-code marker"
                    .to_string(),
        });
    }
    match check_contract_deploy_full_with_metadata(
        runtime,
        purpose,
        desc_hash,
        asset_name,
        asset_symbol,
        registry,
    ) {
        QaResult::Reject(r) => Err(VmError::QaRejected {
            rule_id: r.rule_id.0,
            message: format!("init-code runtime: {}", r.message),
        }),
        QaResult::Allow | QaResult::Unsure => Ok(()),
    }
}

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
        // In-tx CREATE2 has no QA-pool path; Unsure must not silently install (issue #61 item 27).
        QaResult::Unsure => {
            return Err(VmError::QaRejected {
                rule_id: boing_qa::RuleId::SOFT_RULE_FAILED.to_string(),
                message: "in-tx CREATE2 QA Unsure: deploy via a top-level ContractDeploy for community review"
                    .to_string(),
            });
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
        if exec_ctx.vm_hardening {
            qa_check_init_runtime(&runtime, Some("dapp"), None, None, None, qa_registry)?;
        }
        (runtime, g)
    } else {
        (bytecode, 0)
    };

    state.set_contract_code(contract_addr, stored_code);
    Ok((contract_addr, init_gas))
}

#[cfg(test)]
mod create2_qa_unsure_tests {
    use super::*;
    use boing_qa::RuleRegistry;

    #[test]
    fn in_tx_create2_rejects_qa_unsure() {
        // Purpose "dapp" is always-review → Unsure; CREATE2 must not proceed (issue #61 item 27).
        let reg = RuleRegistry::new().with_always_review_categories(vec!["dapp".to_string()]);
        let mut state = StateStore::new();
        let deployer = AccountId([3u8; 32]);
        let mut logs = Vec::new();
        let err = apply_in_tx_create2(
            &mut state,
            &reg,
            deployer,
            [1u8; 32],
            vec![0x00], // minimal STOP bytecode
            GAS_PER_CONTRACT_DEPLOY_INIT,
            &mut logs,
            0,
            VmExecutionContext {
                block_height: 1,
                block_timestamp: 1,
                native_value: false,
                vm_hardening: true,
            },
        )
        .expect_err("Unsure CREATE2 must fail");
        match err {
            VmError::QaRejected { message, .. } => {
                assert!(
                    message.contains("CREATE2 QA Unsure"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected QaRejected, got {other}"),
        }
        assert!(
            state
                .get_contract_code(&create2_contract_address(&deployer, &[1u8; 32], &[0x00]))
                .is_none(),
            "no code installed on Unsure"
        );
    }
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
        assert!(
            vm.registration_proof_ok(5, &owner, &node_id, &v1),
            "no chain id: v1"
        );
        assert!(!vm.registration_proof_ok(5, &owner, &node_id, &v2));
        vm.set_registration_proof_rule(Some(6913), 10);
        assert!(
            vm.registration_proof_ok(9, &owner, &node_id, &v1),
            "v1 before activation"
        );
        assert!(
            vm.registration_proof_ok(9, &owner, &node_id, &v2),
            "v2 before activation"
        );
        assert!(!vm.registration_proof_ok(9, &owner, &node_id, &other));
        assert!(
            !vm.registration_proof_ok(10, &owner, &node_id, &v1),
            "v1 rejected from activation"
        );
        assert!(vm.registration_proof_ok(10, &owner, &node_id, &v2));
        assert!(
            !vm.registration_proof_ok(10, &owner, &node_id, &other),
            "other chain rejected"
        );
        assert!(
            !vm.registration_proof_ok(10, &AccountId([8u8; 32]), &node_id, &v2),
            "bound to owner"
        );
    }
}

#[cfg(test)]
mod slash_appeal_phase14_tests {
    use super::*;
    use boing_primitives::{AccessList, Account, PHASE14_MAX_APPEAL_EVIDENCE_BYTES};
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
            vm_hardening: false,
        }
    }

    #[test]
    fn submit_resolve_approved_restores_failed_serve_balance() {
        let provider = AccountId([2u8; 32]);
        let validator = AccountId([3u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 50, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
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
        vm.execute_with_context(&submit, &mut state, ctx(50))
            .unwrap();
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
        vm.execute_with_context(&resolve, &mut state, ctx(51))
            .unwrap();
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
        fund(&mut state, validator, 1_000_000, 1_000, 0);
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
        vm.execute_with_context(&submit, &mut state, ctx(2))
            .unwrap();
        let resolve = Transaction {
            nonce: 0,
            sender: validator,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id: 1,
                approved: false,
            },
            access_list: AccessList::new(vec![validator], vec![validator]),
        };
        vm.execute_with_context(&resolve, &mut state, ctx(3))
            .unwrap();
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
        fund(&mut state, validator, 1_000_000, 1_000, 0);
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
        assert!(vm
            .execute_with_context(&oversize, &mut state, ctx(2))
            .is_err());

        let submit = Transaction {
            nonce: 0,
            sender: provider,
            payload: TransactionPayload::SubmitSlashAppeal {
                slash_id: 1,
                evidence: vec![],
            },
            access_list: AccessList::new(vec![provider], vec![provider]),
        };
        vm.execute_with_context(&submit, &mut state, ctx(2))
            .unwrap();

        let bad = Transaction {
            nonce: 0,
            sender: outsider,
            payload: TransactionPayload::ResolveSlashAppeal {
                appeal_id: 1,
                approved: true,
            },
            access_list: AccessList::new(vec![outsider], vec![outsider]),
        };
        let err = vm
            .execute_with_context(&bad, &mut state, ctx(3))
            .unwrap_err();
        assert!(format!("{err}").contains("consensus validator"));
    }

    #[test]
    fn double_resolve_rejected() {
        let provider = AccountId([9u8; 32]);
        let validator = AccountId([10u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 0, 0, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
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
        vm.execute_with_context(&resolve, &mut state, ctx(3))
            .unwrap();
        let mut resolve2 = resolve.clone();
        resolve2.nonce = 1;
        assert!(vm
            .execute_with_context(&resolve2, &mut state, ctx(4))
            .is_err());
    }

    #[test]
    fn equivocation_appeal_restores_stake_when_media_inactive() {
        let offender = AccountId([11u8; 32]);
        let validator = AccountId([12u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, offender, 100, 10_000, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
        // Burn sink already holds the slashed stake.
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 5_000, 0, 0);
        state.get_mut(&offender).unwrap().stake = 5_000;
        let slash_id = state
            .slash_appeals_mut()
            .record_slash(
                offender,
                5_000,
                ConsensusSlashReason::Equivocation,
                10,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![validator]));

        // Media blobs inactive — Phase 15 always-on for 30/31.
        let vm = Vm::new();
        assert!(vm.media_blobs_from().is_none());
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: offender,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id,
                    evidence: b"ok".to_vec(),
                },
                access_list: AccessList::new(vec![offender], vec![offender]),
            },
            &mut state,
            ctx(50),
        )
        .unwrap();
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: validator,
                payload: TransactionPayload::ResolveSlashAppeal {
                    appeal_id: 1,
                    approved: true,
                },
                access_list: AccessList::new(vec![validator], vec![validator]),
            },
            &mut state,
            ctx(51),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(slash_id));
        assert_eq!(state.get(&offender).unwrap().stake, 10_000);
        assert_eq!(
            state.get(&boing_tokenomics::FEE_BURN_SINK).unwrap().balance,
            0
        );
    }

    #[test]
    fn phase17_equal_stakes_needs_two_approves_before_restore() {
        // Equal stakes [100,100,100] → W=300; need 200 (two votes).
        // Phase 19: multi-pin casts use attested 32 (31 rejected when |pinned|>1).
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        let provider = AccountId([20u8; 32]);
        let key1 = SigningKey::from_bytes(&[21u8; 32]);
        let key2 = SigningKey::from_bytes(&[22u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId(key2.verifying_key().to_bytes());
        let v3 = AccountId([23u8; 32]);
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                42,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 42,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();

        // First approve: still Pending, no restore.
        let sig1 = sign_slash_appeal_resolve_ballot(chain_id, 1, 42, true, &key1);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v1,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig1,
                },
                access_list: AccessList::new(vec![v1], vec![v1]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(!state.slash_appeals().is_reversed(42));
        assert_eq!(state.get(&provider).unwrap().balance, 100);
        assert_eq!(
            state.slash_appeals().get_appeal(1).unwrap().status,
            boing_state::ConsensusAppealStatus::Pending
        );

        // Second approve: Finalized + restore.
        let sig2 = sign_slash_appeal_resolve_ballot(chain_id, 1, 42, true, &key2);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v2,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig2,
                },
                access_list: AccessList::new(vec![v2], vec![v2]),
            },
            &mut state,
            ctx(4),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(42));
        assert_eq!(state.get(&provider).unwrap().balance, 1_100);
    }

    #[test]
    fn phase17_whale_one_shot_finalize() {
        // Stakes [900,50,50] → W=1000; whale alone finalizes.
        // Phase 19: multi-pin casts use attested 32 (31 rejected when |pinned|>1).
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        let provider = AccountId([30u8; 32]);
        let whale_key = SigningKey::from_bytes(&[31u8; 32]);
        let whale = AccountId(whale_key.verifying_key().to_bytes());
        let small1 = AccountId([32u8; 32]);
        let small2 = AccountId([33u8; 32]);
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, whale, 1_000_000, 900, 0);
        fund(&mut state, small1, 1_000_000, 50, 0);
        fund(&mut state, small2, 1_000_000, 50, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                99,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![whale, small1, small2]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 99,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        let sig = sign_slash_appeal_resolve_ballot(chain_id, 1, 99, true, &whale_key);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: whale,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig,
                },
                access_list: AccessList::new(vec![whale], vec![whale]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(99));
        assert_eq!(state.get(&provider).unwrap().balance, 1_100);
    }

    #[test]
    fn phase18_attested_threshold_finalize_and_bad_sig_reject() {
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        let provider = AccountId([40u8; 32]);
        let key1 = SigningKey::from_bytes(&[41u8; 32]);
        let key2 = SigningKey::from_bytes(&[42u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId(key2.verifying_key().to_bytes());
        let v3 = AccountId([43u8; 32]);
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                55,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));

        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 55,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();

        // Bad ballot rejected.
        let bad = sign_slash_appeal_resolve_ballot(chain_id, 1, 55, false, &key1);
        let err = vm
            .execute_with_context(
                &Transaction {
                    nonce: 0,
                    sender: v1,
                    payload: TransactionPayload::ResolveSlashAppealAttested {
                        appeal_id: 1,
                        approved: true,
                        ballot_signature: bad, // wrong `approved` binding
                    },
                    access_list: AccessList::new(vec![v1], vec![v1]),
                },
                &mut state,
                ctx(3),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("invalid resolve ballot")
                || err.to_string().contains("ballot"),
            "got {err}"
        );
        assert!(state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .votes
            .is_empty());

        // Two valid attested approves → finalize.
        let sig1 = sign_slash_appeal_resolve_ballot(chain_id, 1, 55, true, &key1);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v1,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig1,
                },
                access_list: AccessList::new(vec![v1], vec![v1]),
            },
            &mut state,
            ctx(4),
        )
        .unwrap();
        assert!(!state.slash_appeals().is_reversed(55));
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .votes
                .get(&v1)
                .unwrap()
                .ballot_signature,
            Some(sig1)
        );

        let sig2 = sign_slash_appeal_resolve_ballot(chain_id, 1, 55, true, &key2);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v2,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig2,
                },
                access_list: AccessList::new(vec![v2], vec![v2]),
            },
            &mut state,
            ctx(5),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(55));
        assert_eq!(state.get(&provider).unwrap().balance, 1_100);
    }

    #[test]
    fn phase31_bls_aggregate_33_finalizes_and_keeps_32_path() {
        use boing_primitives::{
            phase27_aggregate_bls_signatures, phase27_bls_pubkey_from_ikm, phase27_bls_sign,
            phase31_bls_aggregate_ballot_preimage, phase31_encode_signer_bitset,
            phase31_pin_set_commitment, PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED,
            PHASE31_BLS_LIVE_TX32_STILL_ED25519,
        };

        assert!(PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED);
        assert!(PHASE31_BLS_LIVE_TX32_STILL_ED25519);

        let provider = AccountId([0x61; 32]);
        let a1 = AccountId([0x62; 32]);
        let a2 = AccountId([0x63; 32]);
        let a3 = AccountId([0x64; 32]);
        let chain_id = 6913u64;
        let ikm1 = [0x71u8; 32];
        let ikm2 = [0x72u8; 32];
        let pk1 = phase27_bls_pubkey_from_ikm(&ikm1).unwrap();
        let pk2 = phase27_bls_pubkey_from_ikm(&ikm2).unwrap();

        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, a1, 1_000_000, 100, 0);
        fund(&mut state, a2, 1_000_000, 100, 0);
        fund(&mut state, a3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                610,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![a1, a2, a3]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 610,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();

        let pinned = [a1, a2, a3];
        let pin_commit = phase31_pin_set_commitment(&pinned);
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .pin_set_commitment,
            pin_commit
        );
        let msg = phase31_bls_aggregate_ballot_preimage(chain_id, 1, 610, true, &pin_commit);
        let s1 = phase27_bls_sign(&ikm1, &msg).unwrap();
        let s2 = phase27_bls_sign(&ikm2, &msg).unwrap();
        let agg = phase27_aggregate_bls_signatures(&[s1, s2]).unwrap();
        let bits = phase31_encode_signer_bitset(&pinned, &[a1, a2]).unwrap();

        // Submitter a1 (validator committee member) relays the aggregate.
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: a1,
                payload: TransactionPayload::ResolveSlashAppealBlsAggregate {
                    appeal_id: 1,
                    approved: true,
                    signer_bitset: bits,
                    aggregate_signature: agg,
                    signer_bls_pubkeys: vec![pk1, pk2],
                },
                access_list: AccessList::new(vec![a1], vec![a1]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(610));
        assert_eq!(state.get(&provider).unwrap().balance, 1_100);
        assert_eq!(
            state.slash_appeals().get_appeal(1).unwrap().votes.len(),
            2,
            "both bitset signers recorded"
        );
    }

    #[test]
    fn phase18_unattested_31_still_works() {
        let provider = AccountId([50u8; 32]);
        let validator = AccountId([51u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                77,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
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
                    slash_id: 77,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: validator,
                payload: TransactionPayload::ResolveSlashAppeal {
                    appeal_id: 1,
                    approved: true,
                },
                access_list: AccessList::new(vec![validator], vec![validator]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(77));
        let v = state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .votes
            .get(&validator)
            .unwrap();
        assert!(v.ballot_signature.is_none());
    }

    #[test]
    fn phase19_multi_pin_31_rejected() {
        let provider = AccountId([60u8; 32]);
        let v1 = AccountId([61u8; 32]);
        let v2 = AccountId([62u8; 32]);
        let v3 = AccountId([63u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                88,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 88,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        let err = vm
            .execute_with_context(
                &Transaction {
                    nonce: 0,
                    sender: v1,
                    payload: TransactionPayload::ResolveSlashAppeal {
                        appeal_id: 1,
                        approved: true,
                    },
                    access_list: AccessList::new(vec![v1], vec![v1]),
                },
                &mut state,
                ctx(3),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("ResolveSlashAppealAttested")
                || err.to_string().contains("|slash_appeal_resolvers| > 1"),
            "got {err}"
        );
        assert!(state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .votes
            .is_empty());
        assert!(!state.slash_appeals().is_reversed(88));
    }

    #[test]
    fn phase19_multi_pin_32_threshold_finalize() {
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        let provider = AccountId([70u8; 32]);
        let key1 = SigningKey::from_bytes(&[71u8; 32]);
        let key2 = SigningKey::from_bytes(&[72u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId(key2.verifying_key().to_bytes());
        let v3 = AccountId([73u8; 32]);
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                89,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));

        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 89,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();

        let sig1 = sign_slash_appeal_resolve_ballot(chain_id, 1, 89, true, &key1);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v1,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig1,
                },
                access_list: AccessList::new(vec![v1], vec![v1]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(!state.slash_appeals().is_reversed(89));

        let sig2 = sign_slash_appeal_resolve_ballot(chain_id, 1, 89, true, &key2);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v2,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig2,
                },
                access_list: AccessList::new(vec![v2], vec![v2]),
            },
            &mut state,
            ctx(4),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(89));
        assert_eq!(state.get(&provider).unwrap().balance, 1_100);
    }

    #[test]
    fn phase19_solo_pinned_one_31_still_works() {
        // |pinned|=1 keeps unattested 31 (Phase 19 committee rule is len > 1 only).
        let provider = AccountId([80u8; 32]);
        let validator = AccountId([81u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                90,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
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
                    slash_id: 90,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: validator,
                payload: TransactionPayload::ResolveSlashAppeal {
                    appeal_id: 1,
                    approved: true,
                },
                access_list: AccessList::new(vec![validator], vec![validator]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(90));
    }

    #[test]
    fn phase19_mid_tally_mixed_none_and_some_finalizes() {
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        // Simulate pre-upgrade unattested vote (ballot=None) retained mid-tally, then
        // new multi-pin casts via 32 still finalize without stripping historical weight.
        let provider = AccountId([85u8; 32]);
        let key1 = SigningKey::from_bytes(&[86u8; 32]);
        let key2 = SigningKey::from_bytes(&[87u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId(key2.verifying_key().to_bytes());
        let v3 = AccountId([88u8; 32]);
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                91,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));

        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 91,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();

        // Historical unattested cast (pre-Phase-19 / direct registry) — keep weight.
        state
            .slash_appeals_mut()
            .cast_resolve_vote(1, v1, true, 100, 300, None)
            .unwrap();
        assert!(state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .votes
            .get(&v1)
            .unwrap()
            .ballot_signature
            .is_none());
        assert!(!state.slash_appeals().is_reversed(91));

        // New multi-pin cast must be 32.
        let sig2 = sign_slash_appeal_resolve_ballot(chain_id, 1, 91, true, &key2);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v2,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig2,
                },
                access_list: AccessList::new(vec![v2], vec![v2]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(91));
        assert_eq!(state.get(&provider).unwrap().balance, 1_100);
        // Historical unattested weight still present on the finalized appeal.
        assert!(state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .votes
            .get(&v1)
            .unwrap()
            .ballot_signature
            .is_none());
    }

    #[test]
    fn phase20_submit_snapshots_w300_and_frozen_after_restake() {
        // Pin 100/100/100 → Submit stores W=300. After Submit, inflate one resolver's
        // stake so live W=900 (two×100 would fail live ⌈2×900/3⌉) but still finalize
        // against frozen ⌈2×300/3⌉=200.
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        let provider = AccountId([90u8; 32]);
        let key1 = SigningKey::from_bytes(&[91u8; 32]);
        let key2 = SigningKey::from_bytes(&[92u8; 32]);
        let key3 = SigningKey::from_bytes(&[93u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId(key2.verifying_key().to_bytes());
        let v3 = AccountId(key3.verifying_key().to_bytes());
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                200,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));

        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 200,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        let appeal = state.slash_appeals().get_appeal(1).unwrap();
        assert_eq!(appeal.electorate_w_snapshot, 300);
        assert_ne!(appeal.pin_set_commitment, [0u8; 32]);

        // Inflate v3 after Submit — live W would be 900; frozen snapshot stays 300.
        // Also unbond is covered: live restake must not change the denominator.
        state.get_mut(&v3).unwrap().stake = 700;
        assert!(!boing_primitives::phase20_resolve_threshold_met(200, 900));
        assert!(boing_primitives::phase20_resolve_threshold_met(200, 300));

        let sig1 = sign_slash_appeal_resolve_ballot(chain_id, 1, 200, true, &key1);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v1,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig1,
                },
                access_list: AccessList::new(vec![v1], vec![v1]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(!state.slash_appeals().is_reversed(200));

        let sig2 = sign_slash_appeal_resolve_ballot(chain_id, 1, 200, true, &key2);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v2,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig2,
                },
                access_list: AccessList::new(vec![v2], vec![v2]),
            },
            &mut state,
            ctx(4),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(200));
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .electorate_w_snapshot,
            300
        );
    }

    #[test]
    fn phase20_pin_set_change_rejects_resolve() {
        let provider = AccountId([94u8; 32]);
        let validator = AccountId([95u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                201,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(None);
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 201,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .electorate_w_snapshot,
            1
        );
        // Pin set changed vs Submit unpinned commitment → reject.
        state.set_slash_appeal_resolvers(Some(vec![validator]));
        let err = vm
            .execute_with_context(
                &Transaction {
                    nonce: 0,
                    sender: validator,
                    payload: TransactionPayload::ResolveSlashAppeal {
                        appeal_id: 1,
                        approved: true,
                    },
                    access_list: AccessList::new(vec![validator], vec![validator]),
                },
                &mut state,
                ctx(3),
            )
            .unwrap_err();
        assert!(err.to_string().contains("pin set changed"), "got {err}");
    }

    #[test]
    fn phase20_unpinned_submit_resolve_one_shot() {
        let provider = AccountId([96u8; 32]);
        let validator = AccountId([97u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                202,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(None);
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 202,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .electorate_w_snapshot,
            1
        );
        // Unpinned resolve: any consensus validator (validator set membership).
        // With no pin set, sender_may_resolve typically allows bonded validators.
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: validator,
                payload: TransactionPayload::ResolveSlashAppeal {
                    appeal_id: 1,
                    approved: true,
                },
                access_list: AccessList::new(vec![validator], vec![validator]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(202));
    }

    #[test]
    fn phase20_w0_at_submit_stays_pending() {
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        let provider = AccountId([98u8; 32]);
        let key1 = SigningKey::from_bytes(&[99u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId([100u8; 32]);
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 0, 0); // zero stake
        fund(&mut state, v2, 1_000_000, 0, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                203,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                1_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 203,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .electorate_w_snapshot,
            0
        );
        // Rebond after Submit must NOT rewrite snapshot — still Pending at W=0.
        state.get_mut(&v1).unwrap().stake = 1000;
        let sig = sign_slash_appeal_resolve_ballot(chain_id, 1, 203, true, &key1);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v1,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig,
                },
                access_list: AccessList::new(vec![v1], vec![v1]),
            },
            &mut state,
            ctx(3),
        )
        .unwrap();
        assert!(!state.slash_appeals().is_reversed(203));
        assert_eq!(
            state.slash_appeals().get_appeal(1).unwrap().status,
            boing_state::ConsensusAppealStatus::Pending
        );
    }

    #[test]
    fn phase21_submit_stores_resolve_deadline_h_plus_1000() {
        let provider = AccountId([110u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                210,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                5_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(None);
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 210,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(50),
        )
        .unwrap();
        let appeal = state.slash_appeals().get_appeal(1).unwrap();
        assert_eq!(
            appeal.resolve_deadline,
            boing_primitives::phase21_resolve_deadline(50)
        );
        assert_eq!(appeal.resolve_deadline, 1050);
    }

    #[test]
    fn phase21_late_resolve_expires_and_rejects() {
        let provider = AccountId([111u8; 32]);
        let validator = AccountId([112u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, validator, 1_000_000, 1_000, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                211,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                5_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(None);
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 211,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(10),
        )
        .unwrap();
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .resolve_deadline,
            1010
        );
        let err = vm
            .execute_with_context(
                &Transaction {
                    nonce: 0,
                    sender: validator,
                    payload: TransactionPayload::ResolveSlashAppeal {
                        appeal_id: 1,
                        approved: true,
                    },
                    access_list: AccessList::new(vec![validator], vec![validator]),
                },
                &mut state,
                ctx(1011),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("resolve window closed"),
            "got {err}"
        );
        assert_eq!(
            state.slash_appeals().get_appeal(1).unwrap().status,
            boing_state::ConsensusAppealStatus::Expired
        );
        assert!(state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .votes
            .is_empty());
        assert!(!state.slash_appeals().is_reversed(211));
    }

    #[test]
    fn phase21_housekeeping_expires_stuck_pending() {
        let provider = AccountId([113u8; 32]);
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                212,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                5_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(None);
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 212,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(5),
        )
        .unwrap();
        assert_eq!(
            state.slash_appeals().get_appeal(1).unwrap().status,
            boing_state::ConsensusAppealStatus::Pending
        );
        apply_media_blob_block_housekeeping(&mut state, 1006).unwrap();
        assert_eq!(
            state.slash_appeals().get_appeal(1).unwrap().status,
            boing_state::ConsensusAppealStatus::Expired
        );
        assert!(!state.slash_appeals().is_reversed(212));
    }

    #[test]
    fn phase21_frozen_w_still_works_before_deadline() {
        // Phase 20 frozen W + Phase 19 multi-pin attested still finalize before deadline.
        use boing_primitives::sign_slash_appeal_resolve_ballot;
        use ed25519_dalek::SigningKey;

        let provider = AccountId([114u8; 32]);
        let key1 = SigningKey::from_bytes(&[115u8; 32]);
        let key2 = SigningKey::from_bytes(&[116u8; 32]);
        let key3 = SigningKey::from_bytes(&[117u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId(key2.verifying_key().to_bytes());
        let v3 = AccountId(key3.verifying_key().to_bytes());
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                213,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                5_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 213,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .electorate_w_snapshot,
            300
        );
        assert_eq!(
            state
                .slash_appeals()
                .get_appeal(1)
                .unwrap()
                .resolve_deadline,
            1002
        );
        // Unattested 31 still rejected for multi-pin (Phase 19).
        let err = vm
            .execute_with_context(
                &Transaction {
                    nonce: 0,
                    sender: v1,
                    payload: TransactionPayload::ResolveSlashAppeal {
                        appeal_id: 1,
                        approved: true,
                    },
                    access_list: AccessList::new(vec![v1], vec![v1]),
                },
                &mut state,
                ctx(3),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("ResolveSlashAppealAttested"),
            "got {err}"
        );
        let sig1 = sign_slash_appeal_resolve_ballot(chain_id, 1, 213, true, &key1);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v1,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig1,
                },
                access_list: AccessList::new(vec![v1], vec![v1]),
            },
            &mut state,
            ctx(4),
        )
        .unwrap();
        let sig2 = sign_slash_appeal_resolve_ballot(chain_id, 1, 213, true, &key2);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v2,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig2,
                },
                access_list: AccessList::new(vec![v2], vec![v2]),
            },
            &mut state,
            ctx(5),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(213));
        assert_eq!(
            state.slash_appeals().get_appeal(1).unwrap().status,
            boing_state::ConsensusAppealStatus::Approved
        );
    }

    #[test]
    fn phase22_v2_cast_ok_wrong_pin_rejected_v1_dual_ok() {
        use boing_primitives::{
            sign_slash_appeal_resolve_ballot_v1, sign_slash_appeal_resolve_ballot_v2,
        };
        use ed25519_dalek::SigningKey;

        let provider = AccountId([120u8; 32]);
        let key1 = SigningKey::from_bytes(&[121u8; 32]);
        let key2 = SigningKey::from_bytes(&[122u8; 32]);
        let key3 = SigningKey::from_bytes(&[123u8; 32]);
        let v1 = AccountId(key1.verifying_key().to_bytes());
        let v2 = AccountId(key2.verifying_key().to_bytes());
        let v3 = AccountId(key3.verifying_key().to_bytes());
        let chain_id = 6913u64;
        let mut state = StateStore::new();
        fund(&mut state, provider, 100, 0, 0);
        fund(&mut state, v1, 1_000_000, 100, 0);
        fund(&mut state, v2, 1_000_000, 100, 0);
        fund(&mut state, v3, 1_000_000, 100, 0);
        fund(&mut state, boing_tokenomics::FEE_BURN_SINK, 1_000, 0, 0);
        state
            .slash_appeals_mut()
            .record_slash_with_id(
                220,
                provider,
                1_000,
                ConsensusSlashReason::BlobFailedServe,
                1,
                5_000,
            )
            .unwrap();
        state.set_slash_appeal_resolvers(Some(vec![v1, v2, v3]));
        let mut vm = Vm::new();
        vm.set_media_blobs_from(Some(0));
        vm.set_registration_proof_rule(Some(chain_id), 0);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: provider,
                payload: TransactionPayload::SubmitSlashAppeal {
                    slash_id: 220,
                    evidence: vec![],
                },
                access_list: AccessList::new(vec![provider], vec![provider]),
            },
            &mut state,
            ctx(2),
        )
        .unwrap();
        let pin = state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .pin_set_commitment;

        // Wrong pin bind (V2 over different commitment) → reject.
        let wrong_pin = phase20_pin_set_commitment(&[AccountId([9u8; 32])]);
        assert_ne!(wrong_pin, pin);
        let bad = sign_slash_appeal_resolve_ballot_v2(chain_id, 1, 220, true, &key1, &wrong_pin);
        let err = vm
            .execute_with_context(
                &Transaction {
                    nonce: 0,
                    sender: v1,
                    payload: TransactionPayload::ResolveSlashAppealAttested {
                        appeal_id: 1,
                        approved: true,
                        ballot_signature: bad,
                    },
                    access_list: AccessList::new(vec![v1], vec![v1]),
                },
                &mut state,
                ctx(3),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("invalid resolve ballot"),
            "got {err}"
        );

        // V2 cast with correct pin → ok.
        let sig_v2 = sign_slash_appeal_resolve_ballot_v2(chain_id, 1, 220, true, &key1, &pin);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v1,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig_v2,
                },
                access_list: AccessList::new(vec![v1], vec![v1]),
            },
            &mut state,
            ctx(4),
        )
        .unwrap();
        assert!(state
            .slash_appeals()
            .get_appeal(1)
            .unwrap()
            .votes
            .get(&v1)
            .unwrap()
            .ballot_signature
            .is_some());

        // Historical V1 still dual-verifies for mid-upgrade cast.
        let sig_v1 = sign_slash_appeal_resolve_ballot_v1(chain_id, 1, 220, true, &key2);
        vm.execute_with_context(
            &Transaction {
                nonce: 0,
                sender: v2,
                payload: TransactionPayload::ResolveSlashAppealAttested {
                    appeal_id: 1,
                    approved: true,
                    ballot_signature: sig_v1,
                },
                access_list: AccessList::new(vec![v2], vec![v2]),
            },
            &mut state,
            ctx(5),
        )
        .unwrap();
        assert!(state.slash_appeals().is_reversed(220));
    }
}
