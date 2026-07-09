//! Boing Execution — VM and parallel transaction scheduler
//!
//! Declared dependencies (access lists) enable deterministic parallel execution.

mod bytecode;
mod executor;
mod gas;
mod interpreter;
mod native_amm;
mod native_dex_factory;
mod native_dex_ledger_router;
mod native_dex_multihop_swap_router;
mod native_lp_share_token;
mod native_amm_lp_vault;
mod parallel;
pub mod reference_nft;
pub mod reference_token;
pub mod reference_fungible_secured;
mod scheduler;
mod vm;

pub use bytecode::{gas as bytecode_gas, Opcode};
pub use executor::{BlockExecutor, ExecutionError};
pub use gas::GasConfig;
pub use interpreter::{Interpreter, StorageAccess, VmExecutionContext, MAX_CALL_DEPTH};
pub use parallel::ExecutionView;
pub use scheduler::TransactionScheduler;
pub use reference_nft::{
    encode_owner_of_calldata, encode_set_metadata_hash_calldata, encode_transfer_nft_calldata,
    ref_nft_collection_admin_key, reference_nft_collection_template_bytecode,
    REF_NFT_METADATA_STORAGE_XOR, REF_NFT_OWNER_STORAGE_XOR, SELECTOR_OWNER_OF,
    SELECTOR_SET_METADATA_HASH, SELECTOR_TRANSFER_NFT, token_id_word,
};
pub use reference_token::{
    encode_mint_first_calldata, encode_transfer_calldata, ref_fungible_admin_key,
    ref_fungible_mint_once_key, reference_fungible_template_bytecode, smoke_contract_bytecode,
    REF_FUNGIBLE_BALANCE_XOR, SELECTOR_MINT_FIRST, SELECTOR_TRANSFER,
};
pub use reference_fungible_secured::{
    encode_secured_set_deny_calldata, ref_sec_admin_key, ref_sec_flags_key,
    reference_fungible_secured_deploy_bytecode, reference_fungible_secured_pinned_default_deploy_bytecode,
    reference_fungible_secured_runtime_bytecode, ReferenceFungibleSecuredConfig,
    FLAG_ANTI_BOT, FLAG_COOLDOWN, FLAG_DENYLIST, FLAG_MAX_TX, FLAG_MAX_WALLET, FLAG_NO_MINT,
    FLAG_TRANSFER_UNLOCK, REF_SECURED_BALANCE_XOR, REF_SECURED_COOLDOWN_XOR, REF_SECURED_DENY_XOR,
    SELECTOR_RENOUNCE_ADMIN, SELECTOR_SET_DENY, SELECTOR_SET_PAUSE, SELECTOR_SET_TRANSFER_UNLOCK,
};
pub use native_amm::{
    constant_product_amount_out, constant_product_amount_out_after_fee,
    constant_product_amount_out_after_fee_with_bps, constant_product_amount_out_fee_on_input,
    constant_product_amount_out_fee_on_input_with_bps, constant_product_pool_bytecode,
    constant_product_pool_bytecode_v2, constant_product_pool_bytecode_v3, constant_product_pool_bytecode_v4,
    constant_product_pool_bytecode_v5, constant_product_pool_bytecode_v6,
    decode_add_liquidity_return_lp_minted,
    encode_add_liquidity_calldata, encode_remove_liquidity_calldata, encode_remove_liquidity_to_calldata,
    encode_set_fee_admin_calldata, encode_set_swap_fee_bps_calldata,
    encode_set_tokens_calldata, encode_swap_calldata, encode_swap_to_calldata, fee_admin_key,
    lp_balance_storage_key,
    reserve_a_key, reserve_b_key,
    swap_fee_bps_key, token_a_key, token_b_key, tokens_configured_key, total_lp_supply_key,
    LP_BALANCE_STORAGE_XOR, NATIVE_AMM_TOPIC_ADD_LIQUIDITY, NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY,
    NATIVE_AMM_TOPIC_SWAP, NATIVE_CP_POOL_CREATE2_SALT_V1, NATIVE_CP_POOL_CREATE2_SALT_V2,
    NATIVE_CP_POOL_CREATE2_SALT_V3, NATIVE_CP_POOL_CREATE2_SALT_V4, NATIVE_CP_POOL_CREATE2_SALT_V5,
    NATIVE_CP_POOL_CREATE2_SALT_V6, NATIVE_CP_SWAP_FEE_BPS,
    SELECTOR_ADD_LIQUIDITY, SELECTOR_REMOVE_LIQUIDITY, SELECTOR_REMOVE_LIQUIDITY_TO,
    SELECTOR_SET_FEE_ADMIN, SELECTOR_SET_SWAP_FEE_BPS, SELECTOR_SET_TOKENS,
    SELECTOR_SWAP, SELECTOR_SWAP_TO,
};
pub use native_dex_factory::{
    encode_get_pair_at_calldata, encode_pairs_count_calldata, encode_register_pair_calldata,
    native_dex_factory_bytecode, native_dex_factory_count_key, native_dex_factory_triplet_base_word,
    native_dex_factory_triplet_storage_key,
    NATIVE_DEX_FACTORY_CREATE2_SALT_V1, NATIVE_DEX_FACTORY_MAX_PAIRS, NATIVE_DEX_FACTORY_TOPIC_REGISTER,
    SELECTOR_GET_PAIR_AT, SELECTOR_PAIRS_COUNT, SELECTOR_REGISTER_PAIR,
};
pub use native_dex_ledger_router::{
    encode_ledger_router_forward_calldata, encode_ledger_router_forward_calldata_v2,
    encode_ledger_router_forward_calldata_v3,
    native_dex_ledger_router_bytecode, native_dex_ledger_router_bytecode_v2,
    native_dex_ledger_router_bytecode_v3,
    NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1, NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2,
    NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3,
    SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL, SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V2,
    SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V3,
};
pub use native_dex_multihop_swap_router::{
    encode_swap2_router_calldata_128, encode_swap2_router_calldata_160, encode_swap3_router_calldata_128,
    encode_swap3_router_calldata_160, encode_swap4_router_calldata_128, encode_swap4_router_calldata_160,
    encode_swap5_router_calldata_128, encode_swap5_router_calldata_160, encode_swap6_router_calldata_128,
    encode_swap6_router_calldata_160, native_dex_multihop_swap_router_bytecode, native_dex_swap2_router_bytecode,
    NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1, NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1,
    SELECTOR_SWAP2_ROUTER_128, SELECTOR_SWAP2_ROUTER_160, SELECTOR_SWAP3_ROUTER_128,
    SELECTOR_SWAP3_ROUTER_160, SELECTOR_SWAP4_ROUTER_128, SELECTOR_SWAP4_ROUTER_160,
    SELECTOR_SWAP5_ROUTER_128, SELECTOR_SWAP5_ROUTER_160, SELECTOR_SWAP6_ROUTER_128,
    SELECTOR_SWAP6_ROUTER_160,
};
pub use native_lp_share_token::{
    encode_lp_share_mint_calldata, encode_lp_share_set_minter_once_calldata,
    encode_lp_share_transfer_calldata, lp_share_minter_key, lp_share_token_bytecode, LP_SHARE_BALANCE_XOR,
    LP_SHARE_MINTER_KEY, NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1, SELECTOR_LP_SHARE_MINT,
    SELECTOR_LP_SHARE_SET_MINTER_ONCE, SELECTOR_LP_SHARE_TRANSFER,
};
pub use native_amm_lp_vault::{
    encode_native_amm_lp_vault_configure_calldata, encode_native_amm_lp_vault_deposit_add_calldata,
    native_amm_lp_vault_bytecode, NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1, NATIVE_AMM_LP_VAULT_KEY_CONFIGURED,
    NATIVE_AMM_LP_VAULT_KEY_POOL, NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN, SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE,
    SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD,
};
pub use vm::{
    GAS_PER_CONTRACT_DEPLOY_INIT, TransferState, Vm, VmError, VmExecutionResult,
};
pub use boing_primitives::{Transaction, AccessList};
