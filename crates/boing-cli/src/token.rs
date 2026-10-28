//! `boing token` -- Boing-native fungible token deploy / mint / transfer / balance.
//!
//! Uses only Boing primitives and the canonical **reference fungible template**
//! (`boing_execution::reference_fungible_template_bytecode`, `docs/BOING-REFERENCE-TOKEN.md`).
//! No EVM / Solana / third-party chain runtime is involved: bytecode is Boing VM, transactions are
//! bincode `SignedTransaction` (Ed25519) submitted via `boing_submitTransaction`, and balances are
//! read with `boing_getContractStorage` at `holder ^ REF_FUNGIBLE_BALANCE_XOR`.
//!
//! Signer: 32-byte Ed25519 secret seed (hex) from `--secret-hex` or env `BOING_SECRET_HEX`
//! (same variable the tutorial scripts use). `--dry-run` prints signed tx hex without submitting.

use boing_execution::{
    encode_approve_calldata, encode_mint_first_calldata, encode_transfer_calldata,
    ref_fungible_allowance_key, reference_fungible_template_bytecode,
    reference_fungible_template_v2_bytecode, REF_FUNGIBLE_BALANCE_XOR,
};
use boing_primitives::{
    nonce_derived_contract_address, AccessList, AccountId, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

/// QA purpose category for fungible deploys (`QUALITY-ASSURANCE-NETWORK.md`).
pub const TOKEN_PURPOSE: &str = "token";

/// Reference fungible template version: `v1` (original) or `v2` (adds approve / transfer_from /
/// allowance; fixes v1's self-transfer double credit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenVersion(pub u8);

impl std::str::FromStr for TokenVersion {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().trim_start_matches(['v', 'V']) {
            "1" => Ok(TokenVersion(1)),
            "2" => Ok(TokenVersion(2)),
            _ => Err(format!("invalid token version `{s}` (expected v1 or v2)")),
        }
    }
}

impl TokenVersion {
    pub fn bytecode(self) -> Vec<u8> {
        if self.0 == 1 {
            reference_fungible_template_bytecode()
        } else {
            reference_fungible_template_v2_bytecode()
        }
    }
}

pub fn parse_account(s: &str) -> anyhow::Result<AccountId> {
    let b = hex::decode(s.trim().trim_start_matches("0x"))
        .map_err(|e| anyhow::anyhow!("invalid account hex: {e}"))?;
    let arr: [u8; 32] = b
        .try_into()
        .map_err(|_| anyhow::anyhow!("account id must be 32 bytes (64 hex chars)"))?;
    Ok(AccountId(arr))
}

pub fn parse_signing_key(s: &str) -> anyhow::Result<SigningKey> {
    let b = hex::decode(s.trim().trim_start_matches("0x"))
        .map_err(|_| anyhow::anyhow!("secret must be hex"))?;
    let arr: [u8; 32] = b
        .try_into()
        .map_err(|_| anyhow::anyhow!("secret must be a 32-byte Ed25519 seed (64 hex chars)"))?;
    Ok(SigningKey::from_bytes(&arr))
}

pub fn resolve_signing_key(arg: Option<String>) -> anyhow::Result<SigningKey> {
    let s = arg
        .or_else(|| std::env::var("BOING_SECRET_HEX").ok())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("no signer: pass --secret-hex or set BOING_SECRET_HEX"))?;
    parse_signing_key(&s)
}

pub fn account_of(key: &SigningKey) -> AccountId {
    AccountId(key.verifying_key().to_bytes())
}

pub fn hex_account(a: &AccountId) -> String {
    format!("0x{}", hex::encode(a.0))
}

/// Storage key of `holder`'s balance in the reference fungible template.
pub fn balance_storage_key(holder: &AccountId) -> [u8; 32] {
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = holder.0[i] ^ REF_FUNGIBLE_BALANCE_XOR[i];
    }
    k
}

/// Decode a 32-byte big-endian storage word into `u128` (errors if the high 16 bytes are non-zero).
pub fn decode_balance_word(hex_word: &str) -> anyhow::Result<u128> {
    let b = hex::decode(hex_word.trim().trim_start_matches("0x"))
        .map_err(|e| anyhow::anyhow!("invalid storage word hex: {e}"))?;
    if b.len() != 32 {
        anyhow::bail!("storage word must be 32 bytes, got {}", b.len());
    }
    if b[..16].iter().any(|x| *x != 0) {
        anyhow::bail!("balance exceeds u128");
    }
    let mut lo = [0u8; 16];
    lo.copy_from_slice(&b[16..]);
    Ok(u128::from_be_bytes(lo))
}

