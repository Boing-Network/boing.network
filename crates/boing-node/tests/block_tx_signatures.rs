//! Security review #16: blocks carry one signature per transaction and every importing node verifies
//! them. A malicious proposer (a legitimate leader) that injects a tx "from" an account it does not
//! control, with execution roots computed correctly, must have its block rejected by honest nodes.
//! (No chain id and no network history here: signatures are required from genesis, legacy format.)

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

const ATTACKER_SINK: AccountId = AccountId([0xaa; 32]);

/// Leader (validator key 1, malicious) and an honest follower; `victim` (key 10) holds funds.
fn net() -> (BoingNode, BoingNode, SigningKey, SigningKey) {
    let v = key(1);
    let vid = idk(&v);
    let victim = key(10);
    let genesis = ChainState::genesis(vid);
    let mk = |local: AccountId, sk: Option<SigningKey>| {
        let mut n = BoingNode::with_validators(vec![vid], local, sk);
        n.chain = ChainState::from_genesis(genesis.clone());
        n.state = boing_state::StateStore::new();
        for id in [idk(&victim), vid] {
            n.state.insert(Account {
                id,
                state: AccountState { balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING, ..Default::default() },
            });
        }
        n
    };
    (mk(vid, Some(v.clone())), mk(AccountId([0x77; 32]), None), v, victim)
}

/// Build a block on `leader`'s tip with correctly computed roots for `txs` (what a malicious
/// proposer can always do) and the given signatures.
fn craft(leader: &BoingNode, txs: Vec<Transaction>, signatures: Vec<boing_primitives::Signature>) -> Block {
    let height = leader.chain.height() + 1;
    let proposer = leader.consensus.leader(height);
    let exec = BlockExecutor::new();
    let mut state = leader.state.snapshot();
    let (_g, receipts) = exec.execute_block(height, 1, &txs, &mut state, proposer).unwrap();
    let reward = boing_tokenomics::block_emission_validators_in(height, boing_tokenomics::Denomination::BaseUnits);
    if reward > 0 {
        state.get_mut(&proposer).unwrap().balance += reward;
    }
    state.apply_open_validator_block_hook(height, proposer);
    Block {
        header: BlockHeader {
            parent_hash: leader.chain.latest_hash(),
            height,
            timestamp: 1,
            proposer,
            tx_root: tx_root(&txs),
            receipts_root: receipts_root(&receipts),
            state_root: state.state_root(),
            blobs_root: None,
        },
        transactions: txs,
        signatures,
    }
}

#[test]
fn honest_blocks_carry_signatures_and_import() {
    let (mut leader, mut follower, _v, victim) = net();
    let tx = transfer(idk(&victim), 0, AccountId([2; 32]), 5);
    leader.submit_transaction(SignedTransaction::new(tx, &victim)).unwrap();
    leader.produce_block_if_ready().expect("block");
    let b = leader.chain.get_block_by_height(1).unwrap();
    assert_eq!(b.signatures.len(), 1, "produced blocks carry the tx signatures");
    follower.import_network_block_catchup(&b).expect("honest block imports");
    assert_eq!(follower.chain.latest_hash(), leader.chain.latest_hash());
    // Signatures survive the bincode wire/disk encoding.
    let enc = bincode::serialize(&b).unwrap();
    assert_eq!(Block::from_bincode(&enc).unwrap(), b);
}

#[test]
fn malicious_proposer_forged_tx_is_rejected() {
    let (leader, mut follower, v, victim) = net();
    let victim_id = idk(&victim);
    let before = follower.state.get(&victim_id).unwrap().balance;
    // The proposer moves the victim's funds to itself and signs with its own key.
    let steal = transfer(victim_id, 0, ATTACKER_SINK, 900_000);
    let forged_sig = boing_primitives::sign_transaction(&steal, &v);
    let forged = craft(&leader, vec![steal.clone()], vec![forged_sig]);
    match follower.import_network_block_catchup(&forged) {
        Err(BlockValidationError::InvalidTxSignature { index: 0, .. }) => {}
        other => panic!("forged block must be rejected for its signature, got {other:?}"),
    }
    match follower.handle_network_block(&forged) {
        Err(BlockValidationError::InvalidTxSignature { .. }) => {}
        other => panic!("gossiped forged block must be rejected, got {other:?}"),
    }
    // Same tx with no signatures at all (old block format).
    let unsigned = craft(&leader, vec![steal.clone()], vec![]);
    assert!(matches!(
        follower.import_network_block_catchup(&unsigned),
        Err(BlockValidationError::MissingTxSignatures { expected: 1, found: 0 })
    ));
    // Garbage signature bytes.
    let garbage = craft(&leader, vec![steal], vec![boing_primitives::Signature([0u8; 64])]);
    assert!(follower.import_network_block_catchup(&garbage).is_err());
    // An honest-looking first tx does not help a forged second one.
    let ok = transfer(victim_id, 0, AccountId([2; 32]), 1);
    let steal2 = transfer(victim_id, 1, ATTACKER_SINK, 900_000);
    let mixed = craft(
        &leader,
        vec![ok.clone(), steal2.clone()],
        vec![boing_primitives::sign_transaction(&ok, &victim), boing_primitives::sign_transaction(&steal2, &v)],
    );
    assert!(matches!(
        follower.import_network_block_catchup(&mixed),
        Err(BlockValidationError::InvalidTxSignature { index: 1, .. })
    ));
    // Honest node unchanged.
    assert_eq!(follower.chain.height(), 0);
    assert_eq!(follower.state.get(&victim_id).unwrap().balance, before);
    assert!(follower.state.get(&ATTACKER_SINK).is_none());
}
