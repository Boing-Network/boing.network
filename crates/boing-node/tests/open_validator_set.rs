//! Open validator-set mode end to end (in-process network, blocks and signed votes routed by hand):
//! registration rules, epoch join, liveness jailing + unjail, idle periods, anchors only, an offline
//! open majority, late-leader supersede, deregistration, restart persistence and replay determinism.

use std::collections::HashMap;
use std::time::Duration;

use boing_node::chain::ChainState;
use boing_node::node::BoingNode;
use boing_primitives::{
    sign_validator_registration, AccessList, Account, AccountId, AccountState, Block, ConsensusVote,
    Hash, SignedTransaction, Transaction, TransactionPayload,
};
use boing_state::{OpenValidatorParams, StateStore, ValidatorStatus};
use ed25519_dalek::SigningKey;

fn key(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}
fn idk(k: &SigningKey) -> AccountId {
    AccountId(k.verifying_key().to_bytes())
}

const SPAMMER: u8 = 250;
const SINK: AccountId = AccountId([0xee; 32]);

fn anchor_key(i: usize) -> SigningKey {
    key(1 + i as u8)
}
fn open_key(i: usize) -> SigningKey {
    key(40 + i as u8)
}
fn owner_key(i: usize) -> SigningKey {
    key(120 + i as u8)
}

fn params(anchors: Vec<AccountId>, activation_height: u64) -> OpenValidatorParams {
    OpenValidatorParams {
        activation_height,
        anchors,
        bond: 100,
        deregister_delay: 10,
        epoch_len: 5,
        max_open_validators: 10,
        liveness_window: 50,
        miss_threshold_bps: 1_000,
        min_liveness_samples: 2,
        jail_cooldown: 10,
    }
}

fn funded_ids(opens: usize) -> Vec<AccountId> {
    let mut v = vec![idk(&key(SPAMMER))];
    for i in 0..8 {
        v.push(idk(&owner_key(i)));
    }
    for i in 0..opens.max(4) {
        v.push(idk(&open_key(i)));
    }
    v
}

fn genesis_state(accounts: &[AccountId]) -> StateStore {
    let mut s = StateStore::new();
    for a in accounts {
        s.insert(Account {
            id: *a,
            state: AccountState { balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING, nonce: 0, stake: 0, ..Default::default() },
        });
    }
    s
}

struct Member {
    node: BoingNode,
    key: SigningKey,
    online: bool,
}

struct Net {
    members: Vec<Member>,
    nonces: HashMap<AccountId, u64>,
    signers: HashMap<AccountId, SigningKey>,
    params: Option<OpenValidatorParams>,
}

impl Net {
    /// `anchors` anchor nodes (members 0..a) and `opens` open-candidate nodes (members a..).
    fn new(anchors: usize, opens: usize, open_params: Option<OpenValidatorParams>) -> Self {
        let anchor_ids: Vec<AccountId> = (0..anchors).map(|i| idk(&anchor_key(i))).collect();
        let genesis = ChainState::genesis(anchor_ids[0]);
        let funded = funded_ids(opens);
        let mut members = Vec::new();
        let keys: Vec<SigningKey> = (0..anchors)
            .map(anchor_key)
            .chain((0..opens).map(open_key))
            .collect();
        for k in keys {
            let mut node = BoingNode::with_validators(anchor_ids.clone(), idk(&k), Some(k.clone()));
            node.chain = ChainState::from_genesis(genesis.clone());
            node.state = genesis_state(&funded);
            node.anchor_fallback_timeout = Duration::ZERO;
            if let Some(p) = &open_params {
                node.enable_open_validator_set(p.clone());
            }
            members.push(Member { node, key: k, online: true });
        }
        let mut signers = HashMap::new();
        signers.insert(idk(&key(SPAMMER)), key(SPAMMER));
        for i in 0..8 {
            signers.insert(idk(&owner_key(i)), owner_key(i));
        }
        Net { members, nonces: HashMap::new(), signers, params: open_params }
    }

    fn height(&self) -> u64 {
        self.members[0].node.chain.height()
    }

    fn anchor0(&self) -> &BoingNode {
        &self.members[0].node
    }

