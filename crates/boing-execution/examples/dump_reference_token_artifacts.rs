//! Print `0x`-hex of **reference-token / reference-NFT** bytecode for ops / CI / debugging.
//!
//! - [`boing_execution::smoke_contract_bytecode`] — **not** a user fungible (no balances).
//! - [`boing_execution::reference_fungible_template_bytecode`] — balances + `transfer` / `mint_first`
//!   ([BOING-REFERENCE-TOKEN.md](../../docs/BOING-REFERENCE-TOKEN.md)); deploy with purpose **`token`**.
//! - [`boing_execution::reference_nft_collection_template_bytecode`] — minimal **collection**
//!   implementing [BOING-REFERENCE-NFT.md](../../docs/BOING-REFERENCE-NFT.md) selectors (`owner_of`,
//!   `transfer_nft`, `set_metadata_hash`, `mint_batch`) + lazy admin; deploy with purpose **`nft`** / **`NFT`**.
//! - [`boing_execution::reference_fungible_secured_pinned_default_deploy_bytecode_v2`] — **`0xFD`** init +
//!   secured **v2** runtime (self-transfer is a no-op; v1 retired — audit #61 item 24); see
//!   [BOING-REFERENCE-TOKEN.md](../../docs/BOING-REFERENCE-TOKEN.md) § Secured template and
//!   [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](../../docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md).
//!
//! ```text
//! cargo run -p boing-execution --example dump_reference_token_artifacts
//! ```
//!
//! Stdout: line **1** = smoke, line **2** = fungible template **v1** (historical), line **3** = NFT collection,
//! line **4** = secured fungible **v2** (`0xFD` init + runtime),
//! line **5** = secured fungible **v2 runtime only** (embedded as
//! `DEFAULT_REFERENCE_FUNGIBLE_SECURED_RUNTIME_BYTECODE_HEX` for `boing-sdk` init bytecode builder).

fn print_hex(label: &str, code: &[u8]) {
    eprintln!("// {label}: {} bytes", code.len());
    print!("0x");
    for b in code {
        print!("{b:02x}");
    }
    println!();
}

fn main() {
    print_hex(
        "smoke_contract_bytecode (tests / pool hooks only — not a balances token)",
        &boing_execution::smoke_contract_bytecode(),
    );
    print_hex(
        "reference_fungible_template_bytecode (QA: purpose token)",
        &boing_execution::reference_fungible_template_bytecode(),
    );
    print_hex(
        "reference_nft_collection_template_bytecode (QA: purpose nft/NFT)",
        &boing_execution::reference_nft_collection_template_bytecode(),
    );
    print_hex(
        "reference_fungible_secured_pinned_default_deploy_bytecode_v2 (QA: purpose token; v1 retired)",
        &boing_execution::reference_fungible_secured_pinned_default_deploy_bytecode_v2(),
    );
    print_hex(
        "reference_fungible_secured_runtime_bytecode_v2 (SDK secured deploy init builder)",
        &boing_execution::reference_fungible_secured_runtime_bytecode_v2(),
    );
}
