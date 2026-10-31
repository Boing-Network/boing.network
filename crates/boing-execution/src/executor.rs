//! Block executor — runs transactions through scheduler + VM.
//!
//! Batches run sequentially; within each batch, Transfer-only txs run in parallel via rayon.

use rayon::prelude::*;

use boing_primitives::{AccountId, ExecutionReceipt, Transaction, TransactionPayload};
use boing_primitives::{Denomination, BASE_UNITS_PER_BOING};
use boing_state::{StateCheckpoint, StateStore};
use boing_tokenomics::{charge_priority_tip, network_fee_amount_in, NetworkFeePolicy};

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
            self.vm
                .set_registration_proof_rule(cid, h.registration_proof_v2_from);
            self.vm.set_security_v4_from(h.security_v4_from);
            self.vm.set_base_units_from(h.base_units_from);
            self.vm.set_native_value_from(h.native_value_from);
            self.vm.set_vm_hardening_from(h.vm_hardening_from);
        } else {
            self.vm.set_base_units_from(0);
            self.vm.set_native_value_from(0);
            self.vm.set_vm_hardening_from(0);
        }
        self.history = history.map(std::sync::Arc::new);
    }

    /// Chain id for the validator registration proof (security review #14) and the height from
    /// which only the chain-bound v2 proof is accepted. `None` = v1 proof only (local dev).
    pub fn set_registration_proof_rule(&mut self, chain_id: Option<u64>, v2_from: u64) {
        self.vm.set_registration_proof_rule(chain_id, v2_from);
    }

    pub fn registration_proof_rule(&self) -> (Option<u64>, u64) {
        self.vm.registration_proof_rule()
    }

    pub fn execution_history(&self) -> Option<&crate::history::ExecutionHistory> {
        self.history.as_deref()
    }

    /// Fee policy + remainder rule in force at `height`.
    ///
    /// Before [`ExecutionHistory::micro_fees_from`], `execution_gas_fee_bps` is clamped to **10000**
    /// (full historical gas) so reduced/micro/waived BPS cannot change past state roots. After that
    /// height (or with no history / `micro_fees_from = 0`), the configured BPS applies.
    pub fn fee_rule_at(&self, height: u64) -> (NetworkFeePolicy, boing_tokenomics::FeeRemainder) {
        let (mut policy, remainder) =
            if let Some(era) = self.history.as_ref().and_then(|h| h.fee_era_at(height)) {
                (era.policy.clone(), era.remainder)
            } else {
                (
                    self.fee_policy.clone(),
                    boing_tokenomics::FeeRemainder::Treasury,
                )
            };
        let micro_ok = self
            .history
            .as_ref()
            .map(|h| h.micro_fees_active(height))
            .unwrap_or(true);
        if !micro_ok && policy.execution_gas_fee_bps < 10_000 {
            policy.execution_gas_fee_bps = 10_000;
        }
        (policy, remainder)
    }

    /// Whether micro / reduced execution-gas BPS is active at `height`.
    pub fn micro_fees_active(&self, height: u64) -> bool {
        self.history
            .as_ref()
            .map(|h| h.micro_fees_active(height))
            .unwrap_or(true)
    }

    /// Whether failed txs pay the failed-tx fee at `height` (always, for chains without history).
    pub fn failed_tx_fee_active(&self, height: u64) -> bool {
        self.history
            .as_ref()
            .map(|h| h.failed_tx_fee_active(height))
            .unwrap_or(true)
    }

    /// Ledger unit at `height`: base units, except on a network history before its migration height.
    pub fn denomination_at(&self, height: u64) -> Denomination {
        self.history
            .as_ref()
            .map(|h| h.denomination_at(height))
            .unwrap_or(Denomination::BaseUnits)
    }

    /// Migration height of the ledger to base units (`None`: base units from genesis, nothing to do).
    pub fn redenomination_height(&self) -> Option<u64> {
        self.history
            .as_ref()
            .map(|h| h.base_units_from)
            .filter(|h| *h > 0)
    }

    /// Bring a state taken at height `next_height - 1` to the unit `next_height` executes in: at the
    /// migration height this multiplies every native amount by 10^18 (exactly once: only the state of
    /// height `H - 1` is ever migrated). Block execution calls it; admission and simulation for the next
    /// block call it on their state copy so they agree with execution.
    pub fn prepare_state_for_height(&self, next_height: u64, state: &mut StateStore) -> bool {
        if self.redenomination_height() == Some(next_height) {
            state.redenominate(BASE_UNITS_PER_BOING);
            true
        } else {
            false
        }
    }

    /// Full network fee for a tx included at `height`.
    pub fn network_fee_at(&self, gas_used: u64, payload: &TransactionPayload, height: u64) -> u128 {
        let (policy, _) = self.fee_rule_at(height);
        network_fee_amount_in(gas_used, payload, &policy, self.denomination_at(height))
    }

    /// Proposer emission for the block at `height`, in the ledger unit of that height.
    pub fn block_emission(&self, height: u64) -> u128 {
        boing_tokenomics::block_emission_validators_in(height, self.denomination_at(height))
    }

    /// Whether contract calls may carry native value at `height` (always, for chains without history).
    pub fn native_value_active(&self, height: u64) -> bool {
        self.vm.native_value_active(height)
    }

    /// Whether the security-review v4 rules apply at `height` (always, for chains without history).
    pub fn security_v4_active(&self, height: u64) -> bool {
        self.history
            .as_ref()
            .map(|h| h.security_v4_active(height))
            .unwrap_or(true)
    }

    pub fn vm(&self) -> &Vm {
        &self.vm
    }

    /// Enable L1 media PutBlob from `height` (`None` = disabled). Must match all validators.
    pub fn set_media_blobs_from(&mut self, from: Option<u64>) {
        self.vm.set_media_blobs_from(from);
    }

    pub fn set_blobs_root_header_from(&mut self, from: Option<u64>) {
        self.vm.set_blobs_root_header_from(from);
    }

    /// Phase 7 gated Put/Announce cap raise (`None` = keep 16 MiB).
    pub fn set_media_blobs_phase7_cap_from(&mut self, from: Option<u64>) {
        self.vm.set_media_blobs_phase7_cap_from(from);
    }

    /// Phase 32 BLS multi-pin committee flip (`None` = keep Ed25519 tx 32 default).
    pub fn set_bls_committee_flip_from(&mut self, from: Option<u64>) {
        self.vm.set_bls_committee_flip_from(from);
    }

    /// Governance consensus txs (indices 37–39). `None` = never.
    pub fn set_governance_consensus_from(&mut self, from: Option<u64>) {
        self.vm.set_governance_consensus_from(from);
    }

    pub fn set_blake3_from(&mut self, from: Option<u64>) {
        self.vm.set_blake3_from(from);
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
        self.execute_block_with_tips(
            block_height,
            block_timestamp,
            txs,
            state,
            fee_recipient,
            &[],
        )
    }

    /// Like [`Self::execute_block`], with per-tx priority tips (empty = all zero).
    /// Tips are charged after the protocol fee and credited 100% to `fee_recipient`.
    pub fn execute_block_with_tips(
        &self,
        block_height: u64,
        block_timestamp: u64,
        txs: &[Transaction],
        state: &mut StateStore,
        fee_recipient: AccountId,
        priority_tips: &[u128],
    ) -> Result<(u64, Vec<ExecutionReceipt>), ExecutionError> {
        if !priority_tips.is_empty() && priority_tips.len() != txs.len() {
            return Err(ExecutionError::ConflictDetected(format!(
                "priority_tips length {} != txs length {}",
                priority_tips.len(),
                txs.len()
            )));
        }
        let tip_at = |i: usize| priority_tips.get(i).copied().unwrap_or(0);
        let exec_ctx = VmExecutionContext {
            block_height,
            block_timestamp,
            native_value: self.vm.native_value_active(block_height),
            vm_hardening: self.vm.vm_hardening_active(block_height),
            blake3_opcode: false,
        };
        // Ledger redenomination (whole BOING -> base units) happens once, at the start of block H.
        self.prepare_state_for_height(block_height, state);
        let denom = self.denomination_at(block_height);
        let (fee_policy, fee_remainder) = self.fee_rule_at(block_height);
        let failed_fee = self
            .failed_tx_fee_active(block_height)
            .then_some(FailedTxFee {
                policy: &fee_policy,
                remainder: fee_remainder,
                recipient: fee_recipient,
                denom,
            });
        // Security review #9: parallel merges write each tx's pre-batch snapshot back, so a later tx
        // whose access list includes a fee-receiving account (fee recipient, protocol treasury, burn
        // sink) overwrote fees credited earlier in the batch. From v4 such batches run sequentially
        // (same result as the parallel path for every other batch).
        let exclude_fee_recipient = self.security_v4_active(block_height);
        let batches = self.scheduler.schedule(txs);
        let mut total_gas = 0u64;
        let mut receipts: Vec<Option<ExecutionReceipt>> = vec![None; txs.len()];

        for batch in batches {
            let all_transfer = batch
                .iter()
                .all(|&i| matches!(&txs[i].payload, TransactionPayload::Transfer { .. }));

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
            if all_transfer
                && batch.len() > 1
                && parallel_transfer_batch_is_safe(&batch, txs)
                && !(exclude_fee_recipient
                    && batch.iter().any(|&i| {
                        txs[i].access_list.all().any(|id| {
                            *id == fee_recipient
                                || *id == boing_tokenomics::PROTOCOL_TREASURY
                                || *id == boing_tokenomics::FEE_BURN_SINK
                        })
                    }))
            {
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
                        let fee = network_fee_amount_in(gas, &txs[idx].payload, &fee_policy, denom);
                        boing_tokenomics::charge_and_distribute_fee_with_remainder(
                            state,
                            &txs[idx].sender,
                            fee,
                            &fee_recipient,
                            &fee_policy,
                            fee_remainder,
                        )
                        .map_err(ExecutionError::Fee)?;
                        charge_priority_tip(
                            state,
                            &txs[idx].sender,
                            tip_at(idx),
                            &fee_recipient,
                            &fee_policy,
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
                            let fee = network_fee_amount_in(
                                out.gas_used,
                                &tx.payload,
                                &fee_policy,
                                denom,
                            );
                            let tip = tip_at(idx);
                            let fee_ok =
                                boing_tokenomics::charge_and_distribute_fee_with_remainder(
                                    state,
                                    &tx.sender,
                                    fee,
                                    &fee_recipient,
                                    &fee_policy,
                                    fee_remainder,
                                )
                                .and_then(|_| {
                                    charge_priority_tip(
                                        state,
                                        &tx.sender,
                                        tip,
                                        &fee_recipient,
                                        &fee_policy,
                                    )
                                });
                            match fee_ok {
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
                                    total_gas =
                                        total_gas.saturating_add(apply_failed_tx_consuming_nonce(
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
            .map(|(i, r)| {
                r.unwrap_or_else(|| panic!("internal error: missing receipt at index {i}"))
            })
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
    failed_tx_fee_amount_in(policy, Denomination::BaseUnits)
}

/// [`failed_tx_fee_amount`] in the ledger unit `denom`.
pub fn failed_tx_fee_amount_in(policy: &NetworkFeePolicy, denom: Denomination) -> u128 {
    boing_tokenomics::execution_gas_fee_in(FAILED_TX_GAS, policy, denom)
        .saturating_add(policy.extra_fixed_fee)
}

/// Fee rule for failed txs in one block (`None` before the activation height).
struct FailedTxFee<'a> {
    policy: &'a NetworkFeePolicy,
    remainder: boing_tokenomics::FeeRemainder,
    recipient: AccountId,
    denom: Denomination,
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
                charge = failed_tx_fee_amount_in(f.policy, f.denom).min(sender_state.balance);
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
    use boing_primitives::{
        AccessList, Account, AccountId, AccountState, Transaction, TransactionPayload,
    };

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
            state.insert(Account {
                id,
                state: AccountState {
                    balance: 100_000 * B,
                    ..Default::default()
                },
            });
        }
        let mut t1 = tx(a, b, 0, 5);
        t1.access_list = AccessList::default();
        let t2 = tx(c, d, 0, 7);
        let (_gas, receipts) = exec
            .execute_block(1, 0, &[t1, t2], &mut state, proposer)
            .unwrap();
        assert!(receipts.iter().all(|r| r.success));
        assert_eq!(state.get(&b).unwrap().balance, 5);
        assert_eq!(state.get(&d).unwrap().balance, 7);
        // Same result regardless of order.
        let mut s2 = StateStore::new();
        for id in [a, c] {
            s2.insert(Account {
                id,
                state: AccountState {
                    balance: 100_000 * B,
                    ..Default::default()
                },
            });
        }
        let mut t1b = tx(a, b, 0, 5);
        t1b.access_list = AccessList::default();
        exec.execute_block(1, 0, &[tx(c, d, 0, 7), t1b], &mut s2, proposer)
            .unwrap();
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
        state.insert(Account {
            id: a,
            state: AccountState {
                balance: 100_000 * B,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: c,
            state: AccountState {
                balance: 50 * B,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: e,
            state: AccountState {
                balance: 100_000 * B,
                nonce: 3,
                ..Default::default()
            },
        });
        // c sends its whole balance (cannot pay the fee); e uses a stale nonce.
        let txs = [tx(a, b, 0, 5), tx(c, d, 0, 50 * B), tx(e, f, 0, 1)];
        assert!(parallel_transfer_batch_is_safe(&[0, 1, 2], &txs));
        let (_gas, receipts) = exec
            .execute_block(1, 0, &txs, &mut state, proposer)
            .expect("block must execute");
        assert!(receipts[0].success);
        assert!(!receipts[1].success);
        assert!(!receipts[2].success);
        assert_eq!(state.get(&b).unwrap().balance, 5);
        assert!(state.get(&d).is_none(), "failed transfer is reverted");
        assert_eq!(state.get(&c).unwrap().balance, 50 * B);
        assert_eq!(
            state.get(&c).unwrap().nonce,
            1,
            "nonce consumed like the sequential path"
        );
        // Identical to executing the same txs one block-tx at a time on the sequential path.
        let mut seq = StateStore::new();
        seq.insert(Account {
            id: a,
            state: AccountState {
                balance: 100_000 * B,
                ..Default::default()
            },
        });
        seq.insert(Account {
            id: c,
            state: AccountState {
                balance: 50 * B,
                ..Default::default()
            },
        });
        seq.insert(Account {
            id: e,
            state: AccountState {
                balance: 100_000 * B,
                nonce: 3,
                ..Default::default()
            },
        });
        let mut t0 = tx(a, b, 0, 5);
        t0.access_list = AccessList::default(); // forces the sequential path
        let (_g, r2) = exec
            .execute_block(
                1,
                0,
                &[t0, tx(c, d, 0, 50 * B), tx(e, f, 0, 1)],
                &mut seq,
                proposer,
            )
            .unwrap();
        assert_eq!(
            r2.iter().map(|r| r.success).collect::<Vec<_>>(),
            vec![true, false, false]
        );
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
            s.insert(Account {
                id: a,
                state: AccountState {
                    balance: 100_000 * B,
                    ..Default::default()
                },
            });
            s.insert(Account {
                id: c,
                state: AccountState {
                    balance: 50 * B,
                    ..Default::default()
                },
            });
            s.insert(Account {
                id: e,
                state: AccountState {
                    balance: 100_000 * B,
                    nonce: 3,
                    ..Default::default()
                },
            });
            s
        };
        let txs = [tx(a, b, 0, 5), tx(c, d, 0, 50 * B), tx(e, f, 0, 1)];
        let treasury_before = mk()
            .get(&boing_tokenomics::PROTOCOL_TREASURY)
            .map(|x| x.balance)
            .unwrap_or(0);

        // Before activation (height 4): legacy behaviour.
        let mut s4 = mk();
        exec.execute_block(4, 0, &txs, &mut s4, proposer).unwrap();
        assert_eq!(s4.get(&c).unwrap().balance, 50 * B);

        // After activation (height 5), parallel-safe batch.
        let mut s5 = mk();
        let (gas, r) = exec.execute_block(5, 0, &txs, &mut s5, proposer).unwrap();
        assert_eq!(
            r.iter().map(|r| r.success).collect::<Vec<_>>(),
            vec![true, false, false]
        );
        assert_eq!(
            s5.get(&c).unwrap().balance,
            50 * B - fee,
            "failed tx charged the failed-tx fee"
        );
        assert_eq!(s5.get(&c).unwrap().nonce, 1, "nonce advances");
        assert!(s5.get(&d).is_none(), "state reverted");
        assert_eq!(r[1].gas_used, FAILED_TX_GAS);
        assert!(r[1].error.is_some());
        // stale nonce: nothing charged, nonce unchanged
        assert_eq!(s5.get(&e).unwrap().balance, 100_000 * B);
        assert_eq!(s5.get(&e).unwrap().nonce, 3);
        assert_eq!(r[2].gas_used, 0);
        assert_eq!(gas, r.iter().map(|r| r.gas_used).sum::<u64>());
        let treasury_after = s5
            .get(&boing_tokenomics::PROTOCOL_TREASURY)
            .map(|x| x.balance)
            .unwrap_or(0);
        assert!(
            treasury_after >= treasury_before + fee,
            "failed-tx fee is distributed"
        );

        // Sequential path gives the identical state root (determinism).
        let mut seq = mk();
        let mut t0 = tx(a, b, 0, 5);
        t0.access_list = AccessList::default();
        exec.execute_block(
            5,
            0,
            &[t0, tx(c, d, 0, 50 * B), tx(e, f, 0, 1)],
            &mut seq,
            proposer,
        )
        .unwrap();
        assert_eq!(seq.state_root(), s5.state_root());
        // Re-executing the same block from the same state is deterministic.
        let mut again = mk();
        exec.execute_block(5, 0, &txs, &mut again, proposer)
            .unwrap();
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
        state.insert(Account {
            id: a,
            state: AccountState {
                balance: 1_000 * B,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: z,
            state: AccountState {
                balance: 0,
                ..Default::default()
            },
        });
        // second call overdraws: the whole batch must revert (b gets nothing)
        let batch = Transaction {
            nonce: 0,
            sender: a,
            payload: TransactionPayload::Batch {
                calls: vec![
                    BatchCall::Transfer { to: b, amount: 100 },
                    BatchCall::Transfer {
                        to: c,
                        amount: 10_000 * B,
                    },
                ],
            },
            access_list: AccessList::new(vec![a, b, c], vec![a, b, c]),
        };
        // z has a zero balance: its failing tx is included, nonce advances, nothing to charge.
        let zt = tx(z, b, 0, 1);
        let (_g, r) = exec
            .execute_block(1, 0, &[batch, zt], &mut state, proposer)
            .expect("block executes");
        assert!(!r[0].success && !r[1].success);
        assert!(
            state.get(&b).is_none() && state.get(&c).is_none(),
            "batch reverted atomically"
        );
        assert_eq!(state.get(&a).unwrap().balance, 1_000 * B - fee);
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
        assert!(parallel_transfer_batch_is_safe(
            &[0, 1],
            &[tx(a, b, 0, 1), tx(c, d, 0, 1)]
        ));
        let mut bad = tx(a, b, 0, 1);
        bad.access_list = AccessList::new(vec![a], vec![a]);
        assert!(!parallel_transfer_batch_is_safe(
            &[0, 1],
            &[bad, tx(c, d, 0, 1)]
        ));
        assert!(!parallel_transfer_batch_is_safe(
            &[0, 1],
            &[tx(a, b, 0, 1), tx(c, b, 0, 1)]
        ));
    }

    /// Network fee in base units (chains without history are in base units from genesis).
    fn base_fee(gas: u64, payload: &TransactionPayload, policy: &NetworkFeePolicy) -> u128 {
        boing_tokenomics::network_fee_amount_in(gas, payload, policy, Denomination::BaseUnits)
    }

    const B: u128 = BASE_UNITS_PER_BOING;

    /// 18 decimals: the ledger is multiplied by 10^18 at exactly `base_units_from`, once, and fees and
    /// amounts switch unit with it. Re-executing block H from the H-1 state is deterministic, and the
    /// blocks after H do not multiply again.
    #[test]
    fn redenomination_multiplies_exactly_once_at_h() {
        const H: u64 = 5;
        let mut exec = BlockExecutor::new();
        exec.set_execution_history(Some(crate::history::ExecutionHistory {
            base_units_from: H,
            ..Default::default()
        }));
        let policy = NetworkFeePolicy::treasury_default();
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: a,
            state: AccountState {
                balance: 1_000,
                stake: 7,
                ..Default::default()
            },
        });

        // H - 1: whole BOING.
        let t1 = tx(a, b, 0, 10);
        let (g1, r) = exec
            .execute_block(H - 1, 0, std::slice::from_ref(&t1), &mut state, proposer)
            .unwrap();
        assert!(r[0].success);
        let fee1 = boing_tokenomics::network_fee_amount_in(
            g1,
            &t1.payload,
            &policy,
            Denomination::WholeBoing,
        );
        assert_eq!(fee1, 1, "whole-unit rule: ceil(21000/21000)");
        assert_eq!(state.get(&a).unwrap().balance, 1_000 - 10 - fee1);
        let treasury_h1 = state
            .get(&boing_tokenomics::PROTOCOL_TREASURY)
            .map(|s| s.balance)
            .unwrap_or(0);
        let pre_h = state.snapshot();

        // H: migration, then base units (a fractional fee).
        let t2 = tx(a, b, 1, 3 * B / 2);
        let (g2, r) = exec
            .execute_block(H, 0, std::slice::from_ref(&t2), &mut state, proposer)
            .unwrap();
        assert!(r[0].success);
        let fee2 = exec.network_fee_at(g2, &t2.payload, H);
        assert_eq!(fee2, 21_000 * boing_tokenomics::GAS_PRICE_BASE_UNITS);
        assert_eq!(fee2, 21 * B / 1_000, "0.021 BOING per transfer");
        let a_after_h = (1_000 - 10 - fee1) * B - 3 * B / 2 - fee2;
        assert_eq!(state.get(&a).unwrap().balance, a_after_h);
        assert_eq!(state.get(&a).unwrap().stake, 7 * B);
        assert_eq!(state.get(&b).unwrap().balance, 10 * B + 3 * B / 2);
        assert_eq!(
            state
                .get(&boing_tokenomics::PROTOCOL_TREASURY)
                .unwrap()
                .balance,
            treasury_h1 * B + fee2
        );
        let root_h = state.state_root();

        // Re-executing H from the H-1 state gives the same root (deterministic, still exactly once).
        let mut again = pre_h.snapshot();
        exec.execute_block(H, 0, std::slice::from_ref(&t2), &mut again, proposer)
            .unwrap();
        assert_eq!(again.state_root(), root_h);

        // H + 1: no second multiplication.
        assert!(!exec.prepare_state_for_height(H + 1, &mut state.snapshot()));
        let t3 = tx(a, b, 2, 1);
        let (g3, r) = exec
            .execute_block(H + 1, 0, std::slice::from_ref(&t3), &mut state, proposer)
            .unwrap();
        assert!(r[0].success);
        let fee3 = exec.network_fee_at(g3, &t3.payload, H + 1);
        assert_eq!(state.get(&a).unwrap().balance, a_after_h - 1 - fee3);
        assert_eq!(state.get(&b).unwrap().balance, 10 * B + 3 * B / 2 + 1);
        assert_eq!(state.get(&a).unwrap().stake, 7 * B);
    }

    #[test]
    fn denomination_and_emission_follow_the_gate() {
        let mut exec = BlockExecutor::new();
        assert_eq!(
            exec.denomination_at(0),
            Denomination::BaseUnits,
            "no history: base units from genesis"
        );
        assert_eq!(exec.redenomination_height(), None);
        exec.set_execution_history(Some(crate::history::ExecutionHistory {
            base_units_from: 1_050,
            ..Default::default()
        }));
        assert_eq!(exec.redenomination_height(), Some(1_050));
        assert_eq!(exec.denomination_at(1_049), Denomination::WholeBoing);
        assert_eq!(exec.denomination_at(1_050), Denomination::BaseUnits);
        assert_eq!(
            exec.block_emission(1_049),
            boing_tokenomics::block_emission_validators(1_049)
        );
        let e = exec.block_emission(1_050);
        assert!(
            e > 3 * B && e < 4 * B,
            "base-unit emission is the exact ~3.55 BOING, got {e}"
        );
        assert_eq!(exec.vm().denomination_at(1_049), Denomination::WholeBoing);
        assert_eq!(exec.vm().denomination_at(1_050), Denomination::BaseUnits);
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
            state: AccountState {
                balance: 100_000 * B,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: b,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let txs = vec![tx(a, b, 0, 100)];
        let (gas, receipts) = exec
            .execute_block(1, 0, &txs, &mut state, proposer)
            .unwrap();
        assert_eq!(gas, super::super::vm::GAS_PER_TRANSFER);
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].success);
        let fee = base_fee(
            gas,
            &txs[0].payload,
            &boing_tokenomics::NetworkFeePolicy::treasury_default(),
        );
        assert_eq!(state.get(&a).unwrap().balance, 100_000 * B - 100 - fee);
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

    /// Security review #9: a parallel batch in which a later tx touches the treasury lost the fee of an
    /// earlier tx (its view snapshot overwrote the credit). From v4 the batch runs sequentially and every
    /// unit is accounted for; below the activation height the historical result replays unchanged.
    #[test]
    fn fees_conserved_when_batch_touches_treasury_from_v4() {
        let treasury = boing_tokenomics::PROTOCOL_TREASURY;
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let proposer = AccountId::from_bytes([9u8; 32]);
        let run = |v4_from: u64| {
            let mut exec = BlockExecutor::new();
            exec.set_execution_history(Some(crate::history::ExecutionHistory {
                security_v4_from: v4_from,
                ..Default::default()
            }));
            let mut state = StateStore::new();
            state.insert(Account {
                id: a,
                state: AccountState {
                    balance: 100_000 * B,
                    ..Default::default()
                },
            });
            state.insert(Account {
                id: c,
                state: AccountState {
                    balance: 100_000 * B,
                    ..Default::default()
                },
            });
            state.insert(Account {
                id: treasury,
                state: AccountState {
                    balance: 1_000 * B,
                    ..Default::default()
                },
            });
            let total_before: u128 = [a, b, c, treasury, proposer, boing_tokenomics::FEE_BURN_SINK]
                .iter()
                .map(|id| state.get(id).map(|s| s.balance).unwrap_or(0))
                .sum();
            let txs = vec![tx(a, b, 0, 100), tx(c, treasury, 0, 50)];
            let (_, receipts) = exec
                .execute_block(10, 0, &txs, &mut state, proposer)
                .unwrap();
            assert!(receipts.iter().all(|r| r.success));
            let total_after: u128 = [a, b, c, treasury, proposer, boing_tokenomics::FEE_BURN_SINK]
                .iter()
                .map(|id| state.get(id).map(|s| s.balance).unwrap_or(0))
                .sum();
            (
                total_before,
                total_after,
                state.get(&treasury).unwrap().balance,
            )
        };
        let fee = base_fee(
            super::super::vm::GAS_PER_TRANSFER,
            &TransactionPayload::Transfer { to: b, amount: 100 },
            &boing_tokenomics::NetworkFeePolicy::treasury_default(),
        );
        let (before, after, t) = run(10);
        assert_eq!(after, before, "v4: no value lost");
        assert_eq!(t, 1_000 * B + 50 + 2 * fee);
        let (before, after, t) = run(11);
        assert_eq!(
            t,
            1_000 * B + 50 + fee,
            "pre-v4 replay keeps the historical (lossy) result"
        );
        assert_eq!(after, before - fee);
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
        state.insert(Account {
            id: a,
            state: AccountState {
                balance: 100_000 * B,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: b,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: c,
            state: AccountState {
                balance: 100_000 * B,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: d,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        // Independent transfers a->b and c->d — same batch, parallel execution
        let txs = vec![tx(a, b, 0, 100), tx(c, d, 0, 50 * B)];
        let (gas, receipts) = exec
            .execute_block(1, 0, &txs, &mut state, proposer)
            .unwrap();
        assert_eq!(receipts.len(), 2);
        assert!(receipts.iter().all(|r| r.success));
        let fee_each = base_fee(
            super::super::vm::GAS_PER_TRANSFER,
            &txs[0].payload,
            &boing_tokenomics::NetworkFeePolicy::treasury_default(),
        );
        assert_eq!(state.get(&a).unwrap().balance, 100_000 * B - 100 - fee_each);
        assert_eq!(state.get(&b).unwrap().balance, 100);
        assert_eq!(
            state.get(&c).unwrap().balance,
            100_000 * B - 50 * B - fee_each
        );
        assert_eq!(state.get(&d).unwrap().balance, 50 * B);
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
                balance: 100_000 * B,
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
        use boing_primitives::{nonce_derived_contract_address, QaPoolVoteKind};
        use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};
        use boing_tokenomics::PROTOCOL_TREASURY;

        let mut pool = QaPoolGovernanceConfig::production_default();
        pool.min_quorum_votes = 1;
        // This test funds the voter with balance only; disable the production stake gate.
        pool.min_voter_stake = 0;
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
                balance: 1_000_000 * B,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: voter,
            state: AccountState {
                balance: 1_000_000 * B,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        state.insert(Account {
            id: PROTOCOL_TREASURY,
            state: AccountState {
                balance: 100 * B,
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
        let deploy_fee = boing_tokenomics::fee_for_gas_in(
            super::super::vm::GAS_PER_CONTRACT_DEPLOY,
            Denomination::BaseUnits,
        );
        let vote_fee = boing_tokenomics::fee_for_gas_in(
            super::super::vm::GAS_PER_QA_POOL_VOTE,
            Denomination::BaseUnits,
        );
        assert_eq!(
            state.get(&PROTOCOL_TREASURY).unwrap().balance,
            100 * B + deploy_fee + vote_fee - 5 * B
        );
        assert_eq!(
            state.get(&voter).unwrap().balance,
            1_000_000 * B - vote_fee + 5 * B
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
