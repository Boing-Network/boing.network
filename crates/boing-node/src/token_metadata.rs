//! JSON-RPC **`boing_getTokenMetadata`** and **`boing_listTokens`**: name, symbol, decimals and total
//! supply for **every** token the node can see, not only tokens that sit in a native DEX pool
//! (compare [`crate::native_dex_discovery`]).
//!
//! Read-only and **non-consensus**: nothing here touches execution, state transitions or block
//! validity. The node builds an in-memory index from blocks it already holds.
//!
//! ## What is (and is not) on chain
//!
//! * **Name and symbol** exist only in the `ContractDeployWithPurposeAndMetadata` deploy transaction
//!   (`asset_name` <= 256 bytes, `asset_symbol` <= 32 bytes). Contract storage never holds them. A
//!   token deployed without that metadata has **no** name or symbol anywhere; the RPC returns `null`
//!   (`metadataSource: null`) and never invents a label.
//! * **Decimals**: the reference fungible templates have no decimals slot (whole units, `0`), wrapped
//!   BOING is `18`, an operator map (`BOING_DEX_TOKEN_DECIMALS_JSON`) may override, anything else is
//!   `null` (unknown), the node does not guess. Same resolver as `boing_getDexToken`.
//! * **Total supply**: wrapped BOING keeps it in storage. Reference tokens have no supply slot; their
//!   supply is the amount of the single successful one-time `mint_first` call, found in the blocks
//!   and receipts the node still holds (`totalSupplySource: "mint_first"`). Anything else is `null`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use boing_execution::{
    ref_fungible_mint_once_key, reference_fungible_secured_runtime_bytecode,
    reference_fungible_secured_runtime_bytecode_v2, reference_fungible_template_bytecode,
    reference_fungible_template_v2_bytecode, wboing_total_supply_key,
};
use boing_primitives::{
    AccountId, BatchCall, ExecutionReceipt, Hash, Transaction, TransactionPayload,
};
use boing_state::StateStore;
use serde_json::{json, Map, Value};

use crate::chain::ChainState;
use crate::native_dex_discovery::{
    account_id_hex_lower, decimals_json, err_bad, parse_account_id_hex, parse_token_decimals_json,
    predicted_deploy_addresses, resolve_token_decimals, source_json, DexDiscoveryRpcError,
};

const DEFAULT_PAGE_LIMIT: usize = 100;
const MAX_PAGE_LIMIT: usize = 500;
const MAX_NAME_CHARS: usize = 256;
const MAX_SYMBOL_CHARS: usize = 32;
const SELECTOR_MINT_FIRST: u8 = 0x02;

/// Known token implementations (recognised by exact deployed code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenStandard {
    ReferenceFungibleV1,
    ReferenceFungibleV2,
    ReferenceFungibleSecuredV1,
    ReferenceFungibleSecuredV2,
    WrappedBoing,
}

impl TokenStandard {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReferenceFungibleV1 => "reference-fungible-v1",
            Self::ReferenceFungibleV2 => "reference-fungible-v2",
            Self::ReferenceFungibleSecuredV1 => "reference-fungible-secured-v1",
            Self::ReferenceFungibleSecuredV2 => "reference-fungible-secured-v2",
            Self::WrappedBoing => "wrapped-boing",
        }
    }

    fn mint_once_key(self) -> Option<[u8; 32]> {
        match self {
            Self::ReferenceFungibleV1 | Self::ReferenceFungibleV2 => {
                Some(ref_fungible_mint_once_key())
            }
            Self::ReferenceFungibleSecuredV1 | Self::ReferenceFungibleSecuredV2 => {
                Some(boing_execution::reference_fungible_secured::ref_sec_mint_once_key())
            }
            Self::WrappedBoing => None,
        }
    }
}

