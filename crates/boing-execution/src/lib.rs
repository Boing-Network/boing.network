//! Boing Execution — VM and parallel transaction scheduler
//!
//! Declared dependencies (access lists) enable deterministic parallel execution.

mod bytecode;
mod executor;
mod gas;
mod interpreter;
mod native_amm;
mod parallel;
pub mod reference_nft;
pub mod reference_token;
mod scheduler;
mod vm;

pub use bytecode::{gas as bytecode_gas, Opcode};
pub use executor::{BlockExecutor, ExecutionError};
pub use gas::GasConfig;
pub use interpreter::{Interpreter, StorageAccess};
pub use parallel::ExecutionView;
pub use scheduler::TransactionScheduler;
pub use reference_nft::{
    encode_owner_of_calldata, encode_set_metadata_hash_calldata, encode_transfer_nft_calldata,
    SELECTOR_OWNER_OF, SELECTOR_SET_METADATA_HASH, SELECTOR_TRANSFER_NFT, token_id_word,
};
pub use reference_token::{
    encode_mint_first_calldata, encode_transfer_calldata, smoke_contract_bytecode, SELECTOR_MINT_FIRST,
    SELECTOR_TRANSFER,
};
pub use native_amm::{
    constant_product_amount_out, constant_product_pool_bytecode, encode_add_liquidity_calldata,
    encode_remove_liquidity_calldata, encode_swap_calldata, reserve_a_key, reserve_b_key,
    SELECTOR_ADD_LIQUIDITY, SELECTOR_REMOVE_LIQUIDITY, SELECTOR_SWAP,
};
pub use vm::{TransferState, Vm, VmError, VmExecutionResult};
pub use boing_primitives::{Transaction, AccessList};
