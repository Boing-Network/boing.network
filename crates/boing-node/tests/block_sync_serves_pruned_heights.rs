//! A long-running node prunes old heights from RAM (`DEFAULT_IN_MEMORY_BLOCK_WINDOW`). Block-sync must
//! still serve them from disk, or a fresh node can never get past height 0 once the chain is longer
//! than the window (public testnet, 2026-11).

use boing_node::chain::ChainState;
use boing_node::persistence::Persistence;
use boing_node::ChainBlockProvider;
use boing_p2p::BlockProvider;
use boing_primitives::{AccountId, Block, BlockHeader, Hash};

#[test]
fn provider_falls_back_to_disk_for_heights_outside_memory_window() {
    let dir = std::env::temp_dir().join(format!(
        "boing-sync-pruned-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let persistence = Persistence::new(&dir);
    let proposer = AccountId([1u8; 32]);
    let chain = ChainState::from_genesis_with_window(ChainState::genesis(proposer), 3);
    let mut parent = chain.latest_hash();
    for h in 1..=10u64 {
        let block = Block {
            header: BlockHeader {
                parent_hash: parent,
                height: h,
                timestamp: h,
                proposer,
                tx_root: Hash::ZERO,
                receipts_root: Hash::ZERO,
                state_root: Hash::ZERO,
                blobs_root: None,
            },
            transactions: vec![],
            signatures: vec![],
            priority_tips: vec![],
        };
        parent = block.hash();
        persistence.save_block(&block).unwrap();
        chain.append(block).unwrap();
    }
    assert!(
        chain.get_block_by_height(1).is_none(),
        "height 1 pruned from RAM"
    );

    let memory_only = ChainBlockProvider::new(chain.clone());
    assert!(memory_only.get_block_by_height(1).is_none());

    let provider = ChainBlockProvider::with_persistence(chain, Some(persistence));
    for h in 1..=10u64 {
        let b = provider
            .get_block_by_height(h)
            .unwrap_or_else(|| panic!("height {h} must be served"));
        assert_eq!(b.header.height, h);
    }
    assert!(provider.get_block_by_height(11).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
