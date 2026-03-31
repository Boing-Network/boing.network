//! Test disk persistence: run node, produce block, restart, verify state restored.

use boing_node::chain::ChainState;
use boing_node::persistence::Persistence;
use boing_primitives::{Account, AccountId, AccountState, Block};
use boing_qa::{QaPoolGovernanceConfig, RuleRegistry};
use boing_state::StateStore;

#[test]
fn test_persistence_roundtrip() {
    let temp = std::env::temp_dir().join("boing-persistence-test");
    let _ = std::fs::remove_dir_all(&temp);

    let proposer = AccountId([1u8; 32]);
    let genesis = ChainState::genesis(proposer);
    let chain = ChainState::from_genesis(genesis.clone());

    let mut state = StateStore::new();
    state.insert(Account {
        id: proposer,
        state: AccountState {
            balance: 1_000_000,
            nonce: 0,
            stake: 0,
        },
    });

    let p = Persistence::new(&temp);
    p.ensure_dirs().unwrap();
    p.save_block(&genesis).unwrap();
    p.save_chain_meta(0, genesis.hash()).unwrap();
    p.save_state(&state).unwrap();

    let block1 = Block {
        header: boing_primitives::BlockHeader {
            parent_hash: genesis.hash(),
            height: 1,
            timestamp: 1,
            proposer,
            tx_root: boing_primitives::Hash::ZERO,
            receipts_root: boing_primitives::Hash::ZERO,
            state_root: boing_primitives::Hash::ZERO,
        },
        transactions: vec![],
    };
    chain.append(block1.clone()).unwrap();
    state.get_mut(&proposer).unwrap().nonce = 1;
    state.get_mut(&proposer).unwrap().balance = 999_900;

    p.save_block(&block1).unwrap();
    p.save_chain_meta(1, block1.hash()).unwrap();
    p.save_state(&state).unwrap();

    let chain2 = p.load_chain().unwrap().expect("chain");
    let state2 = p.load_state().unwrap().expect("state");

    assert_eq!(chain2.height(), 1);
    assert_eq!(chain2.latest_hash(), block1.hash());
    assert_eq!(state2.get(&proposer).unwrap().balance, 999_900);
    assert_eq!(state2.get(&proposer).unwrap().nonce, 1);
}

#[test]
fn test_qa_config_json_roundtrip() {
    let temp = std::env::temp_dir().join("boing-qa-persist-test");
    let _ = std::fs::remove_dir_all(&temp);
    let p = Persistence::new(&temp);
    p.ensure_dirs().unwrap();

    let reg = RuleRegistry::new().with_max_bytecode_size(12345);
    p.save_qa_registry(&reg).unwrap();
    let reg2 = p.load_qa_registry().unwrap().expect("registry");
    assert_eq!(reg2.max_bytecode_size(), 12345);

    let mut pool = QaPoolGovernanceConfig::development_default();
    pool.max_pending_items = 99;
    p.save_qa_pool_config(&pool).unwrap();
    let pool2 = p.load_qa_pool_config().unwrap().expect("pool");
    assert_eq!(pool2.max_pending_items, 99);
}