fn sign(tx: Transaction, key: &SigningKey) -> SignedTransaction {
    SignedTransaction::new(tx, key)
}

fn with_access_list(mut tx: Transaction) -> Transaction {
    tx.access_list = tx.suggested_parallel_access_list();
    tx
}

/// Signed `ContractDeployWithPurposeAndMetadata` for the reference fungible template.
#[allow(dead_code)] // v1 deploy (tests / back-compat)
pub fn build_deploy_tx(
    key: &SigningKey,
    nonce: u64,
    name: Option<String>,
    symbol: Option<String>,
) -> SignedTransaction {
    build_deploy_tx_versioned(key, nonce, name, symbol, TokenVersion(1))
}

/// Deploy tx for the chosen template version.
pub fn build_deploy_tx_versioned(
    key: &SigningKey,
    nonce: u64,
    name: Option<String>,
    symbol: Option<String>,
    version: TokenVersion,
) -> SignedTransaction {
    let tx = with_access_list(Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
            bytecode: version.bytecode(),
            purpose_category: TOKEN_PURPOSE.to_string(),
            description_hash: None,
            asset_name: name,
            asset_symbol: symbol,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    });
    sign(tx, key)
}

pub fn build_call_tx(
    key: &SigningKey,
    nonce: u64,
    contract: AccountId,
    calldata: Vec<u8>,
) -> SignedTransaction {
    let tx = with_access_list(Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractCall { contract, calldata },
        access_list: AccessList::default(),
    });
    sign(tx, key)
}

pub fn build_mint_tx(
    key: &SigningKey,
    nonce: u64,
    token: AccountId,
    to: AccountId,
    amount: u128,
) -> SignedTransaction {
    build_call_tx(key, nonce, token, encode_mint_first_calldata(&to, amount))
}

pub fn build_transfer_tx(
    key: &SigningKey,
    nonce: u64,
    token: AccountId,
    to: AccountId,
    amount: u128,
) -> SignedTransaction {
    build_call_tx(key, nonce, token, encode_transfer_calldata(&to, amount))
}

/// Refuse `transfer` to the signer itself: reference fungible **v1**, the secured template v1 and LP share v1
/// credit the recipient without debiting when `to == caller` (balance doubling). On v2 it is a
/// no-op, so refusing is always safe.
pub fn check_not_self_transfer(sender: &AccountId, to: &AccountId) -> anyhow::Result<()> {
    if sender == to {
        anyhow::bail!(
            "refusing self-transfer (--to is the signer): v1 reference tokens double the balance on self-transfer (known bug); it is a no-op on v2"
        );
    }
    Ok(())
}

/// v2 `approve(spender, amount)` (overwrites the allowance).
pub fn build_approve_tx(
    key: &SigningKey,
    nonce: u64,
    token: AccountId,
    spender: AccountId,
    amount: u128,
) -> SignedTransaction {
    build_call_tx(key, nonce, token, encode_approve_calldata(&spender, amount))
}

pub fn encode_signed(stx: &SignedTransaction) -> anyhow::Result<String> {
    Ok(format!("0x{}", hex::encode(bincode::serialize(stx)?)))
}

pub fn validate_metadata(name: &Option<String>, symbol: &Option<String>) -> anyhow::Result<()> {
    if let Some(n) = name {
        if n.len() > TransactionPayload::MAX_ASSET_NAME_LEN {
            anyhow::bail!(
                "name exceeds {} bytes",
                TransactionPayload::MAX_ASSET_NAME_LEN
            );
        }
    }
    if let Some(s) = symbol {
        if s.len() > TransactionPayload::MAX_ASSET_SYMBOL_LEN {
            anyhow::bail!(
                "symbol exceeds {} bytes",
                TransactionPayload::MAX_ASSET_SYMBOL_LEN
            );
        }
    }
    Ok(())
}

// --- RPC ---