    fn idx_of(&self, id: &AccountId) -> Option<usize> {
        self.members.iter().position(|m| idk(&m.key) == *id)
    }

    fn signed(&self, signer: AccountId, payload: TransactionPayload) -> SignedTransaction {
        let nonce = *self.nonces.get(&signer).unwrap_or(&0);
        let access_list = match &payload {
            TransactionPayload::Transfer { to, .. } => AccessList::new(vec![signer, *to], vec![signer, *to]),
            _ => AccessList::new(vec![signer], vec![signer]),
        };
        let tx = Transaction { nonce, sender: signer, payload, access_list };
        SignedTransaction::new(tx, &self.signers[&signer])
    }

    fn filler(&self) -> SignedTransaction {
        self.signed(idk(&key(SPAMMER)), TransactionPayload::Transfer { to: SINK, amount: 1 })
    }

    /// Who proposes the next block: the scheduled leader if online, else the fallback anchor.
    fn next_proposer(&self) -> usize {
        let h = self.height() + 1;
        let c = &self.members[0].node.consensus;
        let leader = c.leader(h);
        if let Some(i) = self.idx_of(&leader) {
            if self.members[i].online {
                return i;
            }
        }
        let fb = c.fallback_anchor(h).expect("anchor mode or static leader online");
        self.idx_of(&fb).expect("fallback anchor is a member")
    }

    /// Admission check only (does not produce).
    fn submit_to_proposer(&mut self, tx: SignedTransaction) -> Result<usize, String> {
        let p = self.next_proposer();
        self.members[p]
            .node
            .submit_transaction(tx.clone())
            .map_err(|e| e.to_string())?;
        *self.nonces.entry(tx.tx.sender).or_insert(0) += 1;
        Ok(p)
    }

    /// Produce + gossip + vote one block containing `tx` (or a filler). Returns the block.
    fn step_with(&mut self, tx: Option<SignedTransaction>) -> Result<Block, String> {
        let tx = tx.unwrap_or_else(|| self.filler());
        let p = self.submit_to_proposer(tx)?;
        self.produce_and_gossip(p)
    }

    fn produce_and_gossip(&mut self, p: usize) -> Result<Block, String> {
        let h = self.height() + 1;
        let _ = self.members[p].node.produce_block_if_ready();
        let block = self.members[p]
            .node
            .pending_commit
            .as_ref()
            .map(|pc| pc.block.clone())
            .or_else(|| self.members[p].node.chain.get_block_by_height(h))
            .ok_or_else(|| format!("member {p} produced nothing at height {h}"))?;
        self.gossip_block(p, &block)?;
        Ok(block)
    }

    fn gossip_block(&mut self, from: usize, block: &Block) -> Result<(), String> {
        let h = block.header.height;
        for i in 0..self.members.len() {
            if i == from || !self.members[i].online {
                continue;
            }
            self.members[i]
                .node
                .handle_network_block(block)
                .map_err(|e| format!("member {i} rejected block {h}: {e}"))?;
        }
        let votes: Vec<(usize, ConsensusVote)> = self
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.online
                    && m.node.last_local_vote_round == Some(h)
                    && m.node.chain.get_block_by_height(h).map(|b| b.hash()) != Some(Hash::ZERO)
            })
            .map(|(i, m)| (i, ConsensusVote::sign(h, block.hash(), &m.key)))
            .collect();
        for (from_i, v) in votes {
            for i in 0..self.members.len() {
                if i != from_i && self.members[i].online {
                    let _ = self.members[i].node.on_consensus_vote(v.clone());
                }
            }
        }
        for (i, m) in self.members.iter().enumerate() {
            if m.online && m.node.chain.height() != h {
                return Err(format!("member {i} at height {} after block {h}", m.node.chain.height()));
            }
        }
        Ok(())
    }

    fn step(&mut self) -> Block {
        self.step_with(None).expect("step")
    }

    fn run_to(&mut self, height: u64) {
        while self.height() < height {
            self.step();
        }
    }

    fn register_tx(&self, owner: usize, node: usize) -> SignedTransaction {
        let owner_id = idk(&owner_key(owner));
        self.signed(
            owner_id,
            TransactionPayload::ValidatorRegister {
                node_id: idk(&open_key(node)),
                node_signature: sign_validator_registration(&owner_id, &open_key(node)),
            },
        )
    }

    fn status(&self, node: usize) -> Option<ValidatorStatus> {
        self.anchor0()
            .state
            .open_validators()
            .records
            .get(&idk(&open_key(node)))
            .map(|r| r.status)
    }

    fn set(&self) -> Vec<AccountId> {
        self.anchor0().consensus.validators().to_vec()
    }

    fn assert_all_agree(&self) {
        let root0 = self.anchor0().chain.get_block_by_height(self.height()).unwrap().header.state_root;
        for (i, m) in self.members.iter().enumerate() {
            if !m.online {
                continue;
            }
            assert_eq!(m.node.consensus.validators(), self.set().as_slice(), "member {i} set");
            assert_eq!(
                m.node.state.open_validators(),
                self.anchor0().state.open_validators(),
                "member {i} registry"
            );
            assert_eq!(
                m.node.chain.get_block_by_height(self.height()).unwrap().header.state_root,
                root0,
                "member {i} root"
            );
        }
    }

    /// Bring an offline member back: import every missing block via the catch-up path.
    fn bring_online(&mut self, i: usize) {
        let tip = self.height();
        let from = self.members[i].node.chain.height() + 1;
        for h in from..=tip {
            let b = self.members[0].node.chain.get_block_by_height(h).unwrap();
            self.members[i].node.import_network_block_catchup(&b).expect("catch-up");
        }
        self.members[i].online = true;
    }
}