fn standard_codes() -> &'static [(TokenStandard, Vec<u8>)] {
    static CODES: OnceLock<Vec<(TokenStandard, Vec<u8>)>> = OnceLock::new();
    CODES.get_or_init(|| {
        vec![
            (
                TokenStandard::ReferenceFungibleV2,
                reference_fungible_template_v2_bytecode(),
            ),
            (
                TokenStandard::ReferenceFungibleV1,
                reference_fungible_template_bytecode(),
            ),
            (
                TokenStandard::ReferenceFungibleSecuredV2,
                reference_fungible_secured_runtime_bytecode_v2(),
            ),
            (
                TokenStandard::ReferenceFungibleSecuredV1,
                reference_fungible_secured_runtime_bytecode(),
            ),
            (
                TokenStandard::WrappedBoing,
                boing_execution::wrapped_boing_bytecode(),
            ),
        ]
    })
}

/// Which known token implementation `id` runs, if any.
pub fn classify_standard(state: &StateStore, id: &AccountId) -> Option<TokenStandard> {
    let code = state.get_contract_code(id)?;
    standard_codes()
        .iter()
        .find(|(_, c)| c == code)
        .map(|(s, _)| *s)
}

#[derive(Clone, Debug)]
struct DeployRecord {
    name: Option<String>,
    symbol: Option<String>,
    deploy_height: u64,
    deployer: AccountId,
    deploy_tx: Hash,
}

/// Incremental in-memory index over deploy transactions and `mint_first` calls.
#[derive(Default)]
pub struct TokenIndex {
    /// Next block height to scan.
    next_height: u64,
    /// `(height, hash)` of the last scanned block; a mismatch means the chain changed under us.
    anchor: Option<(u64, Hash)>,
    deploys: BTreeMap<AccountId, DeployRecord>,
    first_mint: HashMap<AccountId, u128>,
}

fn clip(s: Option<&str>, max_chars: usize) -> Option<String> {
    let t = s.unwrap_or("").trim();
    if t.is_empty() {
        None
    } else {
        Some(t.chars().take(max_chars).collect())
    }
}

/// `mint_first(to, amount)` calldata: selector word `0x02`, `to` word, amount word (u128 in the low 16 bytes).
fn parse_mint_first(calldata: &[u8]) -> Option<u128> {
    if calldata.len() != 96 {
        return None;
    }
    if calldata[..31].iter().any(|b| *b != 0) || calldata[31] != SELECTOR_MINT_FIRST {
        return None;
    }
    if calldata[64..80].iter().any(|b| *b != 0) {
        return None;
    }
    let mut a = [0u8; 16];
    a.copy_from_slice(&calldata[80..96]);
    Some(u128::from_be_bytes(a))
}