pub(crate) async fn rpc(rpc_url: &str, method: &str, params: Value) -> anyhow::Result<Value> {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let resp = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("cannot reach {rpc_url}: {e}. Run `boing dev` first."))?;
    let v: Value = resp.json().await?;
    if let Some(err) = v.get("error") {
        anyhow::bail!("{method} failed: {err}");
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

async fn fetch_nonce(rpc_url: &str, account: &AccountId) -> anyhow::Result<u64> {
    let r = rpc(rpc_url, "boing_getAccount", json!([hex_account(account)])).await?;
    Ok(r.get("nonce").and_then(Value::as_u64).unwrap_or(0))
}

pub(crate) async fn submit(
    rpc_url: &str,
    stx: &SignedTransaction,
    dry_run: bool,
) -> anyhow::Result<()> {
    let hex_tx = encode_signed(stx)?;
    let id = format!("0x{}", hex::encode(stx.tx.id().0));
    if dry_run {
        println!("  tx_id {id}");
        println!("  signed {hex_tx}");
        return Ok(());
    }
    match rpc(rpc_url, "boing_submitTransaction", json!([hex_tx])).await {
        Ok(v) => {
            // Current nodes return the real id; older ones returned "ok" (ignored).
            if let Some(h) = v.get("tx_hash").and_then(|h| h.as_str()) {
                if h.starts_with("0x") && !h.eq_ignore_ascii_case(&id) {
                    eprintln!("  warning: node reported tx_hash {h}, expected {id}");
                }
            }
            println!("  submitted tx_id {id}")
        }
        // Same signed bytes already in the mempool (e.g. gateway fan-out raced P2P gossip): it was accepted.
        Err(e) if is_already_in_mempool(&e.to_string()) => {
            println!("  submitted tx_id {id} (already pending in mempool)")
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

/// The node's duplicate-signed-payload rejection (`-32000`, `data.reason = "already_in_mempool"`).
pub(crate) fn is_already_in_mempool(err: &str) -> bool {
    err.contains("already_in_mempool") || err.contains("Duplicate transaction")
}

pub(crate) async fn nonce_for(
    rpc_url: &str,
    key: &SigningKey,
    nonce: Option<u64>,
) -> anyhow::Result<u64> {
    match nonce {
        Some(n) => Ok(n),
        None => fetch_nonce(rpc_url, &account_of(key)).await,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn deploy(
    rpc_url: &str,
    secret: Option<String>,
    name: Option<String>,
    symbol: Option<String>,
    initial_supply: Option<u128>,
    mint_to: Option<String>,
    nonce: Option<u64>,
    dry_run: bool,
    version: TokenVersion,
) -> anyhow::Result<()> {
    validate_metadata(&name, &symbol)?;
    let key = resolve_signing_key(secret)?;
    let me = account_of(&key);
    let n = nonce_for(rpc_url, &key, nonce).await?;
    let token = nonce_derived_contract_address(&me, n);
    println!(
        "Deploying Boing reference fungible token (template v{})",
        version.0
    );
    println!("  deployer {}", hex_account(&me));
    println!("  token    {}", hex_account(&token));
    submit(
        rpc_url,
        &build_deploy_tx_versioned(&key, n, name, symbol, version),
        dry_run,
    )
    .await?;
    if let Some(supply) = initial_supply.filter(|s| *s > 0) {
        let to = match mint_to {
            Some(s) => parse_account(&s)?,
            None => me,
        };
        println!("Minting fixed supply {supply} to {}", hex_account(&to));
        submit(
            rpc_url,
            &build_mint_tx(&key, n + 1, token, to, supply),
            dry_run,
        )
        .await?;
    }
    println!(
        "Note: the template allows exactly one mint_first (fixed supply), admin = first caller."
    );
    Ok(())
}

pub async fn mint(
    rpc_url: &str,
    secret: Option<String>,
    token: &str,
    to: Option<String>,
    amount: u128,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let key = resolve_signing_key(secret)?;
    let token = parse_account(token)?;
    let to = match to {
        Some(s) => parse_account(&s)?,
        None => account_of(&key),
    };
    let n = nonce_for(rpc_url, &key, nonce).await?;
    submit(rpc_url, &build_mint_tx(&key, n, token, to, amount), dry_run).await
}

pub async fn transfer(
    rpc_url: &str,
    secret: Option<String>,
    token: &str,
    to: &str,
    amount: u128,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    if amount == 0 {
        anyhow::bail!("amount must be > 0");
    }
    let key = resolve_signing_key(secret)?;
    let (token, to) = (parse_account(token)?, parse_account(to)?);
    check_not_self_transfer(&account_of(&key), &to)?;
    let n = nonce_for(rpc_url, &key, nonce).await?;
    submit(
        rpc_url,
        &build_transfer_tx(&key, n, token, to, amount),
        dry_run,
    )
    .await
}

pub async fn balance(rpc_url: &str, token: &str, holder: &str) -> anyhow::Result<()> {
    let (token, holder) = (parse_account(token)?, parse_account(holder)?);
    let key = format!("0x{}", hex::encode(balance_storage_key(&holder)));
    let r = rpc(
        rpc_url,
        "boing_getContractStorage",
        json!([hex_account(&token), key]),
    )
    .await?;
    let word = r
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("unexpected boing_getContractStorage result: {r}"))?;
    println!("{}", decode_balance_word(word)?);
    Ok(())
}

/// `boing token approve` (template v2 only).
pub async fn approve(
    rpc_url: &str,
    secret: Option<String>,
    token: &str,
    spender: &str,
    amount: u128,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let key = resolve_signing_key(secret)?;
    let (token, spender) = (parse_account(token)?, parse_account(spender)?);
    let n = nonce_for(rpc_url, &key, nonce).await?;
    submit(
        rpc_url,
        &build_approve_tx(&key, n, token, spender, amount),
        dry_run,
    )
    .await
}

/// `boing token allowance` (template v2): read `allowance[owner][spender]` from storage.
pub async fn allowance(
    rpc_url: &str,
    token: &str,
    owner: &str,
    spender: &str,
) -> anyhow::Result<()> {
    let token = parse_account(token)?;
    let (owner, spender) = (parse_account(owner)?, parse_account(spender)?);
    let key = format!(
        "0x{}",
        hex::encode(ref_fungible_allowance_key(&owner, &spender))
    );
    let r = rpc(
        rpc_url,
        "boing_getContractStorage",
        json!([hex_account(&token), key]),
    )
    .await?;
    let word = r
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("unexpected boing_getContractStorage result: {r}"))?;
    println!("{}", decode_balance_word(word)?);
    Ok(())
}

pub fn print_bytecode(version: TokenVersion) {
    println!("0x{}", hex::encode(version.bytecode()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_execution::Interpreter;
    use boing_primitives::Account;
    use boing_state::StateStore;

    /// Dry runs must never touch the network: point at a live listener and assert nothing connects.
    #[test]
    fn dry_run_submit_never_calls_rpc() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stx = build_deploy_tx(&key(9), 0, Some("Dry".into()), Some("DRY".into()));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(submit(&url, &stx, true))
            .expect("dry run is offline");
        assert!(
            listener.accept().is_err(),
            "dry run opened a connection to the RPC"
        );
        // Control: a real submit does connect (and fails, since nobody answers JSON-RPC).
        let _ = rt.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_millis(300),
                submit(&url, &stx, false),
            )
            .await
        });
        assert!(
            listener.accept().is_ok(),
            "non-dry submit should reach the RPC"
        );
    }

    #[test]
    fn duplicate_mempool_error_is_treated_as_accepted() {
        let e = r#"boing_submitTransaction failed: {"code":-32000,"data":{"reason":"already_in_mempool"},"message":"Duplicate transaction"}"#;
        assert!(is_already_in_mempool(e));
        assert!(!is_already_in_mempool(
            r#"boing_submitTransaction failed: {"code":-32000,"message":"insufficient balance"}"#
        ));
    }

    fn key(b: u8) -> SigningKey {
        SigningKey::from_bytes(&[b; 32])
    }

    fn run(state: &mut StateStore, stx: &SignedTransaction) {
        let TransactionPayload::ContractCall { contract, calldata } = &stx.tx.payload else {
            panic!("expected call");
        };
        let code = state.get_contract_code(contract).expect("code").clone();
        let mut it = Interpreter::new(code, 5_000_000);
        it.run(stx.tx.sender, *contract, calldata, state).unwrap();
    }

    fn bal(state: &StateStore, token: &AccountId, who: &AccountId) -> u128 {
        let w = state.get_contract_storage(token, &balance_storage_key(who));
        decode_balance_word(&hex::encode(w)).unwrap()
    }

    #[test]
    fn deploy_tx_is_signed_token_purpose_with_metadata() {
        let k = key(7);
        let stx = build_deploy_tx(&k, 3, Some("Boing Test".into()), Some("BTST".into()));
        stx.verify().expect("valid signature");
        assert_eq!(stx.tx.nonce, 3);
        match &stx.tx.payload {
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                purpose_category,
                asset_symbol,
                create2_salt,
                ..
            } => {
                assert_eq!(bytecode, &reference_fungible_template_bytecode());
                assert_eq!(purpose_category, "token");
                assert_eq!(asset_symbol.as_deref(), Some("BTST"));
                assert!(create2_salt.is_none());
            }
            p => panic!("unexpected payload {p:?}"),
        }
        assert!(stx.tx.access_list_covers_parallel_suggestion());
        let hex_tx = encode_signed(&stx).unwrap();
        let back = SignedTransaction::from_bincode(&hex::decode(&hex_tx[2..]).unwrap()).unwrap();
        assert_eq!(back, stx);
    }

    #[test]
    fn token_versions_and_v2_approve_allowance() {
        assert_eq!("v2".parse::<TokenVersion>().unwrap(), TokenVersion(2));
        assert_eq!("1".parse::<TokenVersion>().unwrap(), TokenVersion(1));
        assert!("v3".parse::<TokenVersion>().is_err());
        assert_eq!(
            TokenVersion(1).bytecode(),
            reference_fungible_template_bytecode()
        );
        assert_eq!(
            TokenVersion(2).bytecode(),
            reference_fungible_template_v2_bytecode()
        );
        let (k, sp) = (key(1), AccountId([0x0c; 32]));
        let me = account_of(&k);
        let mut st = StateStore::new();
        let stx = build_deploy_tx_versioned(&k, 0, None, Some("V2".into()), TokenVersion(2));
        let token = nonce_derived_contract_address(&me, 0);
        st.insert(Account {
            id: token,
            state: Default::default(),
        });
        let TransactionPayload::ContractDeployWithPurposeAndMetadata { bytecode, .. } =
            &stx.tx.payload
        else {
            panic!("deploy");
        };
        st.set_contract_code(token, bytecode.clone());
        run(&mut st, &build_mint_tx(&k, 1, token, me, 500));
        run(&mut st, &build_approve_tx(&k, 2, token, sp, 70));
        let allw = |s: &StateStore| {
            let w = s.get_contract_storage(&token, &ref_fungible_allowance_key(&me, &sp));
            decode_balance_word(&hex::encode(w)).unwrap()
        };
        assert_eq!(allw(&st), 70);
        run(&mut st, &build_approve_tx(&k, 3, token, sp, 5));
        assert_eq!(allw(&st), 5, "approve overwrites");
        // v2 fixes v1's self-transfer double credit.
        run(&mut st, &build_transfer_tx(&k, 4, token, me, 100));
        assert_eq!(bal(&st, &token, &me), 500);
    }

    #[test]
    fn self_transfer_is_refused() {
        let me = account_of(&key(7));
        assert!(check_not_self_transfer(&me, &me).is_err());
        assert!(check_not_self_transfer(&me, &AccountId([1; 32])).is_ok());
    }

    #[test]
    fn parse_helpers() {
        assert!(parse_account("0x12").is_err());
        let a = parse_account(&"ab".repeat(32)).unwrap();
        assert_eq!(a.0, [0xab; 32]);
        let k = parse_signing_key(&format!("0x{}", "07".repeat(32))).unwrap();
        assert_eq!(account_of(&k), account_of(&key(7)));
        assert!(parse_signing_key("zz").is_err());
        assert_eq!(
            decode_balance_word(&format!("0x{}{:032x}", "00".repeat(16), 42u128)).unwrap(),
            42
        );
        assert!(decode_balance_word(&"ff".repeat(32)).is_err());
        assert!(validate_metadata(&None, &Some("X".repeat(64))).is_err());
    }

    #[test]
    fn end_to_end_deploy_mint_transfer_balance_on_boing_vm() {
        let deployer = key(1);
        let me = account_of(&deployer);
        let bob = account_of(&key(2));
        let token = nonce_derived_contract_address(&me, 0);

        let mut state = StateStore::new();
        state.insert(Account {
            id: token,
            state: Default::default(),
        });
        let deploy = build_deploy_tx(&deployer, 0, None, Some("TKN".into()));
        if let TransactionPayload::ContractDeployWithPurposeAndMetadata { bytecode, .. } =
            &deploy.tx.payload
        {
            state.set_contract_code(token, bytecode.clone());
        }

        run(
            &mut state,
            &build_mint_tx(&deployer, 1, token, me, 1_000_000),
        );
        assert_eq!(bal(&state, &token, &me), 1_000_000);

        run(
            &mut state,
            &build_transfer_tx(&deployer, 2, token, bob, 250_000),
        );
        assert_eq!(bal(&state, &token, &me), 750_000);
        assert_eq!(bal(&state, &token, &bob), 250_000);

        // Fixed supply: a second mint is a no-op.
        run(&mut state, &build_mint_tx(&deployer, 3, token, me, 5));
        assert_eq!(bal(&state, &token, &me), 750_000);

        // Overdraft transfer is rejected (balances unchanged).
        run(
            &mut state,
            &build_transfer_tx(&key(2), 0, token, me, 250_001),
        );
        assert_eq!(bal(&state, &token, &bob), 250_000);
    }

    #[test]
    fn template_passes_protocol_qa_as_token() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
        let r = check_contract_deploy_full(
            &reference_fungible_template_bytecode(),
            Some(TOKEN_PURPOSE),
            None,
            &RuleRegistry::new(),
        );
        assert!(matches!(r, QaResult::Allow | QaResult::Unsure), "{r:?}");
    }
}
