//! Block executor — runs transactions through scheduler + VM.
//!
//! Batches run sequentially; within each batch, Transfer-only txs run in parallel via rayon.

use rayon::prelude::*;

use boing_primitives::{AccountId, ExecutionReceipt, Transaction, TransactionPayload};
use boing_state::{StateCheckpoint, StateStore};
use boing_tokenomics::{
    charge_and_distribute_fee_with_policy, network_fee_amount, NetworkFeePolicy,
};

use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};

use super::interpreter::VmExecutionContext;
use super::parallel::ExecutionView;
use super::{TransactionScheduler, Vm, VmError};

/// Executes a block of transactions. Batches run sequentially; within each batch,
/// Transfer-only txs run in parallel.
pub struct BlockExecutor {
    vm: Vm,
    scheduler: TransactionScheduler,
    fee_policy: NetworkFeePolicy,
}

impl BlockExecutor {
    pub fn new() -> Self {
        Self {
            vm: Vm::new(),
            scheduler: TransactionScheduler::new(),
            fee_policy: NetworkFeePolicy::treasury_default(),
        }
    }

    /// Same QA registry as the node mempool so block execution applies identical deploy rules.
    pub fn with_qa_registry(registry: RuleRegistry) -> Self {
        Self {
            vm: Vm::with_qa_registry(registry),
            scheduler: TransactionScheduler::new(),
            fee_policy: NetworkFeePolicy::treasury_default(),
        }
    }

    pub fn with_policies(
        registry: RuleRegistry,
        pool_config: QaPoolGovernanceConfig,
        fee_policy: NetworkFeePolicy,
    ) -> Self {
        Self {
            vm: Vm::with_qa_policies(registry, pool_config),
            scheduler: TransactionScheduler::new(),
            fee_policy,
        }
    }

    pub fn fee_policy(&self) -> &NetworkFeePolicy {
        &self.fee_policy
    }

    pub fn set_fee_policy(&mut self, policy: NetworkFeePolicy) {
        self.fee_policy = policy;
    }

    pub fn vm(&self) -> &Vm {
        &self.vm
    }