impl TokenIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of deploy records (tests / diagnostics).
    pub fn len(&self) -> usize {
        self.deploys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.deploys.is_empty()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn index_call(&mut self, contract: &AccountId, calldata: &[u8], success: bool) {
        if !success || self.first_mint.contains_key(contract) {
            return;
        }
        if let Some(amount) = parse_mint_first(calldata) {
            self.first_mint.insert(*contract, amount);
        }
    }

    fn index_tx(
        &mut self,
        height: u64,
        tx: &Transaction,
        state: &StateStore,
        receipts: &HashMap<Hash, ExecutionReceipt>,
    ) {
        let id = tx.id();
        let receipt = receipts.get(&id);
        // Without a receipt we cannot tell success from failure; skip rather than guess.
        let success = receipt.map(|r| r.success);
        match &tx.payload {
            TransactionPayload::ContractCall { contract, calldata }
            | TransactionPayload::ContractCallWithValue {
                contract, calldata, ..
            } => self.index_call(contract, calldata, success == Some(true)),
            TransactionPayload::Batch { calls } => {
                for c in calls {
                    if let BatchCall::ContractCall { contract, calldata }
                    | BatchCall::ContractCallWithValue {
                        contract, calldata, ..
                    } = c
                    {
                        self.index_call(contract, calldata, success == Some(true));
                    }
                }
            }
            _ => {
                let Some((_, _, _, name, symbol)) = tx.payload.as_contract_deploy() else {
                    return;
                };
                if success == Some(false) {
                    return;
                }
                let name = clip(name, MAX_NAME_CHARS);
                let symbol = clip(symbol, MAX_SYMBOL_CHARS);
                for addr in predicted_deploy_addresses(tx) {
                    // Only the address that actually received code counts (init-code deploys predict two).
                    if state.get_contract_code(&addr).is_none() || self.deploys.contains_key(&addr)
                    {
                        continue;
                    }
                    self.deploys.insert(
                        addr,
                        DeployRecord {
                            name: name.clone(),
                            symbol: symbol.clone(),
                            deploy_height: height,
                            deployer: tx.sender,
                            deploy_tx: id,
                        },
                    );
                }
            }
        }
    }

    /// Bring the index up to the chain tip. Scans only new blocks; rebuilds if the last scanned block
    /// is no longer on the canonical chain.
    pub fn refresh(
        &mut self,
        chain: &ChainState,
        state: &StateStore,
        receipts: &HashMap<Hash, ExecutionReceipt>,
    ) {
        let tip = chain.height();
        if let Some((h, hash)) = self.anchor {
            let still = h <= tip && chain.with_block_by_height(h, |b| b.hash()) == Some(hash);
            if !still {
                self.reset();
            }
        }
        let mut h = self.next_height;
        while h <= tip {
            chain.with_block_by_height(h, |block| {
                for tx in &block.transactions {
                    self.index_tx(h, tx, state, receipts);
                }
            });
            h += 1;
        }
        self.next_height = tip.saturating_add(1);
        self.anchor = chain
            .with_block_by_height(tip, |b| b.hash())
            .map(|hash| (tip, hash));
    }

    fn token_json(
        &self,
        state: &StateStore,
        decimals_map: &HashMap<String, u8>,
        id: &AccountId,
    ) -> Value {
        let rec = self.deploys.get(id);
        let standard = classify_standard(state, id);
        let (dec, dec_src) = resolve_token_decimals(state, decimals_map, id);
        let (supply, supply_src) = self.total_supply(state, id, standard);
        let name = rec.and_then(|r| r.name.clone());
        let symbol = rec.and_then(|r| r.symbol.clone());
        let has_meta = name.is_some() || symbol.is_some();
        json!({
            "id": account_id_hex_lower(id),
            "name": name,
            "symbol": symbol,
            "decimals": decimals_json(dec),
            "decimalsSource": source_json(dec_src),
            "totalSupply": supply.map(|v| Value::from(v.to_string())).unwrap_or(Value::Null),
            "totalSupplySource": source_json(supply_src),
            "standard": standard.map(|s| Value::from(s.as_str())).unwrap_or(Value::Null),
            "metadataSource": if has_meta { Value::from("deploy") } else { Value::Null },
            "deployHeight": rec.map(|r| Value::from(r.deploy_height)).unwrap_or(Value::Null),
            "deployer": rec.map(|r| Value::from(account_id_hex_lower(&r.deployer))).unwrap_or(Value::Null),
            "deployTx": rec.map(|r| Value::from(format!("0x{}", hex::encode(r.deploy_tx.0)))).unwrap_or(Value::Null),
        })
    }

    fn total_supply(
        &self,
        state: &StateStore,
        id: &AccountId,
        standard: Option<TokenStandard>,
    ) -> (Option<u128>, Option<&'static str>) {
        match standard {
            Some(TokenStandard::WrappedBoing) => {
                let w = state.get_contract_storage(id, &wboing_total_supply_key());
                let mut b = [0u8; 16];
                b.copy_from_slice(&w[16..32]);
                (Some(u128::from_be_bytes(b)), Some("storage"))
            }
            Some(s) => {
                let Some(key) = s.mint_once_key() else {
                    return (None, None);
                };
                let minted = state.get_contract_storage(id, &key) != [0u8; 32];
                match (minted, self.first_mint.get(id)) {
                    (true, Some(v)) => (Some(*v), Some("mint_first")),
                    _ => (None, None),
                }
            }
            None => (None, None),
        }
    }

    /// A contract belongs in `boing_listTokens` when it declared a name/symbol at deploy or runs a
    /// known token implementation.
    fn is_listed(&self, state: &StateStore, id: &AccountId, rec: &DeployRecord) -> bool {
        rec.name.is_some() || rec.symbol.is_some() || classify_standard(state, id).is_some()
    }
}

