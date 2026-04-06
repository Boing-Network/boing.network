//! Boing Blockchain — Core Primitives
//!
//! Types, hashing, signatures, and shared data structures.

pub mod hash;
pub mod hd;
pub mod intent;
pub mod paymaster;
pub mod randomness;
pub mod recovery;
pub mod signature;
pub mod types;

pub use hash::{Hash, hasher};
pub use signature::{
    sign_transaction, signable_transaction_hash, verify_signature, Signature, SignatureError,
    SignedTransaction,
};
pub use types::{
    contract_deploy_init_body, contract_deploy_uses_init_code, create2_contract_address,
    nonce_derived_contract_address, receipt_leaf_hash, receipts_root, tx_root, AccountId, Block,
    BlockHeader, ExecutionLog, ExecutionReceipt, Transaction, TransactionPayload, AccessList,
    CONTRACT_DEPLOY_INIT_CODE_MARKER, MAX_EXECUTION_LOG_DATA_BYTES, MAX_EXECUTION_LOGS_PER_TX,
    MAX_EXECUTION_LOG_TOPICS, MAX_RECEIPT_ERROR_STRING_BYTES, MAX_RECEIPT_RETURN_DATA_BYTES,
};
pub use types::{Account, AccountState};
pub use intent::{Intent, IntentKind, SignedIntent};
pub use randomness::{dummy_vrf_output, leader_from_vrf, VdfOutput, VrfOutput};
pub use paymaster::{PaymasterConfig, SponsoredTransaction};
pub use recovery::{Guardian, RecoveryRequest};
pub use hd::HdPath;

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_hash_deterministic() {
        let mut h1 = hasher();
        h1.update(b"hello");
        let out1 = h1.finalize();
        let mut h2 = hasher();
        h2.update(b"hello");
        let out2 = h2.finalize();
        assert_eq!(out1.as_bytes(), out2.as_bytes());
    }

    #[test]
    fn test_transaction_id_deterministic() {
        let id = AccountId::from_bytes([1u8; 32]);
        let tx = Transaction {
            nonce: 0,
            sender: id,
            payload: TransactionPayload::Transfer {
                to: id,
                amount: 100,
            },
            access_list: AccessList::default(),
        };
        assert_eq!(tx.id(), tx.id());
    }

    #[test]
    fn test_signed_transaction_verify() {
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId::from_bytes(key.verifying_key().to_bytes());
        let tx = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::Transfer {
                to: AccountId::from_bytes([2u8; 32]),
                amount: 50,
            },
            access_list: AccessList::default(),
        };
        let signed = SignedTransaction::new(tx, &key);
        assert!(signed.verify().is_ok());
    }

    #[test]
    fn test_access_list_conflicts() {
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let al1 = AccessList::new(vec![a, b], vec![]);
        let al2 = AccessList::new(vec![c], vec![]);
        assert!(!al1.conflicts_with(&al2));
        let al3 = AccessList::new(vec![b, c], vec![]);
        assert!(al1.conflicts_with(&al3));
    }

    #[test]
    fn contract_deploy_init_marker_helpers() {
        assert!(!contract_deploy_uses_init_code(&[0x00]));
        assert!(contract_deploy_uses_init_code(&[CONTRACT_DEPLOY_INIT_CODE_MARKER, 0x00]));
        assert_eq!(contract_deploy_init_body(&[0x60, 0]), &[0x60, 0]);
        assert_eq!(
            contract_deploy_init_body(&[CONTRACT_DEPLOY_INIT_CODE_MARKER, 0x60, 0]),
            &[0x60, 0]
        );
    }

    #[test]
    fn test_create2_contract_address_deterministic() {
        let d = AccountId::from_bytes([3u8; 32]);
        let salt = [5u8; 32];
        let code = vec![0x60, 0x00];
        let a = create2_contract_address(&d, &salt, &code);
        let b = create2_contract_address(&d, &salt, &code);
        assert_eq!(a, b);
        assert_ne!(create2_contract_address(&d, &salt, &vec![0x61]), a);
    }

    #[test]
    fn test_suggested_parallel_access_list() {
        let sender = AccountId::from_bytes([1u8; 32]);
        let to = AccountId::from_bytes([2u8; 32]);
        let contract = AccountId::from_bytes([4u8; 32]);
        let t_transfer = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::Transfer { to, amount: 1 },
            access_list: AccessList::default(),
        };
        let sug = t_transfer.suggested_parallel_access_list();
        assert!(sug.read.contains(&sender) && sug.read.contains(&to));
        assert!(!t_transfer.access_list_covers_parallel_suggestion());
        let t_ok = Transaction {
            access_list: sug.clone(),
            ..t_transfer.clone()
        };
        assert!(t_ok.access_list_covers_parallel_suggestion());
        let t_call = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractCall {
                contract,
                calldata: vec![],
            },
            access_list: AccessList::new(vec![sender, contract], vec![sender, contract]),
        };
        assert!(t_call.access_list_covers_parallel_suggestion());
    }

    #[test]
    fn test_suggested_parallel_access_list_create2_includes_predicted_contract() {
        let sender = AccountId::from_bytes([7u8; 32]);
        let salt = [9u8; 32];
        let bytecode = vec![0x60u8, 0x00];
        let predicted = create2_contract_address(&sender, &salt, &bytecode);
        let t = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: bytecode.clone(),
                create2_salt: Some(salt),
            },
            access_list: AccessList::default(),
        };
        let sug = t.suggested_parallel_access_list();
        assert!(sug.read.contains(&sender));
        assert!(sug.read.contains(&predicted));
        assert!(sug.write.contains(&sender));
        assert!(sug.write.contains(&predicted));
    }
}
