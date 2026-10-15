//! `boing pool router-deploy | router-bytecode | route`: custody router v1
//! (`boing_execution::native_dex_custody_router`).

use crate::pool::{
    build_pool_call_tx, effective_fee_bps, min_out_with_slippage, parse_direction, parse_extras,
    quote_amount_out, read_state, PoolVersion, Signer, POOL_PURPOSE,
};
use crate::token::{
    account_of, build_approve_tx, hex_account, nonce_for, parse_account, resolve_signing_key,
    submit,
};
use boing_execution::{
    encode_custody_router_swap_calldata, native_dex_custody_router_bytecode, CustodyHop,
    CUSTODY_ROUTER_MAX_HOPS,
};
use boing_primitives::{AccessList, AccountId, SignedTransaction, Transaction, TransactionPayload};
use ed25519_dalek::SigningKey;

/// A resolved hop: pool, direction, and the token pair it moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteHop {
    pub pool: AccountId,
    pub direction: u8,
    pub token_in: AccountId,
    pub token_out: AccountId,
}

pub fn build_router_deploy_tx(key: &SigningKey, nonce: u64) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode: native_dex_custody_router_bytecode(),
            purpose_category: POOL_PURPOSE.to_string(),
            description_hash: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

/// Checks the path is connected (`token_out[i] == token_in[i+1]`) and 1..=4 hops long.
pub fn check_path(hops: &[RouteHop]) -> anyhow::Result<()> {
    if hops.is_empty() || hops.len() > CUSTODY_ROUTER_MAX_HOPS {
        anyhow::bail!("route needs 1..={CUSTODY_ROUTER_MAX_HOPS} hops");
    }
    for w in hops.windows(2) {
        if w[0].token_out != w[1].token_in {
            anyhow::bail!(
                "path is not connected: hop output {} != next hop input {}",
                hex_account(&w[0].token_out),
                hex_account(&w[1].token_in)
            );
        }
    }
    Ok(())
}

/// Router call; access list = sender, router, every pool and token on the path, plus extras.
#[allow(clippy::too_many_arguments)]
pub fn build_router_swap_tx(
    key: &SigningKey,
    nonce: u64,
    router: AccountId,
    hops: &[RouteHop],
    amount_in: u128,
    min_out: u128,
    recipient: &AccountId,
    deadline: u64,
    extra: &[AccountId],
) -> SignedTransaction {
    let ch: Vec<CustodyHop> = hops
        .iter()
        .map(|h| CustodyHop {
            pool: h.pool,
            direction: h.direction,
            token_in: h.token_in,
        })
        .collect();
    let cd = encode_custody_router_swap_calldata(&ch, amount_in, min_out, recipient, deadline);
    let mut ex: Vec<AccountId> = extra.to_vec();
    for h in hops {
        ex.extend([h.pool, h.token_in, h.token_out]);
    }
    ex.dedup();
    build_pool_call_tx(key, nonce, router, cd, &ex)
}

fn split_hop(s: &str) -> anyhow::Result<(AccountId, u8, Option<(AccountId, AccountId)>)> {
    let parts: Vec<&str> = s.split(':').collect();
    let (pool, dir) = match parts.as_slice() {
        [p, d] | [p, d, _, _] => (parse_account(p)?, parse_direction(d)? as u8),
        _ => anyhow::bail!("--hop must be POOL:DIRECTION or POOL:DIRECTION:TOKEN_A:TOKEN_B"),
    };
    let toks = if parts.len() == 4 {
        Some((parse_account(parts[2])?, parse_account(parts[3])?))
    } else {
        None
    };
    Ok((pool, dir, toks))
}

/// Resolves a hop from explicit tokens.
pub fn resolve_hop(pool: AccountId, direction: u8, ta: AccountId, tb: AccountId) -> RouteHop {
    let (token_in, token_out) = if direction == 0 { (ta, tb) } else { (tb, ta) };
    RouteHop {
        pool,
        direction,
        token_in,
        token_out,
    }
}