#[test]
fn pre_activation_chain_is_identical_to_static_mode() {
    let a = idk(&anchor_key(0));
    let mut open = Net::new(1, 0, Some(params(vec![a], 1_000)));
    let mut stat = Net::new(1, 0, None);
    // New tx variants are refused before activation (old nodes never see them).
    let err = open.step_with(Some(open.register_tx(0, 0))).unwrap_err();
    assert!(err.contains("activates at height 1000"), "{err}");
    let err = stat.step_with(Some(stat.register_tx(0, 0))).unwrap_err();
    assert!(err.contains("not enabled"), "{err}");
    for _ in 0..12 {
        let b1 = open.step();
        let b2 = stat.step();
        assert_eq!(b1.header.state_root, b2.header.state_root, "height {}", b1.header.height);
        assert_eq!(b1.header.tx_root, b2.header.tx_root);
        assert_eq!(b1.header.receipts_root, b2.header.receipts_root);
    }
    assert!(open.anchor0().state.open_validators().is_empty());
    assert_eq!(open.anchor0().consensus.anchors().len(), 0, "legacy quorum until activation");
}

#[test]
fn registration_rules_bad_signatures_and_duplicates() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 2, Some(params(vec![a], 5)));
    net.run_to(5);
    assert_eq!(net.anchor0().state.open_validators().activated_at, Some(5));

    let owner0 = idk(&owner_key(0));
    let owner1 = idk(&owner_key(1));
    let node0 = idk(&open_key(0));
    // Proof signed for a different owner.
    let bad = net.signed(
        owner0,
        TransactionPayload::ValidatorRegister {
            node_id: node0,
            node_signature: sign_validator_registration(&owner1, &open_key(0)),
        },
    );
    let err = net.step_with(Some(bad)).unwrap_err();
    assert!(err.contains("invalid node signature"), "{err}");
    // Proof signed by the wrong key.
    let bad = net.signed(
        owner0,
        TransactionPayload::ValidatorRegister {
            node_id: node0,
            node_signature: sign_validator_registration(&owner0, &open_key(1)),
        },
    );
    assert!(net.step_with(Some(bad)).unwrap_err().contains("invalid node signature"));
    // Garbage signature.
    let bad = net.signed(
        owner0,
        TransactionPayload::ValidatorRegister { node_id: node0, node_signature: vec![1, 2, 3] },
    );
    assert!(net.step_with(Some(bad)).unwrap_err().contains("invalid node signature"));
    // Anchors cannot register.
    let anchor_reg = net.signed(
        owner0,
        TransactionPayload::ValidatorRegister {
            node_id: a,
            node_signature: sign_validator_registration(&owner0, &anchor_key(0)),
        },
    );
    assert!(net.step_with(Some(anchor_reg)).unwrap_err().contains("anchor"));

    let bal_before = net.anchor0().state.get(&owner0).unwrap().balance;
    net.step_with(Some(net.register_tx(0, 0))).unwrap();
    let bal_after = net.anchor0().state.get(&owner0).unwrap().balance;
    assert!(bal_before - bal_after >= 100 * boing_primitives::BASE_UNITS_PER_BOING, "bond locked");
    assert_eq!(net.status(0), Some(ValidatorStatus::Pending));
    assert_eq!(net.anchor0().state.open_validators().records[&node0].bond, 100 * boing_primitives::BASE_UNITS_PER_BOING);
    assert!(!net.set().contains(&node0), "joins only at the next boundary");

    // Duplicate node (other owner) and second node for the same owner.
    let dup = net.signed(
        owner1,
        TransactionPayload::ValidatorRegister {
            node_id: node0,
            node_signature: sign_validator_registration(&owner1, &open_key(0)),
        },
    );
    assert!(net.step_with(Some(dup)).unwrap_err().contains("already registered"));
    let second = net.register_tx(0, 1);
    assert!(net.step_with(Some(second)).unwrap_err().contains("owner already"));
    // Deregister by a non-owner is refused.
    let wrong = net.signed(owner1, TransactionPayload::ValidatorDeregister { node_id: node0 });
    assert!(net.step_with(Some(wrong)).unwrap_err().contains("owner"));
    net.assert_all_agree();
}

