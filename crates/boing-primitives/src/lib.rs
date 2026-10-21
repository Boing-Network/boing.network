//! Boing Blockchain — Core Primitives
//!
//! Types, hashing, signatures, and shared data structures.

pub mod blobs;
pub mod blobs_erasure;
pub mod blobs_phase6;
pub mod blobs_phase7;
pub mod blobs_phase8;
pub mod blobs_phase9;
pub mod blobs_phase10;
pub mod consensus_vote;
pub mod denomination;
pub mod equivocation;
pub mod hash;
pub mod hd;
pub mod intent;
pub mod names;
pub mod paymaster;
pub mod randomness;
pub mod recovery;
pub mod signature;
pub mod types;
pub mod validator_registration;
pub mod vrf_proof;

pub use denomination::{
    format_boing, parse_boing, Denomination, ParseBoingError, BASE_UNITS_PER_BOING, BOING_DECIMALS,
};
pub use blobs::{
    blob_id_hex, blob_id_of, blobs_root_of_transactions, chunk_media_blob,
    media_blob_chunk_merkle_proof, media_blob_chunks_merkle_root, media_blob_das_chunk_count,
    media_blob_das_sample, media_blob_gas, media_blob_gas_raw, media_blob_meta_tx_gas,
    media_blob_pin_rent_kib_blocks, media_blob_uri, parse_media_blob_id, recover_media_blob_xor,
    validate_blob_announce, validate_blob_announce_with_cap, validate_pay_blob_sample,
    validate_pay_blob_sample_with_max_chunks, validate_put_blob, validate_put_blob_with_cap,
    validate_renew_media_blob_pin, verify_media_blob_das_sample, within_phase3_target_cap,
    BlobError, BlobId, BlobRetention, MediaBlobChunkRef, MediaBlobDasSample, MediaBlobErasureBundle,
    MediaBlobGossip, MAX_MEDIA_BLOB_BYTES, MAX_MEDIA_BLOB_CONTENT_TYPE_LEN,
    MAX_MEDIA_BLOB_INLINE_BYTES, MAX_MEDIA_BLOB_INLINE_PREFER_BYTES,
    MAX_MEDIA_BLOB_RENEW_EXTEND_BLOCKS, MAX_MEDIA_BLOB_TTL_BLOCKS, MEDIA_BLOBS_GOSSIP_TOPIC,
    MEDIA_BLOB_DAS_CHUNK_BYTES, MEDIA_BLOB_FOREVER_MULTIPLIER, MEDIA_BLOB_GAS_BASE,
    MEDIA_BLOB_GAS_PER_WORD, MEDIA_BLOB_URI_PREFIX, MEDIA_BLOB_VIDEO_PIN_HORIZON_BLOCKS,
    PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES,
};
pub use blobs_erasure::{
    chunk_media_blob_rs, chunk_media_blob_rs_with_parity, media_blob_rs_chunk_refs,
    media_blob_rs_data_shards_for_size, recover_media_blob_rs, MediaBlobRsBundle,
    MEDIA_BLOB_RS_MAX_DATA_SHARDS, MEDIA_BLOB_RS_MIN_DATA_SHARDS, MEDIA_BLOB_RS_PARITY_SHARDS,
};
pub use blobs_phase6::{
    build_das_light_client_batch, build_das_light_client_batch_from_bytes,
    das_light_client_sample_indices, das_light_client_seed, manifest_from_segment_bytes,
    match_sample_offer, split_into_live_cap_segments, MediaBlobDasLightClientBatch,
    MediaBlobSampleMarket, MediaBlobSampleMatch, MediaBlobSampleOffer, MediaBlobSegmentManifest,
    PHASE6_DAS_LIGHT_CLIENT_DOMAIN, PHASE6_MAX_DAS_LIGHT_CLIENT_SAMPLES, PHASE6_MAX_SAMPLE_OFFERS,
    PHASE6_MAX_VIDEO_SEGMENTS, PHASE6_SEGMENT_MANIFEST_DOMAIN, PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES,
    PHASE6_VIDEO_SEGMENT_TARGET_BYTES,
};
pub use blobs_phase7::{
    das_light_client_sample_indices_vrf, das_light_client_seed_from_vrf,
    das_succinct_commitment_stub, media_blob_max_bytes_at, media_blob_max_das_chunks_at,
    phase7_cap_active, validate_register_blob_sample_offer, validate_settle_blob_sample_match,
    OnChainBlobSampleOffer, PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE7_CAP_ACTIVATION_ENV,
    PHASE7_DAS_SUCCINCT_STUB_DOMAIN, PHASE7_DAS_VRF_SEED_DOMAIN,
};
pub use blobs_phase8::{
    build_das_light_client_batch_vrf, build_das_succinct_proof_stub,
    build_das_succinct_proof_stub_vrf, match_sample_prefer_on_chain, phase8_cap_activation_checklist,
    verify_das_succinct_proof, BlobSampleOfferSource, BlobSampleSettleQuote,
    ConsensusNativeSampleMarket, DasSuccinctProof, DasSuccinctProofKind,
    PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE8_CAP_ACTIVATION_ENV,
    PHASE8_DAS_SUCCINCT_PROOF_DOMAIN, PHASE8_MAX_SAMPLE_OFFERS,
};
pub use blobs_phase9::{
    blob_sample_ask_escrow_account, das_light_client_seed_from_leader_vrf,
    phase9_cap_activation_checklist, sample_offer_market_key, PHASE9_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
    PHASE9_ASK_ESCROW_DOMAIN, PHASE9_CAP_ACTIVATION_ENV, PHASE9_SAMPLE_OFFER_REGISTRY_DOMAIN,
};
pub use blobs_phase10::{
    build_das_succinct_proof_reserved_placeholder, open_serve_challenge,
    phase10_cap_activation_checklist, phase10_das_succinct_proof_plan, serve_challenge_key,
    validate_challenge_blob_sample_serve, validate_respond_blob_sample_serve,
    verify_das_succinct_proof_reserved, BlobSampleServeChallenge, BlobSampleServeChallengeStatus,
    DasSuccinctProofReserved, PHASE10_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE10_CAP_ACTIVATION_ENV,
    PHASE10_DAS_RESERVED_OPAQUE_DOMAIN, PHASE10_MAX_OPAQUE_PROOF_BYTES,
    PHASE10_MAX_SERVE_CHALLENGES, PHASE10_SERVE_CHALLENGE_REGISTRY_DOMAIN,
    PHASE10_SERVE_CHALLENGE_WINDOW_BLOCKS,
};
pub use consensus_vote::{consensus_vote_signable_hash, ConsensusVote, CONSENSUS_VOTE_DOMAIN};
pub use equivocation::{EquivocationError, EquivocationEvidence};
pub use hash::{hasher, Hash};
pub use hd::HdPath;
pub use intent::{Intent, IntentKind, SignedIntent};
pub use names::{
    check_registrable, display_name, is_reserved, name_commitment, name_rent, name_rent_per_year,
    normalize_name, validate_label, NameError, NAME_MAX_LEN, NAME_MAX_YEARS, NAME_MIN_LEN,
    NAME_MIN_YEARS, NAME_SUFFIX, RESERVED_NAMES,
};
pub use paymaster::{PaymasterConfig, SponsoredTransaction};
pub use randomness::{
    dummy_vrf_output, ecvrf_prove, leader_from_ecvrf_proofs, leader_from_vrf, verify_ecvrf_output,
    verify_vrf_output, vrf_round_alpha, vrf_round_seed, VdfOutput, VrfOutput, ECVRF_PROOF_LEN,
};
pub use recovery::{Guardian, RecoveryRequest};
pub use signature::{
    default_signing_chain_id, set_default_signing_chain_id, set_tx_verify_policy, sign_transaction,
    sign_transaction_v2, sign_transaction_with_tip, signable_transaction_hash,
    signable_transaction_hash_v2, signable_transaction_hash_with_tip, tx_verify_policy,
    verify_signature, verify_signature_with, verify_signature_with_tip, Signature, SignatureError,
    SignedTransaction, TxSigPolicy, TX_SIGNING_DOMAIN_PRIORITY, TX_SIGNING_DOMAIN_V2,
};
pub use types::{
    contract_deploy_init_body, contract_deploy_uses_init_code, create2_contract_address,
    nonce_derived_contract_address, receipt_leaf_hash, receipts_root, tx_root,
    validate_batch_calls, AccessList, AccountId, BatchCall, BatchValidationError, Block,
    BlockHeader, ExecutionLog, ExecutionReceipt, QaPoolVoteKind, Transaction, TransactionPayload,
    CONTRACT_DEPLOY_INIT_CODE_MARKER, MAX_BATCH_CALLDATA_BYTES, MAX_BATCH_CALLS,
    MAX_EXECUTION_LOGS_PER_TX, MAX_EXECUTION_LOG_DATA_BYTES, MAX_EXECUTION_LOG_TOPICS,
    MAX_RECEIPT_ERROR_STRING_BYTES, MAX_RECEIPT_RETURN_DATA_BYTES,
};
pub use types::{Account, AccountState};
pub use validator_registration::{
    sign_validator_registration, sign_validator_registration_v2, validator_registration_message,
    validator_registration_message_v2, verify_validator_registration, verify_validator_registration_v2,
    VALIDATOR_REGISTRATION_DOMAIN, VALIDATOR_REGISTRATION_DOMAIN_V2,
};
pub use vrf_proof::VrfProofGossip;

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_hash_deterministic() {
        let mut h1 = hasher();
        h1.update(b"hello");
        let out1 = h1.finalize();
        let mut h2 = hasher();
        h2.update(b"hello");
        let out2 = h2.finalize();
        assert_eq!(out1.as_bytes(), out2.as_bytes());
    }

    #[test]
    fn test_transaction_id_deterministic() {
        let id = AccountId::from_bytes([1u8; 32]);
        let tx = Transaction {
            nonce: 0,
            sender: id,
            payload: TransactionPayload::Transfer {
                to: id,
                amount: 100,
            },
            access_list: AccessList::default(),
        };
        assert_eq!(tx.id(), tx.id());
    }

    #[test]
    fn test_signed_transaction_verify() {
        let key = SigningKey::generate(&mut OsRng);
        let sender = AccountId::from_bytes(key.verifying_key().to_bytes());
        let tx = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::Transfer {
                to: AccountId::from_bytes([2u8; 32]),
                amount: 50,
            },
            access_list: AccessList::default(),
        };
        let signed = SignedTransaction::new(tx, &key);
        assert!(signed.verify().is_ok());
    }

    #[test]
    fn test_access_list_conflicts() {
        let a = AccountId::from_bytes([1u8; 32]);
        let b = AccountId::from_bytes([2u8; 32]);
        let c = AccountId::from_bytes([3u8; 32]);
        let al1 = AccessList::new(vec![a, b], vec![]);
        let al2 = AccessList::new(vec![c], vec![]);
        assert!(!al1.conflicts_with(&al2));
        let al3 = AccessList::new(vec![b, c], vec![]);
        assert!(al1.conflicts_with(&al3));
    }

    #[test]
    fn contract_deploy_init_marker_helpers() {
        assert!(!contract_deploy_uses_init_code(&[0x00]));
        assert!(contract_deploy_uses_init_code(&[
            CONTRACT_DEPLOY_INIT_CODE_MARKER,
            0x00
        ]));
        assert_eq!(contract_deploy_init_body(&[0x60, 0]), &[0x60, 0]);
        assert_eq!(
            contract_deploy_init_body(&[CONTRACT_DEPLOY_INIT_CODE_MARKER, 0x60, 0]),
            &[0x60, 0]
        );
    }

    #[test]
    fn test_create2_contract_address_deterministic() {
        let d = AccountId::from_bytes([3u8; 32]);
        let salt = [5u8; 32];
        let code = vec![0x60, 0x00];
        let a = create2_contract_address(&d, &salt, &code);
        let b = create2_contract_address(&d, &salt, &code);
        assert_eq!(a, b);
        assert_ne!(create2_contract_address(&d, &salt, &vec![0x61]), a);
    }

    #[test]
    fn test_suggested_parallel_access_list() {
        let sender = AccountId::from_bytes([1u8; 32]);
        let to = AccountId::from_bytes([2u8; 32]);
        let contract = AccountId::from_bytes([4u8; 32]);
        let t_transfer = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::Transfer { to, amount: 1 },
            access_list: AccessList::default(),
        };
        let sug = t_transfer.suggested_parallel_access_list();
        assert!(sug.read.contains(&sender) && sug.read.contains(&to));
        assert!(!t_transfer.access_list_covers_parallel_suggestion());
        let t_ok = Transaction {
            access_list: sug.clone(),
            ..t_transfer.clone()
        };
        assert!(t_ok.access_list_covers_parallel_suggestion());
        let t_call = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractCall {
                contract,
                calldata: vec![],
            },
            access_list: AccessList::new(vec![sender, contract], vec![sender, contract]),
        };
        assert!(t_call.access_list_covers_parallel_suggestion());
    }

    #[test]
    fn test_suggested_parallel_access_list_create2_includes_predicted_contract() {
        let sender = AccountId::from_bytes([7u8; 32]);
        let salt = [9u8; 32];
        let bytecode = vec![0x60u8, 0x00];
        let predicted = create2_contract_address(&sender, &salt, &bytecode);
        let t = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::ContractDeploy {
                bytecode: bytecode.clone(),
                create2_salt: Some(salt),
            },
            access_list: AccessList::default(),
        };
        let sug = t.suggested_parallel_access_list();
        assert!(sug.read.contains(&sender));
        assert!(sug.read.contains(&predicted));
        assert!(sug.write.contains(&sender));
        assert!(sug.write.contains(&predicted));
    }
}