/// Process-wide index used by the RPC handlers.
pub fn global_index() -> &'static Mutex<TokenIndex> {
    static INDEX: OnceLock<Mutex<TokenIndex>> = OnceLock::new();
    INDEX.get_or_init(|| Mutex::new(TokenIndex::new()))
}

fn object_params(params: &Option<Value>) -> Result<Map<String, Value>, DexDiscoveryRpcError> {
    match params {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(m)) => Ok(m.clone()),
        Some(Value::Array(a)) => match a.first() {
            None => Ok(Map::new()),
            Some(Value::Object(m)) => Ok(m.clone()),
            Some(Value::String(s)) => {
                let mut m = Map::new();
                m.insert("id".into(), Value::String(s.clone()));
                Ok(m)
            }
            _ => Err(err_bad(
                "params: expected [\"0x…\"], [{…}] or an object".to_string(),
            )),
        },
        _ => Err(err_bad("params: expected array or object".to_string())),
    }
}

/// `boing_getTokenMetadata`: `params` = `["0x<64 hex>"]`, `[{"id": "0x…"}]` or `{"id": "0x…"}`.
/// Returns `null` when the account is neither a recorded deploy nor a contract with code.
pub fn get_token_metadata(
    index: &Mutex<TokenIndex>,
    state: &StateStore,
    receipts: &HashMap<Hash, ExecutionReceipt>,
    chain: &ChainState,
    params: &Option<Value>,
) -> Result<Value, DexDiscoveryRpcError> {
    let m = object_params(params)?;
    let id_hex = m
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err_bad("params.id: expected hex string".to_string()))?;
    let id = parse_account_id_hex(id_hex)?;
    let mut idx = index.lock().unwrap();
    idx.refresh(chain, state, receipts);
    if !idx.deploys.contains_key(&id) && state.get_contract_code(&id).is_none() {
        return Ok(Value::Null);
    }
    Ok(idx.token_json(state, &parse_token_decimals_json(), &id))
}

