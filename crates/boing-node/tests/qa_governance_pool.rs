//! Governance QA pool: only listed administrators vote; Unsure deploy reaches mempool after admin Allow.

use boing_node::mempool::MempoolError;
use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, SignedTransaction, Transaction,
    TransactionPayload,
};
use boing_qa::pool::{PoolError, QaPoolVote};
use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

#[test]
fn unsure_deploy_only_governance_admin_can_admit() {
    let mut node = BoingNode::new();
    let key = SigningKey::generate(&mut OsRng);
    let deployer = AccountId(key.verifying_key().to_bytes());
    node.state.insert(Account {
        id: deployer,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });

    let admin = AccountId([42u8; 32]);
    let mut pool_cfg = QaPoolGovernanceConfig::development_default();
    pool_cfg.dev_open_voting = false;
    pool_cfg.administrators = vec![format!("0x{}", hex::encode(admin.0))];
    let reg = RuleRegistry::new().with_always_review_categories(vec!["meme".to_string()]);
    node.set_qa_policy(reg, pool_cfg);

    let tx = Transaction {
        nonce: 0,
        sender: deployer,
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
    let signed = SignedTransaction::new(tx, &key);
    let tx_hash = signed.tx.id();

    assert!(matches!(
        node.submit_transaction(signed),
        Err(MempoolError::QaPendingPool(h)) if h == tx_hash
    ));

    let rando = AccountId([99u8; 32]);
    assert!(matches!(
        node.qa_pool_vote(tx_hash, rando, QaPoolVote::Allow),
        Err(PoolError::NotAdministrator)
    ));

    let r = node
        .qa_pool_vote(tx_hash, admin, QaPoolVote::Allow)
        .unwrap();
    assert!(matches!(
        r,
        boing_node::node::QaPoolVoteResult::AllowedAdmitted
    ));
    assert_eq!(node.mempool.len(), 1);
}

/// Production public membership enqueues Unsure deploys (no administrators required).
#[test]
fn production_pool_config_enqueues_unsure_with_public_membership() {
    let mut node = BoingNode::new();
    let key = SigningKey::generate(&mut OsRng);
    let deployer = AccountId(key.verifying_key().to_bytes());
    node.state.insert(Account {
        id: deployer,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });

    node.set_qa_policy(
        RuleRegistry::new().with_always_review_categories(vec!["meme".to_string()]),
        QaPoolGovernanceConfig::production_default(),
    );

    let tx = Transaction {
        nonce: 0,
        sender: deployer,
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
    let signed = SignedTransaction::new(tx, &key);

    assert!(matches!(
        node.submit_transaction(signed),
        Err(MempoolError::QaPendingPool(_))
    ));
}

/// Regression: applying a QA policy at runtime (operator RPC, governance, `--qa-registry`) used to
/// rebuild the executor / VM and silently reset activation heights set via `enable_*`, which would
/// fork a running node at the next gated block.
#[test]
fn set_qa_policy_preserves_activation_heights() {
    let mut node = BoingNode::new();
    node.enable_media_blobs(1200);
    node.enable_blake3_opcode(4242);
    node.enable_governance_consensus_txs(5151);
    node.executor.set_registration_proof_rule(Some(6913), 77);
    node.vm.set_registration_proof_rule(Some(6913), 77);
    let before_exec = node.executor.registration_proof_rule();

    let reg = RuleRegistry::new().with_always_review_categories(vec!["meme".to_string()]);
    let mut pool_cfg = QaPoolGovernanceConfig::development_default();
    pool_cfg.public_membership = true;
    node.set_qa_policy(reg, pool_cfg);

    assert_eq!(node.executor.vm().media_blobs_from(), Some(1200));
    assert_eq!(node.vm.media_blobs_from(), Some(1200));
    assert_eq!(node.executor.vm().blake3_from(), Some(4242));
    assert_eq!(node.vm.blake3_from(), Some(4242));
    assert_eq!(node.executor.vm().governance_consensus_from(), Some(5151));
    assert_eq!(node.executor.registration_proof_rule(), before_exec);
    // New policy is in effect on both the block executor and the node VM.
    assert!(node
        .executor
        .vm()
        .qa_registry()
        .always_review_categories()
        .contains("meme"));
    assert!(node.vm.qa_pool_config().public_membership);
    assert!(node.executor.vm().qa_pool_config().public_membership);
    assert!(node.admission_vm().qa_pool_config().public_membership);
}