#[test]
fn registered_validator_joins_at_next_epoch_and_proposes() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 1, Some(params(vec![a], 5)));
    net.run_to(6);
    net.step_with(Some(net.register_tx(0, 0))).unwrap(); // height 7
    let node0 = idk(&open_key(0));
    for _ in 8..10 {
        net.step();
        assert!(!net.set().contains(&node0));
    }
    net.step(); // height 10 = boundary
    assert_eq!(net.height(), 10);
    assert!(net.set().contains(&node0));
    assert_eq!(net.status(0), Some(ValidatorStatus::Active));
    assert_eq!(net.anchor0().consensus.quorum_size(), 1);
    net.assert_all_agree();
    // Within the next few blocks the open validator proposes its own slot.
    let mut proposed = false;
    for _ in 0..4 {
        if net.step().header.proposer == node0 {
            proposed = true;
        }
    }
    assert!(proposed, "open validator proposed in its slot");
    let report = net.members[1].node.validators_report();
    assert_eq!(report["mode"], "open");
    let v = report["validators"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == format!("0x{}", hex::encode(node0.0)))
        .unwrap()
        .clone();
    assert_eq!(v["status"], "active");
    assert_eq!(v["bond"], "100000000000000000000");
    assert_eq!(v["bond_boing"], "100");
    assert_eq!(v["missed_proposals"], 0);
    assert_eq!(v["uptime_pct"], 100.0);
}

#[test]
fn offline_validator_is_jailed_keeps_bond_and_can_unjail_after_cooldown() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 1, Some(params(vec![a], 5)));
    net.run_to(6);
    net.step_with(Some(net.register_tx(0, 0))).unwrap();
    net.run_to(10);
    let node0 = idk(&open_key(0));
    assert!(net.set().contains(&node0));
    net.members[1].online = false; // goes offline
    net.run_to(15); // anchor proposes the open slots as fallback; chain never stalls
    assert!(matches!(net.status(0), Some(ValidatorStatus::Jailed { until: 25 })), "{:?}", net.status(0));
    assert!(!net.set().contains(&node0));
    assert_eq!(net.anchor0().state.open_validators().records[&node0].bond, 100 * boing_primitives::BASE_UNITS_PER_BOING, "no slashing");
    let report = net.anchor0().validators_report();
    let v = report["validators"].as_array().unwrap().iter().find(|v| v["status"] == "jailed").unwrap().clone();
    assert_eq!(v["jailed_until"], 25);
    assert_eq!(v["jail_count"], 1);
    // Last liveness at jailing is kept and shown (the live window was purged).
    let lj = &v["last_jail"];
    assert_eq!(lj["jailed_at"], 15, "{v}");
    assert!(lj["scheduled_proposals"].as_u64().unwrap() >= 1, "{v}");
    assert_eq!(lj["missed_proposals"], lj["scheduled_proposals"], "{v}");
    assert_eq!(lj["uptime_pct"], 0.0, "{v}");
    assert_eq!(v["scheduled_proposals"], 0, "current window is fresh");

    let owner0 = idk(&owner_key(0));
    let early = net.signed(owner0, TransactionPayload::ValidatorUnjail { node_id: node0 });
    assert!(net.step_with(Some(early)).unwrap_err().contains("cooldown"));
    net.run_to(24);
    net.bring_online(1);
    let unjail = net.signed(owner0, TransactionPayload::ValidatorUnjail { node_id: node0 });
    net.step_with(Some(unjail)).unwrap(); // height 25 >= until; 25 is also a boundary → re-admitted now
    assert_eq!(net.status(0), Some(ValidatorStatus::Active));
    net.run_to(30);
    assert!(net.set().contains(&node0), "re-joined at the next boundary");
    assert_eq!(net.anchor0().state.open_validators().stats(&node0).missed, 0, "fresh liveness window");
    net.assert_all_agree();
}

