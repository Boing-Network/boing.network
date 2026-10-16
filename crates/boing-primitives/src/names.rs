//! Boing name registry: shared, consensus-relevant rules (label format, rent, commitments).
//!
//! The registry state lives in `boing-state` (`NameRegistry`); these helpers are used by the VM,
//! RPC, CLI and (mirrored) the TypeScript SDK, so they must stay deterministic and dependency-free.

use crate::{hasher, AccountId};

/// Display / input suffix. Stored labels never include it.
pub const NAME_SUFFIX: &str = ".boing";
pub const NAME_MIN_LEN: usize = 3;
pub const NAME_MAX_LEN: usize = 32;
/// Registration / renewal term bounds (years) and the cap on total prepaid time.
pub const NAME_MIN_YEARS: u8 = 1;
pub const NAME_MAX_YEARS: u8 = 10;
/// Domain separator for registration commitments.
pub const NAME_COMMIT_DOMAIN: &[u8] = b"BOING_NAME_COMMIT_V1";

/// Labels only governance could assign (not registrable by users in v1).
pub const RESERVED_NAMES: &[&str] = &[
    "boing",
    "admin",
    "administrator",
    "root",
    "system",
    "validator",
    "validators",
    "faucet",
    "observer",
    "wallet",
    "support",
    "help",
    "official",
    "team",
    "dex",
    "finance",
    "express",
    "network",
    "treasury",
    "governance",
    "security",
    "staking",
    "bridge",
    "testnet",
    "mainnet",
    "null",
    "undefined",
    "anchor",
    "registry",
    "names",
];

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("name must be {NAME_MIN_LEN}-{NAME_MAX_LEN} characters")]
    Length,
    #[error("name may only contain a-z, 0-9 and '-'")]
    Charset,
    #[error("name cannot start or end with '-' or contain '--'")]
    Hyphen,
    #[error("name cannot look like an address or be purely numeric")]
    LooksLikeAddress,
    #[error("name is reserved")]
    Reserved,
}

/// Normalize user input into a stored label: trim, lowercase ASCII, drop a trailing `.boing`,
/// then validate. Does **not** check the reserved list (see [`check_registrable`]).
pub fn normalize_name(input: &str) -> Result<String, NameError> {
    let mut s = input.trim().to_ascii_lowercase();
    if let Some(stripped) = s.strip_suffix(NAME_SUFFIX) {
        s = stripped.to_string();
    }
    validate_label(&s)?;
    Ok(s)
}

/// Validate an already-normalized label.
pub fn validate_label(label: &str) -> Result<(), NameError> {
    let len = label.len();
    if !(NAME_MIN_LEN..=NAME_MAX_LEN).contains(&len) {
        return Err(NameError::Length);
    }
    if !label
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(NameError::Charset);
    }
    if label.starts_with('-') || label.ends_with('-') || label.contains("--") {
        return Err(NameError::Hyphen);
    }
    if label.starts_with("0x")
        || label.bytes().all(|b| b.is_ascii_digit())
        || (len >= 32 && label.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(NameError::LooksLikeAddress);
    }
    Ok(())
}

pub fn is_reserved(label: &str) -> bool {
    RESERVED_NAMES.contains(&label)
}

/// Full check for a user registration: valid label and not reserved.
pub fn check_registrable(label: &str) -> Result<(), NameError> {
    validate_label(label)?;
    if is_reserved(label) {
        return Err(NameError::Reserved);
    }
    Ok(())
}

/// Yearly rent in whole BOING (testnet values): 3 chars 10, 4 chars 5, 5 chars 2, 6+ chars 1.
/// The rent is **burned** (removed from the payer's balance, credited to nobody).
pub fn name_rent_per_year(label: &str) -> u128 {
    match label.len() {
        0..=3 => 10,
        4 => 5,
        5 => 2,
        _ => 1,
    }
}

/// Rent for `years` (no overflow: years <= 255).
pub fn name_rent(label: &str, years: u8) -> u128 {
    name_rent_per_year(label) * years as u128
}

/// Commitment for commit-reveal: `blake3(domain ‖ len(label) ‖ label ‖ owner ‖ salt)`.
pub fn name_commitment(label: &str, owner: &AccountId, salt: &[u8; 32]) -> [u8; 32] {
    let mut h = hasher();
    h.update(NAME_COMMIT_DOMAIN);
    h.update(&(label.len() as u32).to_le_bytes());
    h.update(label.as_bytes());
    h.update(&owner.0);
    h.update(salt);
    *h.finalize().as_bytes()
}

