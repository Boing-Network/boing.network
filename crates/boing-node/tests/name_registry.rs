//! Name registry end to end on in-process nodes: activation gating, commit-reveal, rent burn, resolve /
//! reverse RPC, transfer / re-target / release, expiry + grace, persistence across restart, and
//! determinism (a follower importing every block reaches identical state roots).

use boing_node::chain::ChainState;
use boing_node::node::BoingNode;
use boing_node::persistence::Persistence;
use boing_primitives::{
    name_commitment, AccessList, Account, AccountId, AccountState, Block, SignedTransaction,
    Transaction, TransactionPayload,
};
use boing_state::NameRegistryParams;
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

fn key(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}
fn idk(k: &SigningKey) -> AccountId {
    AccountId(k.verifying_key().to_bytes())
}

const SINK: AccountId = AccountId([0xee; 32]);
const START: u128 = 1_000 * boing_primitives::BASE_UNITS_PER_BOING;

fn params(activation_height: u64) -> NameRegistryParams {
    NameRegistryParams {
        activation_height,
        blocks_per_year: 20,
        grace_blocks: 5,
        commit_min_age: 1,
        commit_max_age: 30,
    }
}

struct Net {
    leader: BoingNode,
    follower: BoingNode,
    keys: Vec<SigningKey>,
    nonces: std::collections::HashMap<AccountId, u64>,
}

impl Net {
    fn new(p: Option<NameRegistryParams>) -> Self {
        let v = key(1);
        let vid = idk(&v);
        let keys: Vec<SigningKey> = (10..14).map(key).collect();
        let genesis = ChainState::genesis(vid);
        let mk = |local: AccountId, sk: Option<SigningKey>| {
            let mut n = BoingNode::with_validators(vec![vid], local, sk);
            n.chain = ChainState::from_genesis(genesis.clone());
            n.state = boing_state::StateStore::new();
            for k in &keys {
                n.state.insert(Account {
                    id: idk(k),
                    state: AccountState {
                        balance: START,
                        nonce: 0,
                        stake: 0,
                        ..Default::default()
                    },
                });
            }
            if let Some(p) = &p {
                n.enable_name_registry(p.clone());
            }
            n
        };
        let leader = mk(vid, Some(v.clone()));
        let follower = mk(AccountId([0x77; 32]), None);
        Net {
            leader,
            follower,
            keys,
            nonces: Default::default(),
        }
    }

    fn id(&self, i: usize) -> AccountId {
        idk(&self.keys[i])
    }

    fn signed(&mut self, i: usize, payload: TransactionPayload) -> SignedTransaction {
        let sender = self.id(i);
        let nonce = *self.nonces.get(&sender).unwrap_or(&0);
        let access_list = match &payload {
            TransactionPayload::Transfer { to, .. } => {
                AccessList::new(vec![sender, *to], vec![sender, *to])
            }
            _ => AccessList::new(vec![sender], vec![sender]),
        };
        SignedTransaction::new(
            Transaction {
                nonce,
                sender,
                payload,
                access_list,
            },
            &self.keys[i],
        )
    }

    /// Submit, produce one block, import it on the follower, and check both state roots agree.
    fn send(&mut self, i: usize, payload: TransactionPayload) -> Result<Block, String> {
        let s = self.signed(i, payload);
        self.leader
            .submit_transaction(s.clone())
            .map_err(|e| e.to_string())?;
        *self.nonces.entry(s.tx.sender).or_insert(0) += 1;
        Ok(self.block())
    }

    fn filler(&mut self) -> Block {
        self.send(
            3,
            TransactionPayload::Transfer {
                to: SINK,
                amount: 1,
            },
        )
        .unwrap()
    }

    fn block(&mut self) -> Block {
        let h = self.leader.chain.height();
        self.leader
            .produce_block_if_ready()
            .expect("block produced");
        assert_eq!(self.leader.chain.height(), h + 1);
        let b = self.leader.chain.get_block_by_height(h + 1).unwrap();
        self.follower
            .import_network_block_catchup(&b)
            .expect("follower imports");
        assert_eq!(
            self.follower
                .chain
                .get_block_by_height(h + 1)
                .unwrap()
                .header
                .state_root,
            b.header.state_root
        );
        assert_eq!(
            self.leader.state.state_root(),
            self.follower.state.state_root(),
            "height {}",
            h + 1
        );
        b
    }