#[test]
fn online_validator_survives_epochs_and_idle_periods() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 1, Some(params(vec![a], 5)));
    net.run_to(6);
    net.step_with(Some(net.register_tx(0, 0))).unwrap();
    net.run_to(12);
    let node0 = idk(&open_key(0));
    let before = net.anchor0().state.open_validators().stats(&node0);
    // Idle: nobody has transactions, so nobody produces, and no slot is scheduled or missed.
    for _ in 0..50 {
        for m in net.members.iter_mut() {
            assert!(m.node.produce_block_if_ready().is_none());
        }
    }
    assert_eq!(net.height(), 12);
    assert_eq!(net.anchor0().state.open_validators().stats(&node0), before);
    net.run_to(40);
    assert_eq!(net.status(0), Some(ValidatorStatus::Active));
    let s = net.anchor0().state.open_validators().stats(&node0);
    assert!(s.scheduled >= 10);
    assert_eq!(s.missed, 0);
    net.assert_all_agree();
}

#[test]
fn anchors_only_keeps_producing_after_activation() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 0, Some(params(vec![a], 5)));
    net.run_to(23);
    assert_eq!(net.set(), vec![a]);
    assert_eq!(net.anchor0().consensus.anchors(), &[a]);
    assert_eq!(net.anchor0().state.open_validators().activated_at, Some(5));
}

#[test]
fn offline_open_majority_cannot_stall_and_is_removed() {
    let anchors = vec![idk(&anchor_key(0)), idk(&anchor_key(1))];
    let mut net = Net::new(2, 4, Some(params(anchors.clone(), 5)));
    net.run_to(5);
    for i in 0..4 {
        net.step_with(Some(net.register_tx(i, i))).unwrap();
    }
    net.run_to(10);
    assert_eq!(net.set().len(), 6, "2 anchors + 4 open");
    assert_eq!(net.anchor0().consensus.quorum_size(), 2, "both anchors");
    // The whole open majority (4 of 6) goes offline.
    for i in 2..6 {
        net.members[i].online = false;
    }
    let mut fallback_blocks = 0;
    // n = 6, so each open validator is scheduled about once every 6 blocks; by 25 each has >= 2 misses.
    while net.height() < 25 {
        let b = net.step();
        if !anchors.contains(&net.anchor0().consensus.leader(b.header.height)) {
            fallback_blocks += 1;
        }
    }
    assert!(fallback_blocks > 0);
    for i in 0..4 {
        assert!(matches!(net.status(i), Some(ValidatorStatus::Jailed { .. })), "open {i}: {:?}", net.status(i));
    }
    let mut want = anchors.clone();
    want.sort();
    assert_eq!(net.set(), want);
    net.run_to(30);
    net.assert_all_agree();
}