/// `label.boing`
pub fn display_name(label: &str) -> String {
    format!("{label}{NAME_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates() {
        assert_eq!(normalize_name("  Alice.BOING ").unwrap(), "alice");
        assert_eq!(normalize_name("bob-42").unwrap(), "bob-42");
        assert_eq!(normalize_name("ab"), Err(NameError::Length));
        assert_eq!(normalize_name(&"a".repeat(33)), Err(NameError::Length));
        assert_eq!(normalize_name(&"z".repeat(32)).unwrap().len(), 32);
        assert_eq!(normalize_name("al_ice"), Err(NameError::Charset));
        assert_eq!(normalize_name("аlice"), Err(NameError::Charset)); // Cyrillic а
        assert_eq!(normalize_name("al ice"), Err(NameError::Charset));
        assert_eq!(normalize_name("-alice"), Err(NameError::Hyphen));
        assert_eq!(normalize_name("alice-"), Err(NameError::Hyphen));
        assert_eq!(normalize_name("al--ice"), Err(NameError::Hyphen));
        assert_eq!(normalize_name("0xabc"), Err(NameError::LooksLikeAddress));
        assert_eq!(normalize_name("12345"), Err(NameError::LooksLikeAddress));
        assert_eq!(
            normalize_name(&"ab".repeat(16)),
            Err(NameError::LooksLikeAddress)
        );
        assert!(normalize_name("abc123").is_ok());
    }

    #[test]
    fn reserved_names() {
        assert_eq!(check_registrable("boing"), Err(NameError::Reserved));
        assert_eq!(check_registrable("faucet"), Err(NameError::Reserved));
        assert!(check_registrable("boinger").is_ok());
    }

    #[test]
    fn rent_table() {
        assert_eq!(name_rent_per_year("abc"), 10);
        assert_eq!(name_rent_per_year("abcd"), 5);
        assert_eq!(name_rent_per_year("abcde"), 2);
        assert_eq!(name_rent_per_year("abcdef"), 1);
        assert_eq!(name_rent_per_year(&"a".repeat(32)), 1);
        assert_eq!(name_rent("abc", 3), 30);
    }

    #[test]
    fn commitment_binds_label_owner_salt() {
        let o = AccountId([1; 32]);
        let s = [7u8; 32];
        let c = name_commitment("alice", &o, &s);
        assert_eq!(c, name_commitment("alice", &o, &s));
        assert_ne!(c, name_commitment("alicf", &o, &s));
        assert_ne!(c, name_commitment("alice", &AccountId([2; 32]), &s));
        assert_ne!(c, name_commitment("alice", &o, &[8u8; 32]));
    }

    /// Bincode of every name payload, pinned for the TypeScript SDK encoder (`boingNames.test.ts`).
    #[test]
    fn payload_bincode_vectors() {
        use crate::TransactionPayload as P;
        let a = AccountId([0x11; 32]);
        let cases = [
            P::NameCommit {
                commitment: [0x33; 32],
            },
            P::NameRegister {
                name: "alice".into(),
                salt: [0x22; 32],
                target: a,
                years: 2,
            },
            P::NameRenew {
                name: "alice".into(),
                years: 3,
            },
            P::NameTransfer {
                name: "bob".into(),
                new_owner: a,
            },
            P::NameSetTarget {
                name: "bob".into(),
                target: a,
            },
            P::NameSetPrimary {
                name: Some("bob".into()),
            },
            P::NameSetPrimary { name: None },
            P::NameRelease { name: "bob".into() },
        ];
        let got: Vec<String> = cases
            .iter()
            .map(|c| hex::encode(bincode::serialize(c).unwrap()))
            .collect();
        if std::env::var("PRINT_VECTORS").is_ok() {
            for g in &got {
                println!("VEC {g}");
            }
        }
        assert_eq!(got.len(), PAYLOAD_VECTORS.len());
        for (g, w) in got.iter().zip(PAYLOAD_VECTORS) {
            assert_eq!(g, w);
        }
    }

    /// Fixed vector shared with the TypeScript SDK (`boingNames.test.ts`).
    #[test]
    fn commitment_vector() {
        let c = name_commitment("alice", &AccountId([0x11; 32]), &[0x22; 32]);
        assert_eq!(hex::encode(c), NAME_COMMIT_VECTOR_ALICE);
    }
}

/// `name_commitment("alice", 0x11×32, 0x22×32)`, pinned for cross-language tests.
#[cfg(test)]
const PAYLOAD_VECTORS: &[&str] = &[
    "0d0000003333333333333333333333333333333333333333333333333333333333333333",
    "0e0000000500000000000000616c6963652222222222222222222222222222222222222222222222222222222222222222111111111111111111111111111111111111111111111111111111111111111102",
    "0f0000000500000000000000616c69636503",
    "100000000300000000000000626f621111111111111111111111111111111111111111111111111111111111111111",
    "110000000300000000000000626f621111111111111111111111111111111111111111111111111111111111111111",
    "12000000010300000000000000626f62",
    "1200000000",
    "130000000300000000000000626f62",
];

#[cfg(test)]
const NAME_COMMIT_VECTOR_ALICE: &str =
    "92db9158685ad72efbc250ab5edf055da017cf0ee3819619effa69db3718bc1d";