    fn height(&self) -> u64 {
        self.leader.chain.height()
    }

    fn rpc(&self, method: &str, params: Value) -> Value {
        let a = self.leader.names_rpc(method, &params).expect("rpc ok");
        let b = self.follower.names_rpc(method, &params).expect("rpc ok");
        assert_eq!(a, b, "{method} differs between nodes");
        a
    }

    fn balance(&self, i: usize) -> u128 {
        self.leader.state.get(&self.id(i)).unwrap().balance
    }

    /// `rent` in whole BOING; balances and fees are base units (no history: 18 decimals from genesis).
    fn fee_paid(&self, i: usize, before: u128, rent: u128) -> u128 {
        before - self.balance(i) - rent * boing_primitives::BASE_UNITS_PER_BOING
    }

    /// Commit, then register in the next block. Returns the rent actually burned (balance delta minus fees).
    fn claim(
        &mut self,
        i: usize,
        label: &str,
        target: AccountId,
        years: u8,
        salt: u8,
    ) -> Result<(), String> {
        let owner = self.id(i);
        let salt = [salt; 32];
        self.send(
            i,
            TransactionPayload::NameCommit {
                commitment: name_commitment(
                    &boing_primitives::normalize_name(label).unwrap(),
                    &owner,
                    &salt,
                ),
            },
        )?;
        self.send(
            i,
            TransactionPayload::NameRegister {
                name: label.into(),
                salt,
                target,
                years,
            },
        )?;
        Ok(())
    }
}

#[test]
fn rejected_before_activation_and_when_disabled() {
    let mut off = Net::new(None);
    let err = off
        .send(
            0,
            TransactionPayload::NameCommit {
                commitment: [1; 32],
            },
        )
        .unwrap_err();
    assert!(err.contains("not enabled"), "{err}");
    assert_eq!(off.rpc("boing_resolveName", json!(["alice"])), Value::Null);

    let mut net = Net::new(Some(params(5)));
    let root_before = net.leader.state.state_root();
    let err = net
        .send(
            0,
            TransactionPayload::NameCommit {
                commitment: [1; 32],
            },
        )
        .unwrap_err();
    assert!(err.contains("activates at height 5"), "{err}");
    // Unused registry does not change the state root (same as a node without the feature).
    let mut plain = Net::new(None);
    assert_eq!(root_before, plain.leader.state.state_root());
    while net.height() < 4 {
        net.filler();
        plain.filler();
        assert_eq!(
            net.leader.state.state_root(),
            plain.leader.state.state_root(),
            "pre-activation roots"
        );
    }
    // Next block is height 5 → accepted.
    net.send(
        0,
        TransactionPayload::NameCommit {
            commitment: [1; 32],
        },
    )
    .unwrap();
    let info = net.rpc("boing_getNameInfo", json!(["alice"]));
    assert_eq!(info["registry"]["enabled"], true);
    assert_eq!(info["registry"]["activation_height"], 5);
}

