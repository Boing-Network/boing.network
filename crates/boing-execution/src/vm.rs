//! Boing VM — deterministic execution engine.

use boing_primitives::{
    contract_deploy_init_body, contract_deploy_uses_init_code, create2_contract_address,
    nonce_derived_contract_address, AccountId, AccountState, ExecutionLog, Transaction,
    TransactionPayload,
};
use boing_qa::{check_contract_deploy_full, check_contract_deploy_full_with_metadata, QaResult, RuleRegistry};
use boing_state::StateStore;

use crate::gas::base;
use super::interpreter::Interpreter;

/// Gas used by a transaction.
pub const GAS_PER_TRANSFER: u64 = 21_000;
pub const GAS_PER_CONTRACT_CALL: u64 = 100_000;
pub const GAS_PER_CONTRACT_DEPLOY: u64 = 200_000;

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
}

impl Vm {
    pub fn new() -> Self {
        Self {
            qa_registry: default_qa_registry(),
        }
    }

    /// Use the same [RuleRegistry] as the node mempool so execution rejects the same deploys as admission.
    pub fn with_qa_registry(qa_registry: RuleRegistry) -> Self {
        Self { qa_registry }
    }

    /// Reference to the QA registry used for contract deploy execution checks.
    pub fn qa_registry(&self) -> &RuleRegistry {
        &self.qa_registry
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

    /// Execute a single transaction against the state.
    pub fn execute(&self, tx: &Transaction, state: &mut StateStore) -> Result<VmExecutionResult, VmError> {
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
                let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
                sender_state.stake = sender_state
                    .stake
                    .checked_sub(*amount)
                    .ok_or(VmError::InsufficientBalance)?;
                sender_state.balance = sender_state.balance.saturating_add(*amount);
                sender_state.nonce = sender_state
                    .nonce
                    .checked_add(1)
                    .ok_or(VmError::NonceOverflow)?;
                base::UNBOND
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
                            },
                        });
                    }
                }
                GAS_PER_TRANSFER
            }
            TransactionPayload::ContractCall { contract, calldata } => {
                return self.execute_contract_call(state, tx, contract, calldata);
            }
            TransactionPayload::ContractDeploy { .. }
            | TransactionPayload::ContractDeployWithPurpose { .. }
            | TransactionPayload::ContractDeployWithPurposeAndMetadata { .. } => {
                return self.execute_contract_deploy(state, tx);
            }
        };
        Ok(VmExecutionResult {
            gas_used,
            return_data: Vec::new(),
            logs: Vec::new(),
        })
    }

    fn execute_contract_deploy(&self, state: &mut StateStore, tx: &Transaction) -> Result<VmExecutionResult, VmError> {
        // Defense in depth: same full QA path as mempool (`Mempool::insert`), using payload fields.
        // Malicious blocks must not apply deploys that honest mempools would reject.
        let Some((bytecode, purpose, desc_hash, asset_name, asset_symbol)) = tx.payload.as_contract_deploy()
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
                // Aligns with mempool: Unsure is not a hard reject. Community QA pool admits some Unsure
                // txs; at execution we only enforce Allow vs Reject. Block validity / honest producers
                // are the gate for whether an Unsure deploy was properly accepted off-chain.
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
        if state.get(&contract_addr).is_some() || state.get_contract_code(&contract_addr).is_some() {
            return Err(VmError::DeploymentAddressInUse);
        }

        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;

        state.insert(boing_primitives::Account {
            id: contract_addr,
            state: boing_primitives::AccountState { balance: 0, nonce: 0, stake: 0 },
        });

        let uses_init = contract_deploy_uses_init_code(bytecode);
        let init_body = contract_deploy_init_body(bytecode);
        let (gas_used, logs, stored_code) = if uses_init {
            let mut interpreter = Interpreter::new(init_body.to_vec(), GAS_PER_CONTRACT_CALL);
            let g_init =
                interpreter.run_with_qa(tx.sender, contract_addr, &[], state, &self.qa_registry)?;
            let runtime = interpreter.return_data.take().unwrap_or_default();
            let logs = std::mem::take(&mut interpreter.logs);
            let gas = GAS_PER_CONTRACT_DEPLOY.saturating_add(g_init);
            (gas, logs, runtime)
        } else {
            (
                GAS_PER_CONTRACT_DEPLOY,
                Vec::new(),
                bytecode.to_vec(),
            )
        };

        state.set_contract_code(contract_addr, stored_code);
        Ok(VmExecutionResult {
            gas_used,
            return_data: Vec::new(),
            logs,
        })
    }

    fn execute_contract_call(
        &self,
        state: &mut StateStore,
        tx: &Transaction,
        contract: &AccountId,
        calldata: &[u8],
    ) -> Result<VmExecutionResult, VmError> {
        let sender_state = state.get_mut(&tx.sender).ok_or(VmError::AccountNotFound)?;
        sender_state.nonce = sender_state
            .nonce
            .checked_add(1)
            .ok_or(VmError::NonceOverflow)?;

        let code = state.get_contract_code(contract).ok_or(VmError::AccountNotFound)?.clone();
        let mut interpreter = Interpreter::new(code, GAS_PER_CONTRACT_CALL);
        let gas_used =
            interpreter.run_with_qa(tx.sender, *contract, calldata, state, &self.qa_registry)?;
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
    use boing_primitives::{
        AccessList, Account, AccountState, CONTRACT_DEPLOY_INIT_CODE_MARKER,
    };
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
        assert_eq!(state.get_contract_code(&contract).unwrap().as_slice(), &[0x00]);
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
            },
        });

        let salt = [1u8; 32];
        // Factory: MSTORE 0,0 (runtime byte 0x00 at mem[0]); CREATE2 with offset 0 size 1.
        let mut factory = Vec::new();
        factory.extend([Opcode::Push1 as u8, 0x00, Opcode::Push1 as u8, 0x00, Opcode::MStore as u8]);
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
        assert_eq!(state.get(&factory_addr).unwrap().nonce, 0, "factory nonce unchanged");
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("Account not found")]
    AccountNotFound,
    #[error("Insufficient balance")]
    InsufficientBalance,
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
    #[error("In-contract CREATE2 is not available for this storage backend")]
    Create2NotSupported,
    #[error("CREATE2 init code size is zero")]
    Create2InitCodeEmpty,
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
        state: boing_primitives::AccountState { balance: 0, nonce: 0, stake: 0 },
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