pub struct RouteArgs {
    pub router: String,
    pub hops: Vec<String>,
    pub amount_in: u128,
    pub min_out: Option<u128>,
    pub slippage_bps: u128,
    pub to: Option<String>,
    pub extra: Vec<String>,
    pub no_approve: bool,
    pub deadline: u64,
}

pub async fn deploy(rpc_url: &str, s: Signer) -> anyhow::Result<()> {
    let key = resolve_signing_key(s.secret.clone())?;
    let n = nonce_for(rpc_url, &key, s.nonce).await?;
    let addr = boing_primitives::nonce_derived_contract_address(&account_of(&key), n);
    println!("  router   {}", hex_account(&addr));
    submit(rpc_url, &build_router_deploy_tx(&key, n), s.dry_run).await
}

pub fn print_bytecode() {
    println!("0x{}", hex::encode(native_dex_custody_router_bytecode()));
}

pub async fn route(rpc_url: &str, s: Signer, a: RouteArgs) -> anyhow::Result<()> {
    if a.amount_in == 0 {
        anyhow::bail!("--amount-in must be > 0");
    }
    let key = resolve_signing_key(s.secret.clone())?;
    let me = account_of(&key);
    let router = parse_account(&a.router)?;
    let offline = s.dry_run && s.nonce.is_some();
    let mut hops = Vec::new();
    for h in &a.hops {
        let (pool, dir, toks) = split_hop(h)?;
        let (ta, tb) = match toks {
            Some(t) => t,
            None if offline => {
                anyhow::bail!("offline dry-run needs --hop POOL:DIR:TOKEN_A:TOKEN_B")
            }
            None => {
                let st = read_state(rpc_url, &pool).await?;
                if st.token_a.0 == [0u8; 32] {
                    anyhow::bail!("pool {} has no tokens configured", hex_account(&pool));
                }
                (st.token_a, st.token_b)
            }
        };
        hops.push(resolve_hop(pool, dir, ta, tb));
    }
    check_path(&hops)?;
    if hops.len() > 1 {
        println!("Note: every hop except the last must be a v9 pool (returns amount_out); otherwise the tx reverts.");
    }
    let min_out = match a.min_out {
        Some(m) => m,
        None => {
            if offline {
                anyhow::bail!("offline dry-run needs an explicit --min-out");
            }
            let mut amt = a.amount_in;
            for h in &hops {
                let st = read_state(rpc_url, &h.pool).await?;
                let (ri, ro) = if h.direction == 0 {
                    (st.reserve_a, st.reserve_b)
                } else {
                    (st.reserve_b, st.reserve_a)
                };
                let fee = effective_fee_bps(PoolVersion(9), st.stored_fee_bps);
                amt = quote_amount_out(PoolVersion(9), ri, ro, amt, fee)?;
                if amt == 0 {
                    anyhow::bail!("quote is 0 at pool {}", hex_account(&h.pool));
                }
            }
            let m = min_out_with_slippage(amt, a.slippage_bps)?;
            println!(
                "  quote {amt}, min_out {m} ({} bps slippage)",
                a.slippage_bps
            );
            m
        }
    };
    if min_out == 0 {
        anyhow::bail!("--min-out must be > 0");
    }
    let recipient = match &a.to {
        Some(t) => parse_account(t)?,
        None => me,
    };
    let mut n = nonce_for(rpc_url, &key, s.nonce).await?;
    if !a.no_approve {
        println!(
            "approve {} of {} to the router",
            a.amount_in,
            hex_account(&hops[0].token_in)
        );
        submit(
            rpc_url,
            &build_approve_tx(&key, n, hops[0].token_in, router, a.amount_in),
            s.dry_run,
        )
        .await?;
        n += 1;
    }
    let extra = parse_extras(&a.extra)?;
    let tx = build_router_swap_tx(
        &key,
        n,
        router,
        &hops,
        a.amount_in,
        min_out,
        &recipient,
        a.deadline,
        &extra,
    );
    submit(rpc_url, &tx, s.dry_run).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(b: u8) -> AccountId {
        AccountId([b; 32])
    }

    #[test]
    fn hop_parsing_and_path_checks() {
        let s = format!(
            "0x{}:b-to-a:0x{}:0x{}",
            "11".repeat(32),
            "0a".repeat(32),
            "0b".repeat(32)
        );
        let (p, d, t) = split_hop(&s).unwrap();
        assert_eq!((p, d, t), (id(0x11), 1, Some((id(0x0a), id(0x0b)))));
        let h = resolve_hop(p, d, id(0x0a), id(0x0b));
        assert_eq!((h.token_in, h.token_out), (id(0x0b), id(0x0a)));
        assert!(split_hop("0x11").is_err());
        let a = resolve_hop(id(1), 0, id(0x0a), id(0x0b));
        let b = resolve_hop(id(2), 0, id(0x0b), id(0x0c));
        check_path(&[a, b]).unwrap();
        assert!(check_path(&[b, a]).is_err());
        assert!(check_path(&[]).is_err());
        assert!(check_path(&[a, b, a, b, a]).is_err());
    }

    /// The CLI-built transactions execute end-to-end in the VM (approve + 2-hop route).
    #[test]
    fn cli_route_tx_executes_in_vm() {
        use boing_execution::{
            constant_product_pool_bytecode_v9, encode_approve_calldata, encode_set_tokens_calldata,
            encode_v8_add_liquidity_calldata, reference_fungible_template_v2_bytecode, Interpreter,
        };
        use boing_primitives::Account;
        use boing_state::StateStore;
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let me = account_of(&key);
        let (ta, tb, tc, p1, p2, r) = (id(0x0a), id(0x0b), id(0x0c), id(0x91), id(0x92), id(0xe0));
        let mut st = StateStore::new();
        st.insert(Account {
            id: me,
            state: Default::default(),
        });
        for (a, c) in [
            (ta, reference_fungible_template_v2_bytecode()),
            (tb, reference_fungible_template_v2_bytecode()),
            (tc, reference_fungible_template_v2_bytecode()),
            (p1, constant_product_pool_bytecode_v9()),
            (p2, constant_product_pool_bytecode_v9()),
            (r, native_dex_custody_router_bytecode()),
        ] {
            st.insert(Account {
                id: a,
                state: Default::default(),
            });
            st.set_contract_code(a, c);
        }
        let run = |st: &mut StateStore, to: AccountId, cd: &[u8]| {
            let code = st.get_contract_code(&to).unwrap().clone();
            Interpreter::new(code, 40_000_000)
                .run(me, to, cd, st)
                .is_ok()
        };
        for t in [ta, tb, tc] {
            assert!(run(
                &mut st,
                t,
                &boing_execution::encode_mint_first_calldata(&me, 1_000_000)
            ));
        }
        for (p, x, y) in [(p1, ta, tb), (p2, tb, tc)] {
            assert!(run(&mut st, p, &encode_set_tokens_calldata(&x, &y)));
            assert!(run(&mut st, x, &encode_approve_calldata(&p, 100_000)));
            assert!(run(&mut st, y, &encode_approve_calldata(&p, 100_000)));
            assert!(run(
                &mut st,
                p,
                &encode_v8_add_liquidity_calldata(100_000, 100_000, 1, 0)
            ));
        }
        let hops = [resolve_hop(p1, 0, ta, tb), resolve_hop(p2, 0, tb, tc)];
        let ap = build_approve_tx(&key, 0, ta, r, 1_000);
        let sw = build_router_swap_tx(&key, 1, r, &hops, 1_000, 1, &me, 0, &[]);
        for tx in [ap, sw] {
            let TransactionPayload::ContractCall { contract, calldata } = &tx.tx.payload else {
                panic!("call")
            };
            assert!(run(&mut st, *contract, calldata));
            for a in [r, p1, p2, ta, tb, tc] {
                if *contract == r {
                    assert!(
                        tx.tx.access_list.write.contains(&a),
                        "access list covers path"
                    );
                }
            }
        }
    }
}