#[test]
fn register_resolve_reverse_and_rent_burn() {
    let mut net = Net::new(Some(params(1)));
    let (a, b) = (net.id(0), net.id(1));
    let before = net.balance(0);

    // Reveal without a matured commit is rejected at admission.
    let err = net
        .send(
            0,
            TransactionPayload::NameRegister {
                name: "alice".into(),
                salt: [3; 32],
                target: a,
                years: 2,
            },
        )
        .unwrap_err();
    assert!(err.contains("commitment"), "{err}");
    net.nonces.insert(a, 0);

    net.claim(0, "Alice.boing", a, 2, 3).unwrap();
    // 5 chars → 2 BOING/year → 4 burned; two txs → two network fees.
    let fees = net.fee_paid(0, before, 4);
    assert!(fees > 0 && fees <= boing_primitives::BASE_UNITS_PER_BOING, "fees {fees}");

    let r = net.rpc("boing_resolveName", json!(["alice.boing"]));
    assert_eq!(r["target"], format!("0x{}", hex::encode(a.0)));
    assert_eq!(r["owner"], format!("0x{}", hex::encode(a.0)));
    assert_eq!(r["status"], "active");
    assert_eq!(r["expires_at"], 2 + 40); // commit at 1, register at 2, 2 years × 20 blocks

    // No reverse until set; then verified both ways.
    assert_eq!(
        net.rpc(
            "boing_reverseName",
            json!([format!("0x{}", hex::encode(a.0))])
        ),
        Value::Null
    );
    net.send(
        0,
        TransactionPayload::NameSetPrimary {
            name: Some("alice".into()),
        },
    )
    .unwrap();
    assert_eq!(
        net.rpc("boing_reverseName", json!([hex::encode(a.0)]))["name"],
        "alice"
    );

    // Someone else cannot take it, or set it as their primary.
    let err = net.claim(1, "alice", b, 1, 4).unwrap_err();
    assert!(err.contains("not available"), "{err}");
    net.nonces.insert(b, 1);
    let err = net
        .send(
            1,
            TransactionPayload::NameSetPrimary {
                name: Some("alice".into()),
            },
        )
        .unwrap_err();
    assert!(err.contains("primary"), "{err}");
    net.nonces.insert(b, 1);

    let chk = net.rpc(
        "boing_checkNames",
        json!([["alice", "bobby", "boing", "0xabc", "ab"]]),
    );
    let res = chk["results"].as_array().unwrap();
    assert_eq!(res[0]["available"], false);
    assert_eq!(res[1]["available"], true);
    assert_eq!(res[1]["rent_per_year"], "2000000000000000000");
    assert_eq!(res[1]["rent_per_year_boing"], "2");
    assert_eq!(res[2]["reserved"], true);
    assert_eq!(res[2]["available"], false);
    assert_eq!(res[3]["valid"], false);
    assert_eq!(res[4]["valid"], false);

    let owned = net.rpc("boing_namesOwnedBy", json!([hex::encode(a.0)]));
    assert_eq!(owned.as_array().unwrap().len(), 1);

    // Rent tiers: 3/4/6 chars.
    for (label, rent) in [("abc", 10u128), ("abcd", 5), ("abcdef", 1)] {
        let before = net.balance(1);
        net.claim(1, label, b, 1, 9).unwrap();
        let fees = net.fee_paid(1, before, rent);
        assert!(fees <= boing_primitives::BASE_UNITS_PER_BOING, "{label}: rent {rent} fees {fees}");
    }
}

