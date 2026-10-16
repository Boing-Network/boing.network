//! Boing VM — deterministic execution engine.

use boing_primitives::{
    contract_deploy_init_body, contract_deploy_uses_init_code, create2_contract_address,
    nonce_derived_contract_address, validate_batch_calls, AccountId, AccountState, BatchCall,
    ExecutionLog, Hash, QaPoolVoteKind, Transaction, TransactionPayload,
};
use boing_qa::{
    check_contract_deploy_full, check_contract_deploy_full_with_metadata, QaPoolGovernanceConfig,
    QaQuorumDecision, QaResult, RuleRegistry,
};
use boing_state::{unpaid_counted_voters, QaPendingRecord, StateStore};

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
    /// Same rule set as mempool deploy QA (`check_contract_deploy_full_with_metadata`).
    qa_registry: RuleRegistry,
    /// Pool membership / quorum / voter-reward knobs (execution-time public votes).
    qa_pool_config: QaPoolGovernanceConfig,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            qa_registry: default_qa_registry(),
            qa_pool_config: QaPoolGovernanceConfig::development_default(),
        }
    }

    /// Use the same [RuleRegistry] as the node mempool so execution rejects the same deploys as admission.
    pub fn with_qa_registry(qa_registry: RuleRegistry) -> Self {
        Self {
            qa_registry,
            qa_pool_config: QaPoolGovernanceConfig::development_default(),
        }
    }

    /// Registry + pool governance (public membership, voter rewards).
    pub fn with_qa_policies(
        qa_registry: RuleRegistry,
        qa_pool_config: QaPoolGovernanceConfig,
    ) -> Self {
        Self {
            qa_registry,
            qa_pool_config,
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
                return self.execute_contract_call(state, tx, contract, calldata, exec_ctx);
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
                let params = open_validator_params_at(state, exec_ctx.block_height)?;
                if !boing_primitives::verify_validator_registration(
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
                state
                    .open_validators_mut()
                    .register(&params, exec_ctx.block_height, tx.sender, *node_id)
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
                let refund = state
                    .open_validators_mut()
                    .deregister(tx.sender, *node_id)
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
                execute_name_tx(state, tx, exec_ctx.block_height)?
            }
        };
        Ok(VmExecutionResult {
            gas_used,
            return_data: Vec::new(),
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
            &self.qa_registry,
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
                &self.qa_registry,
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
        let voter_stake = state.get(&voter).map(|s| s.stake).unwrap_or(0);
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
        let reward_each = self.qa_pool_config.reward_per_counted_vote;
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
                &self.qa_registry,
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
                            &self.qa_registry,
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
        let mut interpreter = Interpreter::new(code, GAS_PER_CONTRACT_CALL);
        let gas_used = interpreter.run_with_qa(
            tx.sender,
            *contract,
            calldata,
            state,
            &self.qa_registry,
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
fn execute_name_tx(state: &mut StateStore, tx: &Transaction, height: u64) -> Result<u64, VmError> {
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
                .commit(&params, height, me, *commitment)
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
            let rent = state
                .names()
                .check_register(&params, height, me, &l, salt, *years)
                .map_err(name_err)?;
            if balance < rent {
                return Err(name_err(
                    boing_state::NameRegistryError::InsufficientBalance { rent },
                ));
            }
            state
                .names_mut()
                .register(&params, height, me, &l, salt, *target, *years)
                .map_err(name_err)?
        }
        TransactionPayload::NameRenew { name, years } => {
            let l = label(name)?;
            let mut probe = state.names().clone();
            let rent = probe
                .renew(&params, height, me, &l, *years)
                .map_err(name_err)?;
            if balance < rent {
                return Err(name_err(
                    boing_state::NameRegistryError::InsufficientBalance { rent },
                ));
            }
            state.load_names(probe);
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
