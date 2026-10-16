//! Block executor — runs transactions through scheduler + VM.
//!
//! Batches run sequentially; within each batch, Transfer-only txs run in parallel via rayon.

use rayon::prelude::*;

use boing_primitives::{AccountId, ExecutionReceipt, Transaction, TransactionPayload};
use boing_state::{StateCheckpoint, StateStore};
use boing_tokenomics::{
    network_fee_amount, NetworkFeePolicy,
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
    history: Option<std::sync::Arc<crate::history::ExecutionHistory>>,
}

impl BlockExecutor {
    pub fn new() -> Self {
        Self {
            vm: Vm::new(),
            scheduler: TransactionScheduler::new(),
            fee_policy: NetworkFeePolicy::treasury_default(),
            history: None,
        }
    }

    /// Same QA registry as the node mempool so block execution applies identical deploy rules.
    pub fn with_qa_registry(registry: RuleRegistry) -> Self {
        Self {
            vm: Vm::with_qa_registry(registry),
            scheduler: TransactionScheduler::new(),
            fee_policy: NetworkFeePolicy::treasury_default(),
            history: None,
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
            history: None,
        }
    }

    pub fn fee_policy(&self) -> &NetworkFeePolicy {
        &self.fee_policy
    }

    pub fn set_fee_policy(&mut self, policy: NetworkFeePolicy) {
        self.fee_policy = policy;
    }

    /// Install historical (height-gated) execution rules so already-committed blocks of a live
    /// network replay byte-identically. Heights at/after the last era use the current rules.
    pub fn set_execution_history(&mut self, history: Option<crate::history::ExecutionHistory>) {
        if let Some(h) = &history {
            let cid = (h.chain_id != 0).then_some(h.chain_id);
            self.vm.set_registration_proof_rule(cid, h.registration_proof_v2_from);
        }
        self.history = history.map(std::sync::Arc::new);
    }

    /// Chain id for the validator registration proof (security review #14) and the height from
    /// which only the chain-bound v2 proof is accepted. `None` = v1 proof only (local dev).
    pub fn set_registration_proof_rule(&mut self, chain_id: Option<u64>, v2_from: u64) {
        self.vm.set_registration_proof_rule(chain_id, v2_from);
    }

    pub fn execution_history(&self) -> Option<&crate::history::ExecutionHistory> {
        self.history.as_deref()
    }

    /// Fee policy + remainder rule in force at `height`.
    pub fn fee_rule_at(&self, height: u64) -> (NetworkFeePolicy, boing_tokenomics::FeeRemainder) {
        if let Some(era) = self.history.as_ref().and_then(|h| h.fee_era_at(height)) {
            return (era.policy.clone(), era.remainder);
        }
        (self.fee_policy.clone(), boing_tokenomics::FeeRemainder::Treasury)
    }