#[test]
fn transfer_target_release_expiry_and_grace() {
    let mut net = Net::new(Some(params(1)));
    let (a, b, c) = (net.id(0), net.id(1), net.id(2));
    net.claim(0, "carol", a, 1, 1).unwrap(); // registered at height 2 → expires 22
    net.send(
        0,
        TransactionPayload::NameSetPrimary {
            name: Some("carol".into()),
        },
    )
    .unwrap();

    // Transfer keeps the target and clears the old owner's reverse record.
    net.send(
        0,
        TransactionPayload::NameTransfer {
            name: "carol".into(),
            new_owner: b,
        },
    )
    .unwrap();
    let r = net.rpc("boing_resolveName", json!(["carol"]));
    assert_eq!(r["owner"], format!("0x{}", hex::encode(b.0)));
    assert_eq!(r["target"], format!("0x{}", hex::encode(a.0)));
    assert_eq!(
        net.rpc("boing_reverseName", json!([hex::encode(a.0)])),
        Value::Null
    );
    let err = net
        .send(
            0,
            TransactionPayload::NameSetTarget {
                name: "carol".into(),
                target: c,
            },
        )
        .unwrap_err();
    assert!(err.contains("owner"), "{err}");
    net.nonces
        .insert(a, net.leader.state.get(&a).unwrap().nonce);

    net.send(
        1,
        TransactionPayload::NameSetTarget {
            name: "carol".into(),
            target: c,
        },
    )
    .unwrap();
    net.send(
        2,
        TransactionPayload::NameSetPrimary {
            name: Some("carol".into()),
        },
    )
    .unwrap();
    assert_eq!(
        net.rpc("boing_reverseName", json!([hex::encode(c.0)]))["name"],
        "carol"
    );

    // Run into the grace period: stops resolving, reverse disappears, owner may still renew.
    while net.height() < 21 {
        net.filler();
    }
    assert_eq!(net.rpc("boing_resolveName", json!(["carol"])), Value::Null);
    assert_eq!(
        net.rpc("boing_reverseName", json!([hex::encode(c.0)])),
        Value::Null
    );
    assert_eq!(
        net.rpc("boing_getNameInfo", json!(["carol"]))["record"]["status"],
        "grace"
    );
    net.send(
        1,
        TransactionPayload::NameRenew {
            name: "carol".into(),
            years: 1,
        },
    )
    .unwrap();
    assert_eq!(
        net.rpc("boing_resolveName", json!(["carol"]))["status"],
        "active"
    );
    assert_eq!(
        net.rpc("boing_reverseName", json!([hex::encode(c.0)]))["name"],
        "carol"
    );

    // Release → immediately available to others.
    net.send(
        1,
        TransactionPayload::NameRelease {
            name: "carol".into(),
        },
    )
    .unwrap();
    assert_eq!(net.rpc("boing_resolveName", json!(["carol"])), Value::Null);
    assert_eq!(
        net.rpc("boing_reverseName", json!([hex::encode(c.0)])),
        Value::Null
    );
    net.claim(2, "carol", c, 1, 2).unwrap();

    // Expire fully (past grace) → someone else can register it.
    let exp = net.rpc("boing_resolveName", json!(["carol"]))["expires_at"]
        .as_u64()
        .unwrap();
    while net.height() + 1 < exp + 5 {
        net.filler();
    }
    assert_eq!(
        net.rpc("boing_checkNames", json!([["carol"]]))["results"][0]["available"],
        true
    );
    net.claim(0, "carol", a, 1, 5).unwrap();
    assert_eq!(
        net.rpc("boing_resolveName", json!(["carol"]))["owner"],
        format!("0x{}", hex::encode(a.0))
    );
}

#[test]
fn persists_across_restart_and_replays_identically() {
    let dir = std::env::temp_dir().join(format!("boing-names-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut net = Net::new(Some(params(1)));
    let a = net.id(0);
    net.claim(0, "dora", a, 1, 1).unwrap();
    net.send(
        0,
        TransactionPayload::NameSetPrimary {
            name: Some("dora".into()),
        },
    )
    .unwrap();
    let root = net.leader.state.state_root();

    let p = Persistence::new(&dir);
    p.save_state(&net.leader.state).unwrap();
    let mut loaded = p.load_state().unwrap().expect("state on disk");
    assert_eq!(loaded.state_root(), root);
    assert_eq!(loaded.names(), net.leader.state.names());

    // A fresh node replaying the same blocks from genesis arrives at the same state.
    let mut replay = Net::new(Some(params(1)));
    for h in 1..=net.height() {
        let b = net.leader.chain.get_block_by_height(h).unwrap();
        replay.follower.import_network_block_catchup(&b).unwrap();
    }
    assert_eq!(replay.follower.state.state_root(), root);
}

/// Security review #6: copying a pending `NameCommit` hash no longer blocks the victim (nodes without a
/// network history run the v4 rules from genesis); the copy is useless to the attacker.
#[test]
fn front_running_a_commitment_does_not_block_the_owner() {
    let mut net = Net::new(Some(params(1)));
    let (a, b) = (net.id(0), net.id(1));
    let salt = [5u8; 32];
    let c = name_commitment("alice", &a, &salt);
    net.send(1, TransactionPayload::NameCommit { commitment: c }).unwrap();
    net.send(0, TransactionPayload::NameCommit { commitment: c })
        .expect("victim's commit is accepted despite the copy");
    let err = net
        .send(
            1,
            TransactionPayload::NameRegister { name: "alice".into(), salt, target: b, years: 1 },
        )
        .unwrap_err();
    assert!(err.contains("commitment"), "{err}");
    net.nonces.insert(b, 1);
    net.send(
        0,
        TransactionPayload::NameRegister { name: "alice".into(), salt, target: a, years: 1 },
    )
    .unwrap();
    let r = net.rpc("boing_resolveName", json!(["alice"]));
    assert_eq!(r["owner"], format!("0x{}", hex::encode(a.0)));
}