#[test]
fn late_open_leader_is_superseded_without_equivocation() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 1, Some(params(vec![a], 5)));
    net.run_to(6);
    net.step_with(Some(net.register_tx(0, 0))).unwrap();
    net.run_to(10);
    let node0 = idk(&open_key(0));
    // Advance until the open validator is the scheduled leader.
    while net.anchor0().consensus.leader(net.height() + 1) != node0 {
        net.step();
    }
    let h = net.height() + 1;
    // The open leader proposes but its block never reaches the anchor in time.
    let owner1 = idk(&owner_key(1));
    let late_tx = net.signed(owner1, TransactionPayload::Transfer { to: SINK, amount: 1 });
    net.members[1].node.submit_transaction(late_tx).unwrap();
    assert!(net.members[1].node.produce_block_if_ready().is_none());
    assert!(net.members[1].node.pending_commit.is_some());
    // The anchor times out and proposes as fallback; its own vote is the quorum.
    let filler = net.filler();
    net.members[0].node.submit_transaction(filler).unwrap();
    *net.nonces.entry(idk(&key(SPAMMER))).or_insert(0) += 1;
    assert!(net.members[0].node.produce_block_if_ready().is_some());
    let anchor_block = net.anchor0().chain.get_block_by_height(h).unwrap();
    assert_eq!(anchor_block.header.proposer, a);
    // The open node receives the anchor block: supersedes its own, does not vote twice, commits on the anchor vote.
    net.members[1].node.handle_network_block(&anchor_block).unwrap();
    assert_eq!(net.members[1].node.last_local_vote_round, Some(h));
    let v = ConsensusVote::sign(h, anchor_block.hash(), &net.members[0].key);
    net.members[1].node.on_consensus_vote(v);
    assert_eq!(net.members[1].node.chain.height(), h);
    assert_eq!(net.members[1].node.chain.get_block_by_height(h).unwrap().hash(), anchor_block.hash());
    for m in &net.members {
        assert!(m.node.slash_registry.list_slashes().is_empty(), "no equivocation");
    }
    // The missed slot is on-chain for every node.
    assert_eq!(net.anchor0().state.open_validators().stats(&node0).missed, 1);
    net.step();
    net.assert_all_agree();
}

