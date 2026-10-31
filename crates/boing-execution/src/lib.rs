//! Boing Execution — VM and parallel transaction scheduler
//!
//! Declared dependencies (access lists) enable deterministic parallel execution.

mod aliasing_audit;
mod asm;
mod bytecode;
mod executor;
mod gas;
mod hash_opcode_scaffold;
mod interpreter;
mod native_amm;
mod native_amm_v8;
mod native_dex_custody_router;
mod native_lp_vault_v2;
pub mod native_wrapped_boing;
pub use native_lp_vault_v2::*;
mod builder_attestation_registry;
pub mod history;
mod interface_blob_store;
mod linked_nft_token_registry;
mod linked_nft_token_registry_v2;
mod native_dex_factory;
pub use history::{ExecutionHistory, FeeEra};
mod interface_registry;
mod native_amm_lp_vault;
mod native_dex_ledger_router;
mod native_dex_multihop_swap_router;
mod native_lp_share_token;
mod parallel;
pub mod reference_fungible_secured;
pub mod reference_nft;
pub mod reference_token;
pub mod reference_token_v2;
pub mod reference_token_v3;
mod scheduler;
mod vm;

pub use boing_primitives::{AccessList, Transaction};
pub use builder_attestation_registry::{
    builder_attestation_claimer_storage_key, builder_attestation_registry_bytecode,
    builder_attestation_registry_count_key, builder_attestation_registry_slot_base_word,
    builder_attestation_registry_slot_storage_key, encode_builder_attest_attest_calldata,
    encode_builder_attest_claim_asset_calldata, encode_builder_attest_count_calldata,
    encode_builder_attest_get_at_calldata, encode_builder_attest_get_claimer_calldata,
    encode_builder_attest_revoke_at_calldata, encode_builder_attest_transfer_claimer_calldata,
    BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1, BUILDER_ATTESTATION_REGISTRY_MAX_SLOTS,
    BUILDER_ATTEST_CLAIMER_XOR, BUILDER_ATTEST_TOPIC_ATTEST, BUILDER_ATTEST_TOPIC_REVOKE,
    SELECTOR_BUILDER_ATTEST_ATTEST, SELECTOR_BUILDER_ATTEST_CLAIM_ASSET,
    SELECTOR_BUILDER_ATTEST_COUNT, SELECTOR_BUILDER_ATTEST_GET_AT,
    SELECTOR_BUILDER_ATTEST_GET_CLAIMER, SELECTOR_BUILDER_ATTEST_REVOKE_AT,
    SELECTOR_BUILDER_ATTEST_TRANSFER_CLAIMER,
};
pub use bytecode::{gas as bytecode_gas, Opcode};
pub use executor::{
    failed_tx_fee_amount, failed_tx_fee_amount_in, BlockExecutor, ExecutionError, FAILED_TX_GAS,
};
pub use gas::GasConfig;
pub use hash_opcode_scaffold::{
    blake3_opcode_active, parse_blake3_activation_height, ref_fungible_allowance_key_v3_host,
    ref_fungible_allowance_v3_domain_word, BLAKE3_MAX_INPUT_BYTES, BLAKE3_OPCODE,
    BLAKE3_OPCODE_ACTIVATION_ENV, PLANNED_BLAKE3_OPCODE, REF_FUNGIBLE_ALLOWANCE_V3_DOMAIN,
};
pub use interface_blob_store::{
    decode_interface_blob_words, encode_iface_blob_put_calldatas, interface_blob_id,
    interface_blob_storage_key, interface_blob_store_bytecode, interface_blob_uri,
    interface_blob_words, parse_interface_blob_uri, IFACE_BLOB_ENCODING_DEFLATE_RAW,
    IFACE_BLOB_ENCODING_RAW, IFACE_BLOB_MAGIC, IFACE_BLOB_MAX_STORED_BYTES,
    IFACE_BLOB_MAX_WORDS_PER_PUT, IFACE_BLOB_TAG, IFACE_BLOB_URI_PREFIX,
    INTERFACE_BLOB_STORE_CREATE2_SALT_V1, SELECTOR_IFACE_BLOB_PUT,
};
pub use interface_registry::{
    decode_iface_get_interface_return, encode_iface_claim_asset_calldata,
    encode_iface_get_claimer_calldata, encode_iface_get_interface_calldata,
    encode_iface_set_interface_calldata, encode_iface_transfer_claimer_calldata,
    interface_json_hash, interface_registry_bytecode, interface_registry_claimer_storage_key,
    interface_registry_field_storage_key, interface_registry_uri_words, InterfaceRecord,
    IFACE_CLAIMER_XOR, IFACE_FIELD_HASH, IFACE_FIELD_PUBLISHER, IFACE_FIELD_URI0,
    IFACE_FIELD_URI_LEN, IFACE_FIELD_XOR_BASE, IFACE_TOPIC_SET, INTERFACE_REGISTRY_CREATE2_SALT_V1,
    INTERFACE_REGISTRY_MAX_URI_BYTES, SELECTOR_IFACE_CLAIM_ASSET, SELECTOR_IFACE_GET_CLAIMER,
    SELECTOR_IFACE_GET_INTERFACE, SELECTOR_IFACE_SET_INTERFACE, SELECTOR_IFACE_TRANSFER_CLAIMER,
};
pub use interpreter::{Interpreter, StorageAccess, VmExecutionContext, MAX_CALL_DEPTH};
pub use linked_nft_token_registry::{
    encode_claim_asset_calldata, encode_get_asset_claimer_calldata, encode_get_link_at_calldata,
    encode_links_count_calldata, encode_register_link_calldata,
    encode_transfer_asset_claimer_calldata, encode_unlink_at_calldata,
    linked_nft_token_claimer_storage_key, linked_nft_token_registry_bytecode,
    linked_nft_token_registry_count_key, linked_nft_token_registry_pair_base_word,
    linked_nft_token_registry_pair_storage_key, LINKED_NFT_TOKEN_CLAIMER_XOR,
    LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1, LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS,
    LINKED_NFT_TOKEN_TOPIC_REGISTER, LINKED_NFT_TOKEN_TOPIC_UNLINK, SELECTOR_CLAIM_ASSET,
    SELECTOR_GET_ASSET_CLAIMER, SELECTOR_GET_LINK_AT, SELECTOR_LINKS_COUNT, SELECTOR_REGISTER_LINK,
    SELECTOR_TRANSFER_ASSET_CLAIMER, SELECTOR_UNLINK_AT,
};
pub use linked_nft_token_registry_v2::{
    encode_claim_self_calldata_v2, encode_get_asset_deployer_calldata_v2,
    linked_nft_token_deployer_storage_key_v2, linked_nft_token_registry_v2_claim_self_init_snippet,
    linked_nft_token_registry_v2_deploy_bytecode, linked_nft_token_registry_v2_runtime_bytecode,
    LinkedNftTokenRegistryV2Seed, LINKED_NFT_TOKEN_DEPLOYER_XOR_V2,
    LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2, LINKED_NFT_TOKEN_TOPIC_CLAIM_V2,
    SELECTOR_CLAIM_SELF_V2, SELECTOR_GET_ASSET_DEPLOYER_V2,
};
pub use native_amm::{
    constant_product_amount_out, constant_product_amount_out_after_fee,
    constant_product_amount_out_after_fee_with_bps, constant_product_amount_out_fee_on_input,
    constant_product_amount_out_fee_on_input_with_bps, constant_product_pool_bytecode,
    constant_product_pool_bytecode_v2, constant_product_pool_bytecode_v3,
    constant_product_pool_bytecode_v4, constant_product_pool_bytecode_v5,
    constant_product_pool_bytecode_v6, constant_product_pool_bytecode_v7,
    decode_add_liquidity_return_lp_minted, encode_add_liquidity_calldata,
    encode_remove_liquidity_calldata, encode_remove_liquidity_to_calldata,
    encode_set_fee_admin_calldata, encode_set_swap_fee_bps_calldata, encode_set_tokens_calldata,
    encode_swap_calldata, encode_swap_to_calldata, fee_admin_key, lp_balance_storage_key,
    reserve_a_key, reserve_b_key, swap_fee_bps_key, token_a_key, token_b_key,
    tokens_configured_key, total_lp_supply_key, LP_BALANCE_STORAGE_XOR,
    NATIVE_AMM_TOPIC_ADD_LIQUIDITY, NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY, NATIVE_AMM_TOPIC_SWAP,
    NATIVE_CP_POOL_CREATE2_SALT_V1, NATIVE_CP_POOL_CREATE2_SALT_V2, NATIVE_CP_POOL_CREATE2_SALT_V3,
    NATIVE_CP_POOL_CREATE2_SALT_V4, NATIVE_CP_POOL_CREATE2_SALT_V5, NATIVE_CP_POOL_CREATE2_SALT_V6,
    NATIVE_CP_POOL_CREATE2_SALT_V7, NATIVE_CP_SWAP_FEE_BPS, SELECTOR_ADD_LIQUIDITY,
    SELECTOR_REMOVE_LIQUIDITY, SELECTOR_REMOVE_LIQUIDITY_TO, SELECTOR_SET_FEE_ADMIN,
    SELECTOR_SET_SWAP_FEE_BPS, SELECTOR_SET_TOKENS, SELECTOR_SWAP, SELECTOR_SWAP_TO,
};
pub use native_amm_lp_vault::{
    encode_native_amm_lp_vault_configure_calldata, encode_native_amm_lp_vault_deposit_add_calldata,
    native_amm_lp_vault_bytecode, NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1,
    NATIVE_AMM_LP_VAULT_KEY_CONFIGURED, NATIVE_AMM_LP_VAULT_KEY_POOL,
    NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN, SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE,
    SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD,
};
pub use native_amm_v8::{
    constant_product_pool_bytecode_v10, constant_product_pool_bytecode_v11,
    constant_product_pool_bytecode_v8, constant_product_pool_bytecode_v9,
    encode_v8_add_liquidity_calldata, encode_v8_remove_liquidity_calldata,
    encode_v8_remove_liquidity_to_calldata, encode_v8_swap_calldata, encode_v8_swap_to_calldata,
    isqrt_u128, pool_v8_lock_key, NATIVE_CP_POOL_CREATE2_SALT_V10, NATIVE_CP_POOL_CREATE2_SALT_V11,
    NATIVE_CP_POOL_CREATE2_SALT_V8, NATIVE_CP_POOL_CREATE2_SALT_V9,
};
pub use native_dex_custody_router::{
    custody_router_lock_key, encode_custody_router_swap_calldata,
    native_dex_custody_router_bytecode, CustodyHop, CUSTODY_ROUTER_MAX_HOPS,
    NATIVE_DEX_CUSTODY_ROUTER_CREATE2_SALT_V1, SELECTOR_CUSTODY_ROUTER_SWAP,
};
pub use native_dex_factory::{
    encode_get_pair_at_calldata, encode_pairs_count_calldata, encode_register_pair_calldata,
    native_dex_factory_bytecode, native_dex_factory_count_key,
    native_dex_factory_triplet_base_word, native_dex_factory_triplet_storage_key,
    NATIVE_DEX_FACTORY_CREATE2_SALT_V1, NATIVE_DEX_FACTORY_MAX_PAIRS,
    NATIVE_DEX_FACTORY_TOPIC_REGISTER, SELECTOR_GET_PAIR_AT, SELECTOR_PAIRS_COUNT,
    SELECTOR_REGISTER_PAIR,
};
pub use native_dex_ledger_router::{
    encode_ledger_router_forward_calldata, encode_ledger_router_forward_calldata_v2,
    encode_ledger_router_forward_calldata_v3, native_dex_ledger_router_bytecode,
    native_dex_ledger_router_bytecode_v2, native_dex_ledger_router_bytecode_v3,
    NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1, NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2,
    NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3, SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL,
    SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V2, SELECTOR_LEDGER_ROUTER_FORWARD_POOL_CALL_V3,
};
pub use native_dex_multihop_swap_router::{
    encode_swap2_router_calldata_128, encode_swap2_router_calldata_160,
    encode_swap3_router_calldata_128, encode_swap3_router_calldata_160,
    encode_swap4_router_calldata_128, encode_swap4_router_calldata_160,
    encode_swap5_router_calldata_128, encode_swap5_router_calldata_160,
    encode_swap6_router_calldata_128, encode_swap6_router_calldata_160,
    native_dex_multihop_swap_router_bytecode, native_dex_swap2_router_bytecode,
    NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1, NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1,
    SELECTOR_SWAP2_ROUTER_128, SELECTOR_SWAP2_ROUTER_160, SELECTOR_SWAP3_ROUTER_128,
    SELECTOR_SWAP3_ROUTER_160, SELECTOR_SWAP4_ROUTER_128, SELECTOR_SWAP4_ROUTER_160,
    SELECTOR_SWAP5_ROUTER_128, SELECTOR_SWAP5_ROUTER_160, SELECTOR_SWAP6_ROUTER_128,
    SELECTOR_SWAP6_ROUTER_160,
};
pub use native_lp_share_token::{
    encode_lp_share_mint_calldata, encode_lp_share_set_minter_once_calldata,
    encode_lp_share_transfer_calldata, lp_share_minter_key, lp_share_token_bytecode,
    lp_share_token_bytecode_v2, LP_SHARE_BALANCE_XOR, LP_SHARE_MINTER_KEY,
    NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1, NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V2,
    SELECTOR_LP_SHARE_MINT, SELECTOR_LP_SHARE_SET_MINTER_ONCE, SELECTOR_LP_SHARE_TRANSFER,
};
pub use native_wrapped_boing::{
    encode_wboing_balance_of_calldata, encode_wboing_deposit_calldata,
    encode_wboing_total_supply_calldata, encode_wboing_withdraw_all_calldata,
    encode_wboing_withdraw_calldata, encode_wboing_withdraw_to_calldata, wboing_total_supply_key,
    wrapped_boing_bytecode, WBOING_DECIMALS, WBOING_SELECTOR_BALANCE_OF, WBOING_SELECTOR_DECIMALS,
    WBOING_SELECTOR_DEPOSIT, WBOING_SELECTOR_TOTAL_SUPPLY, WBOING_SELECTOR_WITHDRAW,
    WBOING_SELECTOR_WITHDRAW_ALL, WBOING_SELECTOR_WITHDRAW_TO, WBOING_TOPIC_DEPOSIT,
    WBOING_TOPIC_WITHDRAW,
};
pub use parallel::ExecutionView;
pub use reference_fungible_secured::{
    encode_secured_set_deny_calldata, ref_sec_admin_key, ref_sec_flags_key,
    reference_fungible_secured_deploy_bytecode, reference_fungible_secured_deploy_bytecode_v2,
    reference_fungible_secured_pinned_default_deploy_bytecode,
    reference_fungible_secured_pinned_default_deploy_bytecode_v2,
    reference_fungible_secured_runtime_bytecode, reference_fungible_secured_runtime_bytecode_v2,
    ReferenceFungibleSecuredConfig, FLAG_ANTI_BOT, FLAG_COOLDOWN, FLAG_DENYLIST, FLAG_MAX_TX,
    FLAG_MAX_WALLET, FLAG_NO_MINT, FLAG_TRANSFER_UNLOCK, REF_SECURED_BALANCE_XOR,
    REF_SECURED_COOLDOWN_XOR, REF_SECURED_DENY_XOR, SELECTOR_RENOUNCE_ADMIN, SELECTOR_SET_DENY,
    SELECTOR_SET_PAUSE, SELECTOR_SET_TRANSFER_UNLOCK,
};
pub use reference_nft::{
    encode_mint_batch_calldata, encode_owner_of_calldata, encode_set_metadata_hash_calldata,
    encode_transfer_nft_calldata, ref_nft_collection_admin_key,
    reference_nft_collection_template_bytecode, token_id_word, MAX_REFERENCE_NFT_MINT_BATCH,
    REF_NFT_METADATA_STORAGE_XOR, REF_NFT_OWNER_STORAGE_XOR, SELECTOR_MINT_BATCH,
    SELECTOR_OWNER_OF, SELECTOR_SET_METADATA_HASH, SELECTOR_TRANSFER_NFT,
};
pub use reference_token::{
    encode_mint_first_calldata, encode_transfer_calldata, ref_fungible_admin_key,
    ref_fungible_mint_once_key, reference_fungible_template_bytecode, smoke_contract_bytecode,
    REF_FUNGIBLE_BALANCE_XOR, SELECTOR_MINT_FIRST, SELECTOR_TRANSFER,
};
pub use reference_token_v2::{
    encode_allowance_calldata, encode_approve_calldata, encode_transfer_from_calldata,
    ref_fungible_allowance_key, reference_fungible_template_v2_bytecode,
    REF_FUNGIBLE_ALLOWANCE_XOR, SELECTOR_ALLOWANCE, SELECTOR_APPROVE, SELECTOR_TRANSFER_FROM,
};
pub use reference_token_v3::{
    ref_fungible_allowance_key_v3, reference_fungible_template_v3_bytecode,
};
pub use scheduler::TransactionScheduler;
pub use vm::{
    apply_media_blob_block_housekeeping, settle_expired_serve_challenges, TransferState, Vm,
    VmError, VmExecutionResult, GAS_BATCH_BASE, GAS_BATCH_CONTRACT_BUDGET,
    GAS_PER_BATCH_CONTRACT_CALL_STEP, GAS_PER_BATCH_TRANSFER, GAS_PER_CONTRACT_CALL,
    GAS_PER_CONTRACT_DEPLOY_INIT, GAS_PER_TRANSFER,
};