    /// Execute all transactions. Returns total gas used and one receipt per tx (in block order).
    /// On error, state may be partially applied (caller should revert if needed).
    /// Transfer-only batches run in parallel; other batches run sequentially.
    ///
    /// After each successful tx, charges [`network_fee_amount`] from the sender (fail closed)
    /// and distributes per [`NetworkFeePolicy`] to proposer / treasury / burn.
    pub fn execute_block(
        &self,
        block_height: u64,
        block_timestamp: u64,
        txs: &[Transaction],
        state: &mut StateStore,
        fee_recipient: AccountId,
    ) -> Result<(u64, Vec<ExecutionReceipt>), ExecutionError> {
        let exec_ctx = VmExecutionContext {
            block_height,
            block_timestamp,
        };
        let batches = self.scheduler.schedule(txs);
        let mut total_gas = 0u64;
        let mut receipts: Vec<Option<ExecutionReceipt>> = vec![None; txs.len()];

        for batch in batches {
            let all_transfer = batch.iter().all(|&i| {
                matches!(&txs[i].payload, TransactionPayload::Transfer { .. })
            });

            if all_transfer && batch.len() > 1 {
                // Parallel path: copy state slice per tx (sequential read), execute in parallel, merge
                let snapshots: Vec<_> = batch
                    .iter()
                    .map(|&idx| {
                        let tx = txs[idx].clone();
                        let ids: Vec<_> = tx.access_list.all().copied().collect();
                        let snapshot: std::collections::HashMap<_, _> = ids
                            .iter()
                            .filter_map(|id| state.get(id).map(|s| (*id, s.clone())))
                            .collect();
                        (idx, tx, snapshot)
                    })
                    .collect();

                let batch_results: Result<Vec<_>, ExecutionError> = snapshots
                    .par_iter()
                    .map(|(idx, tx, snapshot)| {
                        let mut view = ExecutionView::from_snapshot(snapshot.clone());
                        let out = self
                            .vm
                            .execute_transfer(tx, &mut view)
                            .map_err(ExecutionError::Vm)?;
                        Ok((*idx, view, out.gas_used))
                    })
                    .collect();

                let batch_results = batch_results?;
                // Sanity check: verify no conflicting writes (access lists should be disjoint)
                let mut written: std::collections::HashSet<AccountId> =
                    std::collections::HashSet::new();
                for (_, view, _) in &batch_results {
                    for id in view.account_ids() {
                        if !written.insert(*id) {
                            return Err(ExecutionError::ConflictDetected(format!(
                                "Parallel batch wrote to same account: {:?}",
                                id
                            )));
                        }
                    }
                }
                for (idx, view, gas) in batch_results {
                    view.merge_into(state);
                    let fee = network_fee_amount(gas, &txs[idx].payload, &self.fee_policy);
                    charge_and_distribute_fee_with_policy(
                        state,
                        &txs[idx].sender,
                        fee,
                        &fee_recipient,
                        &self.fee_policy,
                    )
                    .map_err(ExecutionError::Fee)?;
                    receipts[idx] = Some(ExecutionReceipt::from_tx_outcome(
                        &txs[idx],
                        block_height,
                        idx as u32,
                        true,
                        gas,
                        Vec::new(),
                        vec![],
                        None,
                    ));
                    total_gas = total_gas.saturating_add(gas);
                }
            } else {
                // Sequential path: isolate each tx so a fee/VM failure cannot abort the block
                // (and poison the mempool by re-inserting the same deploy forever).
                for &idx in &batch {
                    let tx = &txs[idx];
                    let tx_ckpt = state.checkpoint();
                    match self.vm.execute_with_context(tx, state, exec_ctx) {
                        Ok(out) => {
                            let fee = network_fee_amount(out.gas_used, &tx.payload, &self.fee_policy);
                            match charge_and_distribute_fee_with_policy(
                                state,
                                &tx.sender,
                                fee,
                                &fee_recipient,
                                &self.fee_policy,
                            ) {
                                Ok(()) => {
                                    total_gas = total_gas.saturating_add(out.gas_used);
                                    receipts[idx] = Some(ExecutionReceipt::from_tx_outcome(
                                        tx,
                                        block_height,
                                        idx as u32,
                                        true,
                                        out.gas_used,
                                        out.return_data,
                                        out.logs,
                                        None,
                                    ));
                                }
                                Err(e) => {
                                    apply_failed_tx_consuming_nonce(
                                        state,
                                        tx,
                                        tx_ckpt,
                                        block_height,
                                        idx as u32,
                                        format!("{}", e),
                                        &mut receipts[idx],
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            apply_failed_tx_consuming_nonce(
                                state,
                                tx,
                                tx_ckpt,
                                block_height,
                                idx as u32,
                                format!("{}", e),
                                &mut receipts[idx],
                            );
                        }
                    }
                }
            }
        }
        let receipts: Vec<ExecutionReceipt> = receipts
            .into_iter()
            .enumerate()
            .map(|(i, r)| r.unwrap_or_else(|| panic!("internal error: missing receipt at index {i}")))
            .collect();
        Ok((total_gas, receipts))
    }
}

/// Revert a failed tx's state, still consume the sender nonce so the same payload cannot
/// stall block production (mempool re-insert of a fee-underfunded deploy).
fn apply_failed_tx_consuming_nonce(
    state: &mut StateStore,
    tx: &Transaction,
    tx_ckpt: StateCheckpoint,
    block_height: u64,
    tx_index: u32,
    error: String,
    slot: &mut Option<ExecutionReceipt>,
) {
    state.revert(tx_ckpt);
    if let Some(sender_state) = state.get_mut(&tx.sender) {
        if sender_state.nonce == tx.nonce {
            sender_state.nonce = sender_state.nonce.saturating_add(1);
        }
    }
    *slot = Some(ExecutionReceipt::from_tx_outcome(
        tx,
        block_height,
        tx_index,
        false,
        0,
        Vec::new(),
        vec![],
        Some(error),
    ));
}

impl Default for BlockExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, Account, AccountId, AccountState, Transaction, TransactionPayload};

    fn tx(sender: AccountId, to: AccountId, nonce: u64, amount: u128) -> Transaction {
        Transaction {
            nonce,
            sender,
            payload: TransactionPayload::Transfer { to, amount },
            access_list: AccessList::new(vec![sender, to], vec![sender, to]),
        }
    }

    #[test]
    fn test_execute_block() {
        let exec = BlockExecutor::new();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: a,
            state: AccountState { balance: 100_000, nonce: 0, stake: 0, ..Default::default() },
        });
        state.insert(Account {
            id: b,
            state: AccountState { balance: 0, nonce: 0, stake: 0, ..Default::default() },
        });
        let txs = vec![tx(a, b, 0, 100)];
        let (gas, receipts) = exec.execute_block(1, 0, &txs, &mut state, proposer).unwrap();
        assert_eq!(gas, super::super::vm::GAS_PER_TRANSFER);
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].success);
        let fee = boing_tokenomics::network_fee_amount(gas, &txs[0].payload, &boing_tokenomics::NetworkFeePolicy::treasury_default());
        assert_eq!(state.get(&a).unwrap().balance, 100_000 - 100 - fee);
        assert_eq!(state.get(&b).unwrap().balance, 100);
        let (v, t, burn) = boing_tokenomics::split_fee(fee);
        assert_eq!(state.get(&proposer).map(|s| s.balance).unwrap_or(0), v);
        assert_eq!(
            state
                .get(&boing_tokenomics::PROTOCOL_TREASURY)
                .map(|s| s.balance)
                .unwrap_or(0),
            t
        );
        assert_eq!(
            state
                .get(&boing_tokenomics::FEE_BURN_SINK)
                .map(|s| s.balance)
                .unwrap_or(0),
            burn
        );
        assert_eq!(t, fee);
        assert_eq!(v, 0);
    }

    #[test]
    fn test_execute_block_parallel_transfers() {
        let exec = BlockExecutor::new();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let d = AccountId::from_bytes([4u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account { id: a, state: AccountState { balance: 100_000, nonce: 0, stake: 0, ..Default::default() } });
        state.insert(Account { id: b, state: AccountState { balance: 0, nonce: 0, stake: 0, ..Default::default() } });
        state.insert(Account { id: c, state: AccountState { balance: 100_000, nonce: 0, stake: 0, ..Default::default() } });
        state.insert(Account { id: d, state: AccountState { balance: 0, nonce: 0, stake: 0, ..Default::default() } });
        // Independent transfers a->b and c->d — same batch, parallel execution
        let txs = vec![
            tx(a, b, 0, 100),
            tx(c, d, 0, 50),
        ];
        let (gas, receipts) = exec.execute_block(1, 0, &txs, &mut state, proposer).unwrap();
        assert_eq!(receipts.len(), 2);
        assert!(receipts.iter().all(|r| r.success));
        let fee_each = boing_tokenomics::network_fee_amount(
            super::super::vm::GAS_PER_TRANSFER,
            &txs[0].payload,
            &boing_tokenomics::NetworkFeePolicy::treasury_default(),
        );
        assert_eq!(state.get(&a).unwrap().balance, 100_000 - 100 - fee_each);
        assert_eq!(state.get(&b).unwrap().balance, 100);
        assert_eq!(state.get(&c).unwrap().balance, 100_000 - 50 - fee_each);
        assert_eq!(state.get(&d).unwrap().balance, 50);
        assert_eq!(gas, super::super::vm::GAS_PER_TRANSFER * 2);
    }

    #[test]
    fn underfunded_deploy_fails_in_block_without_aborting_following_transfer() {
        let exec = BlockExecutor::new();
        let deployer = AccountId::from_bytes([1u8; 32]);
        let payee = AccountId::from_bytes([2u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: deployer,
            state: AccountState {
                // Plain ContractDeploy uses 200_000 gas → 10 BOING after GAS_UNITS_PER_BOING.
                balance: 1,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: payee,
            state: AccountState {
                balance: 100_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let deploy = Transaction {
            nonce: 0,
            sender: deployer,
            payload: TransactionPayload::ContractDeploy {
                bytecode: vec![0x00],
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        let transfer = tx(payee, deployer, 0, 10);
        let (_gas, receipts) = exec
            .execute_block(1, 0, &[deploy, transfer], &mut state, proposer)
            .expect("block must commit even when the deploy cannot pay its fee");
        assert_eq!(receipts.len(), 2);
        assert!(!receipts[0].success);
        assert!(receipts[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("insufficient balance for fee"));
        assert!(receipts[1].success);
        assert_eq!(state.get(&deployer).unwrap().nonce, 1);
        assert_eq!(state.get(&deployer).unwrap().balance, 1 + 10);
        assert_eq!(state.get(&payee).unwrap().nonce, 1);
    }

    #[test]
    fn public_qa_vote_pays_from_treasury_and_completes_deploy() {
        use boing_primitives::{QaPoolVoteKind, nonce_derived_contract_address};
        use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};
        use boing_tokenomics::PROTOCOL_TREASURY;

        let mut pool = QaPoolGovernanceConfig::production_default();
        pool.min_quorum_votes = 1;
        pool.quorum_fraction = 0.5;
        pool.allow_threshold_fraction = 0.5;
        pool.reward_per_counted_vote = 5;
        let registry = RuleRegistry::new().with_always_review_categories(vec!["meme".into()]);
        let exec = BlockExecutor::with_policies(
            registry,
            pool,
            boing_tokenomics::NetworkFeePolicy::treasury_default(),
        );

        let deployer = AccountId::from_bytes([1u8; 32]);
        let voter = AccountId::from_bytes([2u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: deployer,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: voter,
            state: AccountState {
                balance: 1_000_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: PROTOCOL_TREASURY,
            state: AccountState {
                balance: 100,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });

        let deploy = Transaction {
            nonce: 0,
            sender: deployer,
            payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: vec![0x00],
                purpose_category: "meme".into(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        };
        let subject = deploy.id();
        let vote = Transaction {
            nonce: 0,
            sender: voter,
            payload: TransactionPayload::QaPoolVote {
                subject,
                vote: QaPoolVoteKind::Allow,
            },
            access_list: AccessList::new(vec![voter], vec![voter]),
        };

        let (_g, receipts) = exec
            .execute_block(1, 0, &[deploy, vote], &mut state, proposer)
            .unwrap();
        assert!(receipts[0].success);
        assert_eq!(receipts[0].return_data, b"qa_pending");
        assert!(receipts[1].success);
        assert_eq!(receipts[1].return_data, b"qa_vote_allow");
        let deploy_fee = boing_tokenomics::fee_for_gas(super::super::vm::GAS_PER_CONTRACT_DEPLOY);
        let vote_fee = boing_tokenomics::fee_for_gas(super::super::vm::GAS_PER_QA_POOL_VOTE);
        assert_eq!(
            state.get(&PROTOCOL_TREASURY).unwrap().balance,
            100 + deploy_fee + vote_fee - 5
        );
        assert_eq!(
            state.get(&voter).unwrap().balance,
            1_000_000 - vote_fee + 5
        );
        let created = nonce_derived_contract_address(&deployer, 0);
        assert!(state.get_contract_code(&created).is_some());
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("VM error: {0}")]
    Vm(#[from] VmError),
    #[error("Conflict detected: {0}")]
    ConflictDetected(String),
    #[error("Fee error: {0}")]
    Fee(#[from] boing_tokenomics::FeeError),
}