    /// Whether failed txs pay the failed-tx fee at `height` (always, for chains without history).
    pub fn failed_tx_fee_active(&self, height: u64) -> bool {
        self.history.as_ref().map(|h| h.failed_tx_fee_active(height)).unwrap_or(true)
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
        let (fee_policy, fee_remainder) = self.fee_rule_at(block_height);
        let failed_fee = self.failed_tx_fee_active(block_height).then(|| FailedTxFee {
            policy: &fee_policy,
            remainder: fee_remainder,
            recipient: fee_recipient,
        });
        let batches = self.scheduler.schedule(txs);
        let mut total_gas = 0u64;
        let mut receipts: Vec<Option<ExecutionReceipt>> = vec![None; txs.len()];

        for batch in batches {
            let all_transfer = batch.iter().all(|&i| {
                matches!(&txs[i].payload, TransactionPayload::Transfer { .. })
            });

            // The parallel path only sees the accounts each tx declares, so it is used only when every
            // transfer's access list covers its sender and recipient (read + write) and the batch's
            // touched accounts are pairwise disjoint. Otherwise (e.g. an SDK transfer with an empty
            // access list next to a faucet transfer) fall back to the sequential path, which yields the
            // same result. Previously such a block failed with "Account not found" and the producer
            // re-inserted the txs forever, stalling the chain.
            // Parallel attempt. Any failure inside it (bad nonce, insufficient balance, fee shortfall,
            // conflict) reverts the attempt and re-runs the batch on the sequential path, which records
            // the failing tx as a failed receipt instead of failing the whole block. Successful parallel
            // execution is identical to sequential, so blocks that executed before are unaffected; before
            // this, such a block errored and the producer re-inserted the same txs forever (chain stall).
            let mut parallel_done = false;
            if all_transfer && batch.len() > 1 && parallel_transfer_batch_is_safe(&batch, txs) {
                let par_ckpt = state.checkpoint();
                let par_receipts_before: Vec<Option<ExecutionReceipt>> =
                    batch.iter().map(|&i| receipts[i].clone()).collect();
                let attempt = (|| -> Result<u64, ExecutionError> {
                let mut par_gas = 0u64;
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
                    let fee = network_fee_amount(gas, &txs[idx].payload, &fee_policy);
                    boing_tokenomics::charge_and_distribute_fee_with_remainder(
                        state,
                        &txs[idx].sender,
                        fee,
                        &fee_recipient,
                        &fee_policy,
                        fee_remainder,
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
                    par_gas = par_gas.saturating_add(gas);
                }
                Ok(par_gas)
                })();
                match attempt {
                    Ok(g) => {
                        total_gas = total_gas.saturating_add(g);
                        parallel_done = true;
                    }
                    Err(_) => {
                        state.revert(par_ckpt);
                        for (k, &i) in batch.iter().enumerate() {
                            receipts[i] = par_receipts_before[k].clone();
                        }
                    }
                }
            }
            if !parallel_done {
                // Sequential path: isolate each tx so a fee/VM failure cannot abort the block
                // (and poison the mempool by re-inserting the same deploy forever).
                for &idx in &batch {
                    let tx = &txs[idx];
                    let tx_ckpt = state.checkpoint();
                    match self.vm.execute_with_context(tx, state, exec_ctx) {
                        Ok(out) => {
                            let fee = network_fee_amount(out.gas_used, &tx.payload, &fee_policy);
                            match boing_tokenomics::charge_and_distribute_fee_with_remainder(
                                state,
                                &tx.sender,
                                fee,
                                &fee_recipient,
                                &fee_policy,
                                fee_remainder,
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
                                    total_gas = total_gas.saturating_add(apply_failed_tx_consuming_nonce(
                                        state,
                                        tx,
                                        tx_ckpt,
                                        block_height,
                                        idx as u32,
                                        format!("{}", e),
                                        &mut receipts[idx],
                                        failed_fee.as_ref(),
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            total_gas = total_gas.saturating_add(apply_failed_tx_consuming_nonce(
                                state,
                                tx,
                                tx_ckpt,
                                block_height,
                                idx as u32,
                                format!("{}", e),
                                &mut receipts[idx],
                                failed_fee.as_ref(),
                            ));
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

/// Gas recorded (and charged) for a failed tx once the failed-tx fee is active: the intrinsic
/// transfer gas, so a failing tx costs at least as much as the cheapest successful one.
pub const FAILED_TX_GAS: u64 = crate::vm::GAS_PER_TRANSFER;

/// Fee charged for a failed tx: `fee_for_gas(FAILED_TX_GAS) + extra_fixed_fee` (no amount-bps part:
/// nothing was transferred).
pub fn failed_tx_fee_amount(policy: &NetworkFeePolicy) -> u128 {
    boing_tokenomics::fee_for_gas(FAILED_TX_GAS).saturating_add(policy.extra_fixed_fee)
}

/// Fee rule for failed txs in one block (`None` before the activation height).
struct FailedTxFee<'a> {
    policy: &'a NetworkFeePolicy,
    remainder: boing_tokenomics::FeeRemainder,
    recipient: AccountId,
}

/// Revert a failed tx's state, still consume the sender nonce so the same payload cannot
/// stall block production (mempool re-insert of a fee-underfunded deploy).
///
/// When the failed-tx fee is active and the tx passed the nonce check (its nonce equals the sender's),
/// the sender also pays `min(balance, failed_tx_fee_amount)`. The charge is capped at the balance, so
/// this step can never error and a failing tx can never fail (stall) the block. Returns gas used.
#[allow(clippy::too_many_arguments)]
fn apply_failed_tx_consuming_nonce(
    state: &mut StateStore,
    tx: &Transaction,
    tx_ckpt: StateCheckpoint,
    block_height: u64,
    tx_index: u32,
    error: String,
    slot: &mut Option<ExecutionReceipt>,
    failed_fee: Option<&FailedTxFee<'_>>,
) -> u64 {
    state.revert(tx_ckpt);
    let mut gas_used = 0u64;
    let mut charge = 0u128;
    if let Some(sender_state) = state.get_mut(&tx.sender) {
        if sender_state.nonce == tx.nonce {
            sender_state.nonce = sender_state.nonce.saturating_add(1);
            if let Some(f) = failed_fee {
                charge = failed_tx_fee_amount(f.policy).min(sender_state.balance);
                gas_used = FAILED_TX_GAS;
            }
        }
    }
    if let Some(f) = failed_fee {
        if charge > 0 {
            // Cannot fail: the sender exists and `charge <= balance`.
            let _ = boing_tokenomics::charge_and_distribute_fee_with_remainder(
                state,
                &tx.sender,
                charge,
                &f.recipient,
                f.policy,
                f.remainder,
            );
        }
    }
    *slot = Some(ExecutionReceipt::from_tx_outcome(
        tx,
        block_height,
        tx_index,
        false,
        gas_used,
        Vec::new(),
        vec![],
        Some(error),
    ));
    gas_used
}

impl Default for BlockExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// True when every transfer in `batch` declares its sender and recipient for read and write and no two
/// transfers touch the same account, so per-tx snapshots are complete and merges cannot collide.
fn parallel_transfer_batch_is_safe(batch: &[usize], txs: &[Transaction]) -> bool {
    let mut touched = std::collections::HashSet::new();
    for &i in batch {
        let tx = &txs[i];
        let TransactionPayload::Transfer { to, .. } = &tx.payload else {
            return false;
        };
        for id in [tx.sender, *to] {
            if !tx.access_list.read.contains(&id) || !tx.access_list.write.contains(&id) {
                return false;
            }
        }
        let own: std::collections::HashSet<AccountId> = tx.access_list.all().copied().collect();
        for id in own {
            if !touched.insert(id) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, Account, AccountId, AccountState, Transaction, TransactionPayload};

    /// Regression (testnet stall at height 217): a transfer with an empty access list in the same block
    /// as a properly declared transfer must execute (sequentially), not fail the whole block.
    #[test]
    fn transfer_with_empty_access_list_next_to_declared_transfer_executes() {
        let exec = BlockExecutor::new();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let d = AccountId::from_bytes([4u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        for id in [a, c] {
            state.insert(Account { id, state: AccountState { balance: 100_000, ..Default::default() } });
        }
        let mut t1 = tx(a, b, 0, 5);
        t1.access_list = AccessList::default();
        let t2 = tx(c, d, 0, 7);
        let (_gas, receipts) = exec.execute_block(1, 0, &[t1, t2], &mut state, proposer).unwrap();
        assert!(receipts.iter().all(|r| r.success));
        assert_eq!(state.get(&b).unwrap().balance, 5);
        assert_eq!(state.get(&d).unwrap().balance, 7);
        // Same result regardless of order.
        let mut s2 = StateStore::new();
        for id in [a, c] {
            s2.insert(Account { id, state: AccountState { balance: 100_000, ..Default::default() } });
        }
        let mut t1b = tx(a, b, 0, 5);
        t1b.access_list = AccessList::default();
        exec.execute_block(1, 0, &[tx(c, d, 0, 7), t1b], &mut s2, proposer).unwrap();
        assert_eq!(s2.get(&b).unwrap().balance, 5);
        assert_eq!(s2.get(&d).unwrap().balance, 7);
    }

    /// Security review 2026-10-16: a failing transfer inside an otherwise parallel-safe batch (here: the
    /// sender cannot cover amount + fee; also a stale nonce) must become a failed receipt, not fail the
    /// whole block (the producer used to re-insert the same txs forever, stalling the chain).
    #[test]
    fn failing_tx_in_parallel_batch_does_not_fail_block() {
        // Pre-#3 rules (failed txs only consume the nonce), as on testnet below the activation height.
        let mut exec = BlockExecutor::new();
        exec.set_execution_history(Some(crate::history::ExecutionHistory {
            failed_tx_fee_from: u64::MAX,
            ..Default::default()
        }));
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let d = AccountId::from_bytes([4u8; 32]);
        let e = AccountId::from_bytes([5u8; 32]);
        let f = AccountId::from_bytes([6u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account { id: a, state: AccountState { balance: 100_000, ..Default::default() } });
        state.insert(Account { id: c, state: AccountState { balance: 50, ..Default::default() } });
        state.insert(Account { id: e, state: AccountState { balance: 100_000, nonce: 3, ..Default::default() } });
        // c sends its whole balance (cannot pay the fee); e uses a stale nonce.
        let txs = [tx(a, b, 0, 5), tx(c, d, 0, 50), tx(e, f, 0, 1)];
        assert!(parallel_transfer_batch_is_safe(&[0, 1, 2], &txs));
        let (_gas, receipts) = exec.execute_block(1, 0, &txs, &mut state, proposer).expect("block must execute");
        assert!(receipts[0].success);
        assert!(!receipts[1].success);
        assert!(!receipts[2].success);
        assert_eq!(state.get(&b).unwrap().balance, 5);
        assert!(state.get(&d).is_none(), "failed transfer is reverted");
        assert_eq!(state.get(&c).unwrap().balance, 50);
        assert_eq!(state.get(&c).unwrap().nonce, 1, "nonce consumed like the sequential path");
        // Identical to executing the same txs one block-tx at a time on the sequential path.
        let mut seq = StateStore::new();
        seq.insert(Account { id: a, state: AccountState { balance: 100_000, ..Default::default() } });
        seq.insert(Account { id: c, state: AccountState { balance: 50, ..Default::default() } });
        seq.insert(Account { id: e, state: AccountState { balance: 100_000, nonce: 3, ..Default::default() } });
        let mut t0 = tx(a, b, 0, 5);
        t0.access_list = AccessList::default(); // forces the sequential path
        let (_g, r2) = exec.execute_block(1, 0, &[t0, tx(c, d, 0, 50), tx(e, f, 0, 1)], &mut seq, proposer).unwrap();
        assert_eq!(r2.iter().map(|r| r.success).collect::<Vec<_>>(), vec![true, false, false]);
        assert_eq!(seq.state_root(), state.state_root());
    }

    /// Security review #3: after activation a failed tx (nonce matched) is charged the failed-tx fee,
    /// its nonce advances, its state changes are reverted and its receipt is a failure with gas used.
    /// Stale-nonce txs pay nothing. Applies on both the parallel and the sequential path.
    #[test]
    fn failed_tx_pays_fee_after_activation() {
        let mut exec = BlockExecutor::new();
        exec.set_execution_history(Some(crate::history::ExecutionHistory {
            failed_tx_fee_from: 5,
            ..Default::default()
        }));
        let fee = failed_tx_fee_amount(exec.fee_policy());
        assert!(fee >= 1);
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let d = AccountId::from_bytes([4u8; 32]);
        let e = AccountId::from_bytes([5u8; 32]);
        let f = AccountId::from_bytes([6u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mk = || {
            let mut s = StateStore::new();
            s.insert(Account { id: a, state: AccountState { balance: 100_000, ..Default::default() } });
            s.insert(Account { id: c, state: AccountState { balance: 50, ..Default::default() } });
            s.insert(Account { id: e, state: AccountState { balance: 100_000, nonce: 3, ..Default::default() } });
            s
        };
        let txs = [tx(a, b, 0, 5), tx(c, d, 0, 50), tx(e, f, 0, 1)];
        let treasury_before = mk().get(&boing_tokenomics::PROTOCOL_TREASURY).map(|x| x.balance).unwrap_or(0);

        // Before activation (height 4): legacy behaviour.
        let mut s4 = mk();
        exec.execute_block(4, 0, &txs, &mut s4, proposer).unwrap();
        assert_eq!(s4.get(&c).unwrap().balance, 50);

        // After activation (height 5), parallel-safe batch.
        let mut s5 = mk();
        let (gas, r) = exec.execute_block(5, 0, &txs, &mut s5, proposer).unwrap();
        assert_eq!(r.iter().map(|r| r.success).collect::<Vec<_>>(), vec![true, false, false]);
        assert_eq!(s5.get(&c).unwrap().balance, 50 - fee, "failed tx charged the failed-tx fee");
        assert_eq!(s5.get(&c).unwrap().nonce, 1, "nonce advances");
        assert!(s5.get(&d).is_none(), "state reverted");
        assert_eq!(r[1].gas_used, FAILED_TX_GAS);
        assert!(r[1].error.is_some());
        // stale nonce: nothing charged, nonce unchanged
        assert_eq!(s5.get(&e).unwrap().balance, 100_000);
        assert_eq!(s5.get(&e).unwrap().nonce, 3);
        assert_eq!(r[2].gas_used, 0);
        assert_eq!(gas, r.iter().map(|r| r.gas_used).sum::<u64>());
        let treasury_after = s5.get(&boing_tokenomics::PROTOCOL_TREASURY).map(|x| x.balance).unwrap_or(0);
        assert!(treasury_after >= treasury_before + fee, "failed-tx fee is distributed");

        // Sequential path gives the identical state root (determinism).
        let mut seq = mk();
        let mut t0 = tx(a, b, 0, 5);
        t0.access_list = AccessList::default();
        exec.execute_block(5, 0, &[t0, tx(c, d, 0, 50), tx(e, f, 0, 1)], &mut seq, proposer).unwrap();
        assert_eq!(seq.state_root(), s5.state_root());
        // Re-executing the same block from the same state is deterministic.
        let mut again = mk();
        exec.execute_block(5, 0, &txs, &mut again, proposer).unwrap();
        assert_eq!(again.state_root(), s5.state_root());
    }

    /// The failed-tx fee is capped at the balance (never errors, never stalls the block), and a
    /// failing Native Batch is atomic: every call is reverted and the fee is charged once.
    #[test]
    fn failed_batch_is_atomic_and_fee_capped_at_balance() {
        use boing_primitives::BatchCall;
        let exec = BlockExecutor::new(); // no history: rule active from genesis (new chains, mainnet)
        let fee = failed_tx_fee_amount(exec.fee_policy());
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let z = AccountId::from_bytes([7u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account { id: a, state: AccountState { balance: 1_000, ..Default::default() } });
        state.insert(Account { id: z, state: AccountState { balance: 0, ..Default::default() } });
        // second call overdraws: the whole batch must revert (b gets nothing)
        let batch = Transaction {
            nonce: 0,
            sender: a,
            payload: TransactionPayload::Batch {
                calls: vec![
                    BatchCall::Transfer { to: b, amount: 100 },
                    BatchCall::Transfer { to: c, amount: 10_000 },
                ],
            },
            access_list: AccessList::new(vec![a, b, c], vec![a, b, c]),
        };
        // z has a zero balance: its failing tx is included, nonce advances, nothing to charge.
        let zt = tx(z, b, 0, 1);
        let (_g, r) = exec.execute_block(1, 0, &[batch, zt], &mut state, proposer).expect("block executes");
        assert!(!r[0].success && !r[1].success);
        assert!(state.get(&b).is_none() && state.get(&c).is_none(), "batch reverted atomically");
        assert_eq!(state.get(&a).unwrap().balance, 1_000 - fee);
        assert_eq!(state.get(&a).unwrap().nonce, 1);
        assert_eq!(state.get(&z).unwrap().balance, 0);
        assert_eq!(state.get(&z).unwrap().nonce, 1);
    }

    #[test]
    fn parallel_safety_check() {
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let d = AccountId::from_bytes([4u8; 32]);
        assert!(parallel_transfer_batch_is_safe(&[0, 1], &[tx(a, b, 0, 1), tx(c, d, 0, 1)]));
        let mut bad = tx(a, b, 0, 1);
        bad.access_list = AccessList::new(vec![a], vec![a]);
        assert!(!parallel_transfer_batch_is_safe(&[0, 1], &[bad, tx(c, d, 0, 1)]));
        assert!(!parallel_transfer_batch_is_safe(&[0, 1], &[tx(a, b, 0, 1), tx(c, b, 0, 1)]));
    }

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
        // Failed-tx fee (#3) takes the deployer's whole 1 BOING (capped at the balance).
        assert_eq!(state.get(&deployer).unwrap().balance, 10);
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