#[test]
fn deregistration_leaves_at_boundary_and_bond_is_claimable_after_delay() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 1, Some(params(vec![a], 5)));
    net.run_to(6);
    net.step_with(Some(net.register_tx(0, 0))).unwrap();
    net.run_to(10);
    let node0 = idk(&open_key(0));
    let owner0 = idk(&owner_key(0));
    let dereg = net.signed(owner0, TransactionPayload::ValidatorDeregister { node_id: node0 });
    let b = net.step_with(Some(dereg)).unwrap();
    let acct = net.anchor0().state.get(&owner0).unwrap().clone();
    assert_eq!(acct.pending_unbond, 100 * boing_primitives::BASE_UNITS_PER_BOING);
    assert_eq!(acct.unbond_unlock_height, b.header.height + 10);
    assert_eq!(net.status(0), Some(ValidatorStatus::Exiting));
    assert!(net.set().contains(&node0), "stays until the boundary");
    net.run_to(15);
    assert!(!net.set().contains(&node0));
    assert_eq!(net.status(0), None);
    net.run_to(acct.unbond_unlock_height);
    let bal = net.anchor0().state.get(&owner0).unwrap().balance;
    net.step_with(Some(net.signed(owner0, TransactionPayload::ClaimUnbond))).unwrap();
    let after = net.anchor0().state.get(&owner0).unwrap().clone();
    assert_eq!(after.pending_unbond, 0);
    assert!(after.balance > bal + 90 * boing_primitives::BASE_UNITS_PER_BOING, "bond returned (minus fee)");
    // The owner may register again.
    net.step_with(Some(net.register_tx(0, 0))).unwrap();
    assert_eq!(net.status(0), Some(ValidatorStatus::Pending));
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "boing-open-validators-{tag}-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn restart_restores_registry_and_set_and_replay_is_deterministic() {
    let a_key = anchor_key(0);
    let a = idk(&a_key);
    let p = params(vec![a], 5);
    let dir = temp_dir("restart");
    let funded = funded_ids(1);
    let genesis = ChainState::genesis(a);

    let make = |dir: &std::path::Path| {
        let mut n = BoingNode::with_data_dir_and_validators(Some(dir), vec![a], a, Some(a_key.clone())).unwrap();
        n.anchor_fallback_timeout = Duration::ZERO;
        n
    };
    let mut n = make(&dir);
    n.state = genesis_state(&funded);
    n.enable_open_validator_set(p.clone());
    let mut net = Net { members: vec![Member { node: n, key: a_key.clone(), online: true }], nonces: HashMap::new(), signers: HashMap::new(), params: Some(p.clone()) };
    net.signers.insert(idk(&key(SPAMMER)), key(SPAMMER));
    net.signers.insert(idk(&owner_key(0)), owner_key(0));
    net.run_to(6);
    net.step_with(Some(net.register_tx(0, 0))).unwrap();
    net.run_to(17); // node0 joins at 10 (offline: never a member here) and is jailed at 15
    let reg_before = net.anchor0().state.open_validators().clone();
    let set_before = net.set();
    let root_before = net.anchor0().chain.get_block_by_height(17).unwrap().header.state_root;
    assert!(matches!(net.status(0), Some(ValidatorStatus::Jailed { .. })));
    let blocks: Vec<Block> = (1..=17).map(|h| net.anchor0().chain.get_block_by_height(h).unwrap()).collect();
    drop(net);

    // Restart from disk.
    let mut n2 = make(&dir);
    n2.enable_open_validator_set(p.clone());
    assert_eq!(n2.chain.height(), 17);
    assert_eq!(n2.state.open_validators(), &reg_before);
    assert_eq!(n2.consensus.validators(), set_before.as_slice());
    assert_eq!(n2.state.state_root(), root_before);
    // And it keeps producing.
    let mut net2 = Net { members: vec![Member { node: n2, key: a_key.clone(), online: true }], nonces: HashMap::new(), signers: HashMap::new(), params: Some(p.clone()) };
    net2.signers.insert(idk(&key(SPAMMER)), key(SPAMMER));
    net2.nonces.insert(idk(&key(SPAMMER)), net2.anchor0().state.get(&idk(&key(SPAMMER))).unwrap().nonce);
    net2.run_to(21);

    // A fresh follower replaying the same blocks derives the identical registry and set.
    let mut f = BoingNode::with_validators(vec![a], idk(&open_key(3)), Some(open_key(3)));
    f.chain = ChainState::from_genesis(genesis.clone());
    f.state = genesis_state(&funded);
    f.enable_open_validator_set(p.clone());
    for b in &blocks {
        f.import_network_block_catchup(b).unwrap();
    }
    assert_eq!(f.state.open_validators(), &reg_before);
    assert_eq!(f.consensus.validators(), set_before.as_slice());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_mode_config_mismatch_is_detected_by_state_root() {
    let a = idk(&anchor_key(0));
    let mut net = Net::new(1, 0, Some(params(vec![a], 5)));
    net.run_to(5);
    let b5 = net.anchor0().chain.get_block_by_height(5).unwrap();
    // A node without open mode (or other params) rejects the activation block.
    let mut other = Net::new(1, 0, None);
    other.run_to(4);
    let b = (1..=4).map(|h| other.anchor0().chain.get_block_by_height(h).unwrap().header.state_root).collect::<Vec<_>>();
    let mine = (1..=4).map(|h| net.anchor0().chain.get_block_by_height(h).unwrap().header.state_root).collect::<Vec<_>>();
    assert_eq!(b, mine, "identical before activation");
    // Import the open net's own blocks 1..4 into a static-mode node (block hashes include timestamps,
    // so the separately produced `other` chain may not be a valid parent for b5; that made this flaky).
    let mut stat = BoingNode::with_validators(vec![a], idk(&open_key(5)), Some(open_key(5)));
    stat.chain = ChainState::from_genesis(net.anchor0().chain.get_block_by_height(0).unwrap());
    stat.state = genesis_state(&funded_ids(0));
    for h in 1..=4 {
        stat.import_network_block_catchup(&net.anchor0().chain.get_block_by_height(h).unwrap()).unwrap();
    }
    let err = stat.import_network_block_catchup(&b5).unwrap_err();
    assert!(err.to_string().contains("state root"), "{err}");
    let _ = &net.params;
}