/// `boing_listTokens`: `{ limit?, cursor? }`, tokens ordered by id; `cursor` is the last id returned.
pub fn list_tokens(
    index: &Mutex<TokenIndex>,
    state: &StateStore,
    receipts: &HashMap<Hash, ExecutionReceipt>,
    chain: &ChainState,
    params: &Option<Value>,
) -> Result<Value, DexDiscoveryRpcError> {
    let m = object_params(params)?;
    let limit = match m.get("limit") {
        None | Some(Value::Null) => DEFAULT_PAGE_LIMIT,
        Some(v) => v
            .as_u64()
            .filter(|n| *n >= 1)
            .ok_or_else(|| err_bad("params.limit: expected integer >= 1".to_string()))?
            .min(MAX_PAGE_LIMIT as u64) as usize,
    };
    let cursor = match m.get("cursor") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::String(s)) => Some(parse_account_id_hex(s)?),
        _ => return Err(err_bad("params.cursor: expected hex string".to_string())),
    };
    let mut idx = index.lock().unwrap();
    idx.refresh(chain, state, receipts);
    let decimals_map = parse_token_decimals_json();

    let listed: Vec<&AccountId> = idx
        .deploys
        .iter()
        .filter(|(id, rec)| idx.is_listed(state, id, rec))
        .map(|(id, _)| id)
        .collect();
    let total = listed.len();
    let start = match cursor {
        Some(c) => listed.partition_point(|id| **id <= c),
        None => 0,
    };
    let page: Vec<&AccountId> = listed.iter().skip(start).take(limit).copied().collect();
    let tokens: Vec<Value> = page
        .iter()
        .map(|id| idx.token_json(state, &decimals_map, id))
        .collect();
    let next = if start + page.len() < total {
        page.last().map(|id| Value::from(account_id_hex_lower(id)))
    } else {
        None
    };
    Ok(json!({
        "tokens": tokens,
        "nextCursor": next.unwrap_or(Value::Null),
        "total": total,
        "indexedThroughHeight": idx.next_height.saturating_sub(1),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, Block, BlockHeader};

    const DEPLOYER: AccountId = AccountId([0x11; 32]);

    fn header(height: u64, parent: Hash) -> BlockHeader {
        BlockHeader {
            parent_hash: parent,
            height,
            timestamp: height,
            proposer: DEPLOYER,
            tx_root: Hash::ZERO,
            receipts_root: Hash::ZERO,
            state_root: Hash::ZERO,
            blobs_root: None,
        }
    }

    fn deploy_tx(
        nonce: u64,
        bytecode: Vec<u8>,
        name: Option<&str>,
        symbol: Option<&str>,
    ) -> Transaction {
        Transaction {
            nonce,
            sender: DEPLOYER,
            payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                purpose_category: "token".into(),
                description_hash: None,
                asset_name: name.map(String::from),
                asset_symbol: symbol.map(String::from),
                create2_salt: None,
            },
            access_list: AccessList::new(vec![], vec![]),
        }
    }

    fn mint_tx(nonce: u64, token: AccountId, amount: u128) -> Transaction {
        Transaction {
            nonce,
            sender: DEPLOYER,
            payload: TransactionPayload::ContractCall {
                contract: token,
                calldata: boing_execution::encode_mint_first_calldata(&DEPLOYER, amount),
            },
            access_list: AccessList::new(vec![], vec![]),
        }
    }

    fn receipt(tx: &Transaction, height: u64, success: bool) -> (Hash, ExecutionReceipt) {
        (
            tx.id(),
            ExecutionReceipt {
                tx_id: tx.id(),
                block_height: height,
                tx_index: 0,
                success,
                gas_used: 0,
                return_data: vec![],
                logs: vec![],
                error: None,
            },
        )
    }

    struct Fixture {
        chain: ChainState,
        state: StateStore,
        receipts: HashMap<Hash, ExecutionReceipt>,
        named: AccountId,
        unnamed: AccountId,
        plain: AccountId,
        wboing: AccountId,
        failed: AccountId,
    }

    fn fixture() -> Fixture {
        let chain = ChainState::new(DEPLOYER);
        let mut state = StateStore::new();
        let mut receipts = HashMap::new();

        let d_named = deploy_tx(
            1,
            reference_fungible_template_v2_bytecode(),
            Some("  Alpha Coin "),
            Some("ALPH"),
        );
        let d_unnamed = deploy_tx(2, reference_fungible_template_bytecode(), None, None);
        // Not a known token implementation and no metadata: must not be listed.
        let mut other = reference_fungible_template_v2_bytecode();
        other.push(0);
        let d_plain = deploy_tx(3, other.clone(), None, None);
        // Unknown code but with metadata: listed (no standard), decimals unknown.
        let mut other2 = reference_fungible_template_v2_bytecode();
        other2.push(1);
        let d_custom = deploy_tx(4, other2, Some("Custom"), Some("CST"));
        let d_wboing = deploy_tx(
            5,
            boing_execution::wrapped_boing_bytecode(),
            Some("Wrapped BOING"),
            Some("wBOING"),
        );
        let d_failed = deploy_tx(
            6,
            reference_fungible_template_v2_bytecode(),
            Some("Nope"),
            Some("NOPE"),
        );

        let addr = |t: &Transaction| predicted_deploy_addresses(t)[0];
        let named = addr(&d_named);
        let unnamed = addr(&d_unnamed);
        let plain = addr(&d_plain);
        let custom = addr(&d_custom);
        let wboing = addr(&d_wboing);
        let failed = addr(&d_failed);
        for (a, t) in [
            (named, &d_named),
            (unnamed, &d_unnamed),
            (plain, &d_plain),
            (custom, &d_custom),
            (wboing, &d_wboing),
        ] {
            state.set_contract_code(a, t.payload.as_contract_deploy().unwrap().0.to_vec());
        }
        // `failed` reverted: no code, receipt says so.
        receipts.extend([
            receipt(&d_named, 1, true),
            receipt(&d_unnamed, 1, true),
            receipt(&d_plain, 1, true),
            receipt(&d_custom, 1, true),
            receipt(&d_wboing, 1, true),
            receipt(&d_failed, 1, false),
        ]);
        let b1 = Block {
            header: header(1, chain.latest_hash()),
            transactions: vec![d_named, d_unnamed, d_plain, d_custom, d_wboing, d_failed],
            signatures: vec![],
            priority_tips: vec![],
        };
        chain.append(b1).unwrap();

        // One-time mint on `named`, plus a second (no-op) successful mint and a reverted one on `unnamed`.
        let m1 = mint_tx(7, named, 1_000);
        let m2 = mint_tx(8, named, 5);
        let m3 = mint_tx(9, unnamed, 77);
        receipts.extend([
            receipt(&m1, 2, true),
            receipt(&m2, 2, true),
            receipt(&m3, 2, false),
        ]);
        state.merge_contract_storage(named, ref_fungible_mint_once_key(), {
            let mut w = [0u8; 32];
            w[31] = 1;
            w
        });
        let b2 = Block {
            header: header(2, chain.latest_hash()),
            transactions: vec![m1, m2, m3],
            signatures: vec![],
            priority_tips: vec![],
        };
        chain.append(b2).unwrap();

        // wBOING total supply in storage.
        let mut w = [0u8; 32];
        w[16..].copy_from_slice(&42u128.to_be_bytes());
        state.merge_contract_storage(wboing, wboing_total_supply_key(), w);

        Fixture {
            chain,
            state,
            receipts,
            named,
            unnamed,
            plain,
            wboing,
            failed,
        }
    }

    fn get(f: &Fixture, idx: &Mutex<TokenIndex>, id: &AccountId) -> Value {
        get_token_metadata(
            idx,
            &f.state,
            &f.receipts,
            &f.chain,
            &Some(json!([account_id_hex_lower(id)])),
        )
        .unwrap()
    }

    #[test]
    fn named_reference_token_has_deploy_metadata_zero_decimals_and_mint_supply() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        let v = get(&f, &idx, &f.named);
        assert_eq!(v["name"], "Alpha Coin");
        assert_eq!(v["symbol"], "ALPH");
        assert_eq!(v["decimals"], 0);
        assert_eq!(v["decimalsSource"], "template");
        assert_eq!(v["standard"], "reference-fungible-v2");
        assert_eq!(v["metadataSource"], "deploy");
        // First successful mint_first wins; the second call is a no-op on chain.
        assert_eq!(v["totalSupply"], "1000");
        assert_eq!(v["totalSupplySource"], "mint_first");
        assert_eq!(v["deployHeight"], 1);
    }

    #[test]
    fn token_without_metadata_has_null_name_symbol_not_an_abbreviation() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        let v = get(&f, &idx, &f.unnamed);
        assert!(v["name"].is_null());
        assert!(v["symbol"].is_null());
        assert!(v["metadataSource"].is_null());
        assert_eq!(v["decimals"], 0);
        // Reverted mint and no mint flag in storage: supply unknown.
        assert!(v["totalSupply"].is_null());
        assert!(v["totalSupplySource"].is_null());
    }

    #[test]
    fn unknown_code_has_null_decimals_and_supply() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        let v = get(&f, &idx, &f.plain);
        assert!(v["decimals"].is_null());
        assert!(v["decimalsSource"].is_null());
        assert!(v["standard"].is_null());
        assert!(v["totalSupply"].is_null());
    }

    #[test]
    fn wrapped_boing_reports_18_decimals_and_storage_supply() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        let v = get(&f, &idx, &f.wboing);
        assert_eq!(v["decimals"], 18);
        assert_eq!(v["totalSupply"], "42");
        assert_eq!(v["totalSupplySource"], "storage");
        assert_eq!(v["standard"], "wrapped-boing");
        assert_eq!(v["symbol"], "wBOING");
    }

    #[test]
    fn address_that_is_not_a_contract_is_null_and_reverted_deploy_is_not_indexed() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        assert!(get(&f, &idx, &AccountId([0xee; 32])).is_null());
        assert!(get(&f, &idx, &f.failed).is_null());
    }

    #[test]
    fn bad_params_are_invalid_params_errors() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        for p in [
            None,
            Some(json!([])),
            Some(json!(["0x12"])),
            Some(json!([7])),
        ] {
            let e = get_token_metadata(&idx, &f.state, &f.receipts, &f.chain, &p).unwrap_err();
            assert_eq!(e.0, -32602, "{p:?}");
        }
        // Object and array-of-object forms both work.
        let hex = account_id_hex_lower(&f.named);
        for p in [json!({"id": hex}), json!([{"id": hex}])] {
            let v = get_token_metadata(&idx, &f.state, &f.receipts, &f.chain, &Some(p)).unwrap();
            assert_eq!(v["symbol"], "ALPH");
        }
    }

    #[test]
    fn list_includes_named_and_known_standard_tokens_only_and_paginates() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        let all = list_tokens(&idx, &f.state, &f.receipts, &f.chain, &Some(json!({}))).unwrap();
        // named, unnamed (known template), custom (metadata), wboing; not `plain`, not the reverted one.
        assert_eq!(all["total"], 4);
        let ids: Vec<String> = all["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids.len(), 4);
        assert!(!ids.contains(&account_id_hex_lower(&f.plain)));
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "ordered by id");
        assert!(all["nextCursor"].is_null());
        assert_eq!(all["indexedThroughHeight"], 2);

        // Walk with limit 3 then 1.
        let p1 = list_tokens(
            &idx,
            &f.state,
            &f.receipts,
            &f.chain,
            &Some(json!({"limit": 3})),
        )
        .unwrap();
        assert_eq!(p1["tokens"].as_array().unwrap().len(), 3);
        let cur = p1["nextCursor"].as_str().unwrap().to_string();
        assert_eq!(cur, ids[2]);
        let p2 = list_tokens(
            &idx,
            &f.state,
            &f.receipts,
            &f.chain,
            &Some(json!({"limit": 3, "cursor": cur})),
        )
        .unwrap();
        assert_eq!(p2["tokens"].as_array().unwrap().len(), 1);
        assert_eq!(p2["tokens"][0]["id"], ids[3]);
        assert!(p2["nextCursor"].is_null());
    }

    #[test]
    fn list_rejects_bad_limit_and_cursor() {
        let f = fixture();
        let idx = Mutex::new(TokenIndex::new());
        for p in [
            json!({"limit": 0}),
            json!({"limit": "x"}),
            json!({"cursor": "0x12"}),
            json!({"cursor": 5}),
        ] {
            let e = list_tokens(&idx, &f.state, &f.receipts, &f.chain, &Some(p)).unwrap_err();
            assert_eq!(e.0, -32602);
        }
    }

    #[test]
    fn refresh_is_incremental_and_rebuilds_when_the_chain_changes() {
        let f = fixture();
        let mut idx = TokenIndex::new();
        idx.refresh(&f.chain, &f.state, &f.receipts);
        let n = idx.len();
        assert!(n >= 4);
        idx.refresh(&f.chain, &f.state, &f.receipts);
        assert_eq!(idx.len(), n, "second refresh adds nothing");
        // A different chain with a different history invalidates the anchor.
        let other = ChainState::new(AccountId([0x77; 32]));
        idx.refresh(&other, &f.state, &f.receipts);
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn name_and_symbol_are_clipped_to_the_protocol_limits() {
        assert_eq!(
            clip(Some(&"n".repeat(300)), MAX_NAME_CHARS)
                .unwrap()
                .chars()
                .count(),
            256
        );
        assert_eq!(
            clip(Some(&"s".repeat(40)), MAX_SYMBOL_CHARS)
                .unwrap()
                .chars()
                .count(),
            32
        );
        assert_eq!(clip(Some("   "), 10), None);
        assert_eq!(clip(None, 10), None);
    }

    #[test]
    fn mint_first_parser_requires_exact_layout() {
        let cd = boing_execution::encode_mint_first_calldata(&DEPLOYER, 9);
        assert_eq!(parse_mint_first(&cd), Some(9));
        assert_eq!(parse_mint_first(&cd[..95]), None);
        let mut bad = cd.clone();
        bad[31] = 1;
        assert_eq!(parse_mint_first(&bad), None);
        let mut big = cd;
        big[70] = 1; // amount above u128
        assert_eq!(parse_mint_first(&big), None);
    }
}
