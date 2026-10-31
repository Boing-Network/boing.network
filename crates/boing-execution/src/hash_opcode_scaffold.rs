//! Activation-gated **hash opcode** scaffolding for full 32-byte allowance keys
//! (audit #61 item 29).
//!
//! Live reference-token / wBOING templates still use
//! [`crate::reference_token_v2::ref_fungible_allowance_key`] (16-byte truncate + XOR). A hashed
//! key needs either a VM hash opcode or a multi-slot layout. This module locks the planned
//! opcode byte, activation env, and a **host-side** BLAKE3 key helper for future template v3
//! tests — it is **not** callable from contract bytecode until the opcode ships.
//!
//! See [`docs/AUDIT-61-PROTOCOL-SEMANTICS.md`](../../../docs/AUDIT-61-PROTOCOL-SEMANTICS.md).

/// Env var for the height at which a general-purpose BLAKE3 hash opcode would activate.
///
/// Unset / empty / `never` → never (default). Shipping the opcode is a consensus hard fork.
pub const BLAKE3_OPCODE_ACTIVATION_ENV: &str = "BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT";

/// Planned opcode byte for `Blake3` (hash 32-byte memory word → 32-byte digest on stack).
/// Chosen in the unused 0x20–0x2f band; must be appended to `Opcode` + JS tables together.
pub const PLANNED_BLAKE3_OPCODE: u8 = 0x20;

/// XOR used by a future hashed allowance layout (distinct from the v2 truncate XOR).
pub const REF_FUNGIBLE_ALLOWANCE_V3_DOMAIN: &[u8] = b"BOING_REF_FUNGIBLE_ALLOWANCE_V3";

/// Host-side allowance storage key for a future fungible **v3** template:
/// `BLAKE3(domain ‖ owner ‖ spender)`.
///
/// Not used by deployed v2 bytecode. Once [`PLANNED_BLAKE3_OPCODE`] is live, on-chain approve /
/// transfer_from can compute the same key without truncating addresses.
#[must_use]
pub fn ref_fungible_allowance_key_v3_host(
    owner: &boing_primitives::AccountId,
    spender: &boing_primitives::AccountId,
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(REF_FUNGIBLE_ALLOWANCE_V3_DOMAIN);
    h.update(&owner.0);
    h.update(&spender.0);
    *h.finalize().as_bytes()
}

/// Parse activation height (`None` = never).
pub fn parse_blake3_activation_height(raw: Option<&str>) -> Result<Option<u64>, String> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.eq_ignore_ascii_case("never") || s.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    s.parse::<u64>()
        .map(Some)
        .map_err(|e| format!("invalid {BLAKE3_OPCODE_ACTIVATION_ENV}: {e}"))
}

#[must_use]
pub fn blake3_opcode_active(activation: Option<u64>, height: u64) -> bool {
    match activation {
        Some(h) => height >= h,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::AccountId;

    #[test]
    fn activation_defaults_off() {
        assert_eq!(parse_blake3_activation_height(None).unwrap(), None);
        assert!(!blake3_opcode_active(None, 0));
        assert!(blake3_opcode_active(Some(10), 10));
        assert!(!blake3_opcode_active(Some(10), 9));
    }

    #[test]
    fn host_key_is_stable_and_distinct_from_xor_truncate() {
        let o = AccountId([0x11; 32]);
        let s = AccountId([0x22; 32]);
        let k1 = ref_fungible_allowance_key_v3_host(&o, &s);
        let k2 = ref_fungible_allowance_key_v3_host(&o, &s);
        assert_eq!(k1, k2);
        let swapped = ref_fungible_allowance_key_v3_host(&s, &o);
        assert_ne!(k1, swapped);
        // Differ from v2 truncate key for the same pair.
        let v2 = crate::reference_token_v2::ref_fungible_allowance_key(&o, &s);
        assert_ne!(k1, v2);
    }

    #[test]
    fn planned_opcode_unused_in_current_isa() {
        assert!(crate::bytecode::Opcode::from_byte(PLANNED_BLAKE3_OPCODE).is_none());
    }
}
