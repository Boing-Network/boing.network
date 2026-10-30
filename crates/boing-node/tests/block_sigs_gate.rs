//! Security review #16 height gate (own test binary: sets process env). Below the activation height a
//! block without signatures still imports (history replays exactly); from it, a block must carry a
//! valid signature per tx in the format required at that height (v2 for the chain here), so missing,
//! forged, legacy and other-chain signatures are rejected.

use boing_execution::BlockExecutor;
use boing_node::block_validation::BlockValidationError;
use boing_node::chain::ChainState;
use boing_node::node::BoingNode;
use boing_primitives::{
    receipts_root, tx_root, AccessList, Account, AccountId, AccountState, Block, BlockHeader,
    SignedTransaction, Transaction, TransactionPayload,
};
use ed25519_dalek::SigningKey;

fn key(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}
fn idk(k: &SigningKey) -> AccountId {
    AccountId(k.verifying_key().to_bytes())
}
fn transfer(sender: AccountId, nonce: u64, to: AccountId, amount: u128) -> Transaction {
    Transaction {
        nonce,
        sender,
        payload: TransactionPayload::Transfer { to, amount },
        access_list: AccessList::new(vec![sender, to], vec![sender, to]),
    }
}

fn craft(
    leader: &BoingNode,
    txs: Vec<Transaction>,
    signatures: Vec<boing_primitives::Signature>,
) -> Block {
    let height = leader.chain.height() + 1;
    let proposer = leader.consensus.leader(height);
    let timestamp = leader.chain.tip_timestamp().saturating_add(1);
    let mut state = leader.state.snapshot();
    let (_g, receipts) = BlockExecutor::new()
        .execute_block(height, timestamp, &txs, &mut state, proposer)
        .unwrap();
    let reward = boing_tokenomics::block_emission_validators_in(
        height,
        boing_tokenomics::Denomination::BaseUnits,
    );
    if reward > 0 {
        state.get_mut(&proposer).unwrap().balance += reward;
    }
    state.apply_open_validator_block_hook(height, proposer);
    Block {
        header: BlockHeader {
            parent_hash: leader.chain.latest_hash(),
            height,
            timestamp,
            proposer,
            tx_root: tx_root(&txs),
            receipts_root: receipts_root(&receipts),
            state_root: state.state_root(),
            blobs_root: None,
        },
        transactions: txs,
        signatures,
        priority_tips: vec![],
    }
}

#[test]
fn block_signatures_follow_the_activation_height() {
    std::env::set_var("BOING_CHAIN_ID", "6913");
    std::env::set_var("BOING_TX_V2_FROM", "1");
    std::env::set_var("BOING_BLOCK_SIGS_FROM", "3");
    let v = key(1);
    let vid = idk(&v);
    let user = key(10);
    let uid = idk(&user);
    let genesis = ChainState::genesis(vid);
    let mk = |local: AccountId, sk: Option<SigningKey>| {
        let mut n = BoingNode::with_validators(vec![vid], local, sk);
        n.chain = ChainState::from_genesis(genesis.clone());
        n.state = boing_state::StateStore::new();
        for id in [uid, vid] {
            n.state.insert(Account {
                id,
                state: AccountState {
                    balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
                    ..Default::default()
                },
            });
        }
        n
    };
    let mut leader = mk(vid, Some(v.clone()));
    // #14: with a chain id and no network history the chain-bound registration proof applies from
    // genesis, identically for block execution and for admission/simulation.
    assert_eq!(leader.executor.registration_proof_rule(), (Some(6913), 0));
    assert_eq!(
        leader.admission_vm().registration_proof_rule(),
        (Some(6913), 0)
    );
    let mut follower = mk(AccountId([0x77; 32]), None);
    let to = AccountId([2; 32]);
    let v2 = |tx: &Transaction| boing_primitives::sign_transaction_v2(tx, &user, 6913);

    // Heights 1 and 2 (before activation): a block without signatures imports, as history does.
    for nonce in 0..2u64 {
        let tx = transfer(uid, nonce, to, 1);
        let unsigned = craft(&leader, vec![tx.clone()], vec![]);
        follower
            .import_network_block_catchup(&unsigned)
            .expect("pre-activation unsigned block");
        leader.import_network_block_catchup(&unsigned).unwrap();
    }
    assert_eq!(follower.chain.height(), 2);

    // Height 3 (activation): missing, forged, legacy-format and other-chain signatures are rejected.
    let tx = transfer(uid, 2, to, 1);
    let cases: Vec<(&str, Vec<boing_primitives::Signature>)> = vec![
        ("missing", vec![]),
        (
            "forged by the proposer",
            vec![boing_primitives::sign_transaction_v2(&tx, &v, 6913)],
        ),
        (
            "legacy format",
            vec![boing_primitives::sign_transaction(&tx, &user)],
        ),
        (
            "other chain",
            vec![boing_primitives::sign_transaction_v2(&tx, &user, 6914)],
        ),
    ];
    for (what, sigs) in cases {
        let b = craft(&leader, vec![tx.clone()], sigs);
        assert!(
            matches!(
                follower.import_network_block_catchup(&b),
                Err(BlockValidationError::MissingTxSignatures { .. }
                    | BlockValidationError::InvalidTxSignature { .. })
            ),
            "{what} signature must be rejected at activation"
        );
        assert_eq!(follower.chain.height(), 2);
    }
    // A correctly signed block imports.
    let good = craft(&leader, vec![tx.clone()], vec![v2(&tx)]);
    follower
        .import_network_block_catchup(&good)
        .expect("valid v2 signatures import");
    leader.import_network_block_catchup(&good).unwrap();

    // The real producer path: a block produced by the leader carries v2 signatures and imports.
    let tx = transfer(uid, 3, to, 1);
    leader
        .submit_transaction(SignedTransaction::new_for_chain(tx, &user, 6913))
        .unwrap();
    leader.produce_block_if_ready().expect("block");
    let b = leader.chain.get_block_by_height(4).unwrap();
    assert_eq!(b.signatures.len(), 1);
    follower
        .import_network_block_catchup(&b)
        .expect("produced block imports");
    assert_eq!(follower.chain.latest_hash(), leader.chain.latest_hash());
}
