//! Protocol Quality Assurance — deterministic checks for asset deployment.
//!
//! Modules:
//! - `pool` — Community QA pool (pending queue, voting, resolution)
//!
//! See [QUALITY-ASSURANCE-NETWORK.md](https://github.com/Boing-Network/boing.network/blob/main/docs/QUALITY-ASSURANCE-NETWORK.md) for the full design.
//!
//! This crate provides:
//! - [QaResult]: Allow | Reject | Unsure
//! - [RuleId] and [QaReject] for structured rejection
//! - [check_contract_deploy]: full implementation (bytecode size, opcode whitelist, well-formedness, blocklist)
//! - Purpose declaration is optional; when provided and invalid, Reject. Missing is allowed (§9.4).

/// Outcome of a QA check: allow deployment, reject, or send to community pool (unsure).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QaResult {
    /// Deployment passes all checks; allow inclusion.
    Allow,
    /// Deployment fails a rule; reject with reason.
    Reject(QaReject),
    /// Automation cannot firmly decide; refer to community QA pool.
    Unsure,
}

/// Structured rejection reason for diagnostics and RPC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QaReject {
    pub rule_id: RuleId,
    pub message: String,
    /// Optional link to QA docs (deployer checklist in QUALITY-ASSURANCE-NETWORK) for actionable feedback.
    pub doc_url: Option<String>,
}

/// Doc URL for QA guidance (QUALITY-ASSURANCE-NETWORK Appendix A: Deployer checklist). Override with env or config in production.
pub const QA_PASS_GUIDE_URL: &str = "https://github.com/Boing-Network/boing.network/blob/main/docs/QUALITY-ASSURANCE-NETWORK.md#appendix-a-deployer-checklist-how-to-pass-qa";

fn doc_url_for_rule(_rule_id: &str) -> Option<String> {
    Some(format!("{}#common-rejections-and-fixes", QA_PASS_GUIDE_URL))
}

impl std::fmt::Display for QaReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} — {}", self.rule_id.0, self.message)?;
        if let Some(ref u) = self.doc_url {
            write!(f, " See {}", u)?;
        }
        Ok(())
    }
}

impl QaReject {
    fn new(rule_id: RuleId, message: String) -> Self {
        Self {
            doc_url: doc_url_for_rule(&rule_id.0),
            rule_id,
            message,
        }
    }
}

/// Identifies a QA rule (e.g. max size, opcode whitelist, blocklist).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RuleId(pub String);

impl RuleId {
    pub const MAX_BYTECODE_SIZE: &'static str = "MAX_BYTECODE_SIZE";
    pub const INVALID_OPCODE: &'static str = "INVALID_OPCODE";
    pub const MALFORMED_BYTECODE: &'static str = "MALFORMED_BYTECODE";
    pub const BLOCKLIST_MATCH: &'static str = "BLOCKLIST_MATCH";
    pub const SCAM_PATTERN_MATCH: &'static str = "SCAM_PATTERN_MATCH";
    pub const PURPOSE_DECLARATION_INVALID: &'static str = "PURPOSE_DECLARATION_INVALID";
    pub const ALWAYS_REVIEW_CATEGORY: &'static str = "ALWAYS_REVIEW_CATEGORY";
    pub const SOFT_RULE_FAILED: &'static str = "SOFT_RULE_FAILED";
    /// Asset name or symbol exceeds max length.
    pub const METADATA_TOO_LONG: &'static str = "METADATA_TOO_LONG";
    /// Asset name or symbol contains governance-forbidden content (vulgarity, offensiveness, etc.).
    pub const CONTENT_POLICY_VIOLATION: &'static str = "CONTENT_POLICY_VIOLATION";
}

fn is_valid_opcode(b: u8) -> bool {
    matches!(
        b,
        0x00 | 0x01 | 0x02 | 0x03 | 0x04 | 0x06 | 0x08 | 0x09 | 0x10 | 0x11 | 0x14 | 0x15 | 0x16 | 0x17
            | 0x18 | 0x19 | 0x1b | 0x1c | 0x1d
            | 0x30 | 0x33 | 0x80 | 0xa0 | 0xa1 | 0xa2 | 0xa3 | 0xa4 | 0x51
            | 0x52 | 0x54 | 0x55 | 0x56 | 0x57 | 0xf1 | 0xf3 | 0xf5
    ) || (0x60..=0x7f).contains(&b)
}

fn push_immediate_size(b: u8) -> Option<u8> {
    if (0x60..=0x7f).contains(&b) {
        Some(b - 0x5f) // PUSH1=1, PUSH32=32
    } else {
        None
    }
}

/// Validation error: (offset, is_invalid_opcode, message)
fn check_well_formed(bytecode: &[u8]) -> Result<(), (usize, bool, &'static str)> {
    let mut pc = 0;
    while pc < bytecode.len() {
        let op = bytecode[pc];
        if !is_valid_opcode(op) {
            return Err((pc, true, "Invalid opcode"));
        }
        if let Some(imm_size) = push_immediate_size(op) {
            let imm_size = imm_size as usize;
            pc += 1;
            if pc + imm_size > bytecode.len() {
                return Err((pc, false, "Truncated PUSH immediate"));
            }
            pc += imm_size;
        } else {
            pc += 1;
        }
    }
    if pc != bytecode.len() {
        return Err((pc, false, "Trailing bytes or malformed instruction stream"));
    }
    Ok(())
}

/// Default maximum bytecode size (bytes). Governance can change via rule registry.
pub const DEFAULT_MAX_BYTECODE_SIZE: usize = 32 * 1024; // 32 KiB

/// Max length for asset_name (UTF-8 bytes). Must match boing_primitives::TransactionPayload::MAX_ASSET_NAME_LEN.
pub const MAX_ASSET_NAME_LEN: usize = 256;
/// Max length for asset_symbol (UTF-8 bytes). Must match boing_primitives::TransactionPayload::MAX_ASSET_SYMBOL_LEN.
pub const MAX_ASSET_SYMBOL_LEN: usize = 32;

/// Valid purpose categories per QUALITY-ASSURANCE-NETWORK.md §5.3, §10 (meme leniency).
pub const VALID_PURPOSE_CATEGORIES: &[&str] = &[
    "dapp", "dApp", "token", "nft", "NFT", "meme", "community", "entertainment",
    "tooling", "other",
];

fn is_valid_purpose_category(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    VALID_PURPOSE_CATEGORIES
        .iter()
        .any(|c| c.to_lowercase() == s)
}

/// Soft rules: may return Unsure when automation lacks sufficient knowledge.
/// Per spec §7.4: ambiguous declaration, minimal description with "other", etc.
fn check_soft_rules(
    _bytecode: &[u8],
    purpose_category: Option<&str>,
    description_hash: Option<&[u8]>,
) -> Option<QaResult> {
    // Soft rule: "other" category with empty/minimal description → Unsure (§7.4, §11.2)
    if let Some(cat) = purpose_category {
        if cat.trim().to_lowercase() == "other" {
            let desc_len = description_hash.map(|d| d.len()).unwrap_or(0);
            if desc_len < 4 {
                return Some(QaResult::Unsure);
            }
        }
    }
    None
}

/// Check ContractDeploy bytecode and optional purpose declaration.
///
/// Rules applied (in order):
/// 1. Empty bytecode → Reject
/// 2. Size over max → Reject
/// 3. Invalid opcode → Reject
/// 4. Malformed (truncated PUSH, trailing bytes) → Reject
/// 5. Blocklist match → Reject
/// 6. Purpose declaration invalid (when provided) → Reject
/// 7. All pass → Allow
///
/// Purpose is optional (§9.4): if missing, Allow when other checks pass. Meme/community/entertainment are valid.
pub fn check_contract_deploy(
    bytecode: &[u8],
    purpose_category: Option<&str>,
    _description_hash: Option<&[u8]>,
    max_bytecode_size: usize,
) -> QaResult {
    check_contract_deploy_with_blocklist(bytecode, purpose_category, max_bytecode_size, &[])
}

/// Full check using the complete rule registry. Applies all rules: hard rules, blocklist,
/// scam patterns, always-review categories, content policy (vulgarity/offensiveness), and soft rules.
pub fn check_contract_deploy_full(
    bytecode: &[u8],
    purpose_category: Option<&str>,
    description_hash: Option<&[u8]>,
    registry: &RuleRegistry,
) -> QaResult {
    check_contract_deploy_full_with_metadata(
        bytecode,
        purpose_category,
        description_hash,
        None,
        None,
        registry,
    )
}

/// Full check with optional deploy-time metadata (asset_name, asset_symbol) for content policy.
/// If asset_name or asset_symbol exceed max length, or contain governance-forbidden strings, Reject.
pub fn check_contract_deploy_full_with_metadata(
    bytecode: &[u8],
    purpose_category: Option<&str>,
    description_hash: Option<&[u8]>,
    asset_name: Option<&str>,
    asset_symbol: Option<&str>,
    registry: &RuleRegistry,
) -> QaResult {
    // Metadata length (when provided)
    if let Some(name) = asset_name {
        if name.len() > MAX_ASSET_NAME_LEN {
            return QaResult::Reject(QaReject::new(
                RuleId(RuleId::METADATA_TOO_LONG.to_string()),
                format!("asset_name length {} exceeds max {}", name.len(), MAX_ASSET_NAME_LEN),
            ));
        }
    }
    if let Some(sym) = asset_symbol {
        if sym.len() > MAX_ASSET_SYMBOL_LEN {
            return QaResult::Reject(QaReject::new(
                RuleId(RuleId::METADATA_TOO_LONG.to_string()),
                format!("asset_symbol length {} exceeds max {}", sym.len(), MAX_ASSET_SYMBOL_LEN),
            ));
        }
    }

    let base = check_contract_deploy_with_blocklist(
        bytecode,
        purpose_category,
        registry.max_bytecode_size(),
        registry.blocklist(),
    );
    if let QaResult::Reject(_) = base {
        return base;
    }

    // Scam patterns: byte sequences that if found → Reject
    for pattern in registry.scam_patterns() {
        if pattern.len() <= bytecode.len() && bytecode.windows(pattern.len()).any(|w| w == pattern) {
            return QaResult::Reject(QaReject::new(
                RuleId(RuleId::SCAM_PATTERN_MATCH.to_string()),
                "Bytecode contains known scam/malware pattern".to_string(),
            ));
        }
    }

    // Content policy: vulgarity / offensiveness blocklist (governance-mutable)
    if let Some(reject) = check_content_policy(asset_name, asset_symbol, registry.content_blocklist()) {
        return QaResult::Reject(reject);
    }

    // Policy "always review" categories → Unsure
    if let Some(cat) = purpose_category {
        let cat_lower = cat.trim().to_lowercase();
        if registry.always_review_categories().contains(&cat_lower) {
            return QaResult::Unsure;
        }
    }

    // Soft rules: may return Unsure (e.g. ambiguous declaration)
    if let Some(unsure) = check_soft_rules(bytecode, purpose_category, description_hash) {
        return unsure;
    }

    QaResult::Allow
}

/// Check asset name/symbol against governance content blocklist (forbidden substrings).
/// Case-insensitive. Returns Some(QaReject) if any forbidden term is contained.
fn check_content_policy(
    asset_name: Option<&str>,
    asset_symbol: Option<&str>,
    forbidden_terms: &[String],
) -> Option<QaReject> {
    if forbidden_terms.is_empty() {
        return None;
    }
    let check = |s: &str| {
        let lower = s.trim().to_lowercase();
        for term in forbidden_terms {
            if !term.is_empty() && lower.contains(&term.to_lowercase()) {
                return Some(QaReject::new(
                    RuleId(RuleId::CONTENT_POLICY_VIOLATION.to_string()),
                    "Deployment metadata contains governance-forbidden content (vulgarity/offensiveness policy)"
                        .to_string(),
                ));
            }
        }
        None
    };
    if let Some(name) = asset_name {
        if let Some(r) = check(name) {
            return Some(r);
        }
    }
    if let Some(sym) = asset_symbol {
        if let Some(r) = check(sym) {
            return Some(r);
        }
    }
    None
}

/// Same as [check_contract_deploy] but with an optional blocklist of bytecode hashes (e.g. known scams).
pub fn check_contract_deploy_with_blocklist(
    bytecode: &[u8],
    purpose_category: Option<&str>,
    max_bytecode_size: usize,
    blocklist_hashes: &[[u8; 32]],
) -> QaResult {
    use boing_primitives::{contract_deploy_init_body, contract_deploy_uses_init_code};

    if bytecode.is_empty() {
        return QaResult::Reject(QaReject::new(
            RuleId(RuleId::MALFORMED_BYTECODE.to_string()),
            "Bytecode must not be empty".to_string(),
        ));
    }
    if contract_deploy_uses_init_code(bytecode) && contract_deploy_init_body(bytecode).is_empty() {
        return QaResult::Reject(QaReject::new(
            RuleId(RuleId::MALFORMED_BYTECODE.to_string()),
            "Init-code marker present but no init bytecode after prefix".to_string(),
        ));
    }
    if bytecode.len() > max_bytecode_size {
        return QaResult::Reject(QaReject::new(
            RuleId(RuleId::MAX_BYTECODE_SIZE.to_string()),
            format!(
                "Bytecode size {} exceeds maximum {}",
                bytecode.len(),
                max_bytecode_size
            ),
        ));
    }

    let wellformed = contract_deploy_init_body(bytecode);
    if let Err((offset, invalid_opcode, reason)) = check_well_formed(wellformed) {
        let rule_id = if invalid_opcode {
            RuleId(RuleId::INVALID_OPCODE.to_string())
        } else {
            RuleId(RuleId::MALFORMED_BYTECODE.to_string())
        };
        return QaResult::Reject(QaReject::new(rule_id, format!("{} at offset {}", reason, offset)));
    }

    // Blocklist: compare BLAKE3 hash of bytecode against known-bad hashes.
    use boing_primitives::hasher;
    let mut h = hasher();
    h.update(bytecode);
    let hash = h.finalize();
    let hash_arr: [u8; 32] = *hash.as_bytes();
    for blocked in blocklist_hashes {
        if hash_arr == *blocked {
            return QaResult::Reject(QaReject::new(
                RuleId(RuleId::BLOCKLIST_MATCH.to_string()),
                "Bytecode matches blocklist (known scam/malware)".to_string(),
            ));
        }
    }

    // Purpose: if provided, must be valid. Missing is allowed (§9.4, §11).
    if let Some(cat) = purpose_category {
        if !cat.trim().is_empty() && !is_valid_purpose_category(cat) {
            return QaResult::Reject(QaReject::new(
                RuleId(RuleId::PURPOSE_DECLARATION_INVALID.to_string()),
                format!(
                    "Invalid purpose category '{}'; valid: dApp, token, NFT, meme, community, entertainment, tooling, other",
                    cat.trim()
                ),
            ));
        }
    }

    QaResult::Allow
}

/// In-memory rule registry. Production: on-chain or governance-driven registry.
/// Governance can update blocklist, scam_patterns, always_review_categories, and content_blocklist
/// via proposals (target_key "qa_registry", value = serialized RuleRegistry).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RuleRegistry {
    max_bytecode_size: usize,
    blocklist: Vec<[u8; 32]>,
    /// Byte sequences that if found in bytecode → Reject (legitimacy heuristic).
    scam_patterns: Vec<Vec<u8>>,
    /// Purpose categories that always go to pool (policy-required review).
    always_review_categories: std::collections::HashSet<String>,
    /// Forbidden substrings in asset name/symbol (vulgarity, offensiveness). Case-insensitive match. Governance-mutable.
    content_blocklist: Vec<String>,
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self {
            max_bytecode_size: DEFAULT_MAX_BYTECODE_SIZE,
            blocklist: Vec::new(),
            scam_patterns: Vec::new(),
            always_review_categories: std::collections::HashSet::new(),
            content_blocklist: Vec::new(),
        }
    }

    pub fn with_max_bytecode_size(mut self, size: usize) -> Self {
        self.max_bytecode_size = size;
        self
    }

    pub fn with_blocklist(mut self, hashes: Vec<[u8; 32]>) -> Self {
        self.blocklist = hashes;
        self
    }

    pub fn with_scam_patterns(mut self, patterns: Vec<Vec<u8>>) -> Self {
        self.scam_patterns = patterns;
        self
    }

    pub fn with_always_review_categories(mut self, categories: impl IntoIterator<Item = String>) -> Self {
        self.always_review_categories = categories.into_iter().collect();
        self
    }

    /// Set the content blocklist (forbidden terms for asset name/symbol). Replaces existing.
    pub fn with_content_blocklist(mut self, terms: impl IntoIterator<Item = String>) -> Self {
        self.content_blocklist = terms.into_iter().filter(|s| !s.is_empty()).collect();
        self
    }

    pub fn add_blocklist_entry(&mut self, hash: [u8; 32]) {
        if !self.blocklist.iter().any(|h| h == &hash) {
            self.blocklist.push(hash);
        }
    }

    pub fn add_scam_pattern(&mut self, pattern: Vec<u8>) {
        if !pattern.is_empty() && !self.scam_patterns.iter().any(|p| p == &pattern) {
            self.scam_patterns.push(pattern);
        }
    }

    pub fn add_always_review_category(&mut self, category: impl Into<String>) {
        self.always_review_categories.insert(category.into().to_lowercase());
    }

    /// Add a forbidden term to the content blocklist (governance-mutable). Case-insensitive.
    pub fn add_forbidden_content(&mut self, term: impl Into<String>) {
        let t = term.into().trim().to_string();
        if !t.is_empty() && !self.content_blocklist.iter().any(|s| s.eq_ignore_ascii_case(&t)) {
            self.content_blocklist.push(t);
        }
    }

    /// Remove a term from the content blocklist. Returns true if removed.
    pub fn remove_forbidden_content(&mut self, term: &str) -> bool {
        let lower = term.trim().to_lowercase();
        if let Some(pos) = self.content_blocklist.iter().position(|s| s.to_lowercase() == lower) {
            self.content_blocklist.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn max_bytecode_size(&self) -> usize {
        self.max_bytecode_size
    }

    pub fn blocklist(&self) -> &[[u8; 32]] {
        &self.blocklist
    }

    pub fn scam_patterns(&self) -> &[Vec<u8>] {
        &self.scam_patterns
    }

    pub fn always_review_categories(&self) -> &std::collections::HashSet<String> {
        &self.always_review_categories
    }

    pub fn content_blocklist(&self) -> &[String] {
        &self.content_blocklist
    }
}

pub mod pool;
pub mod pool_config;

pub use pool_config::{
    qa_pool_config_from_json, QaPoolExpiryPolicy, QaPoolGovernanceConfig, GOVERNANCE_QA_POOL_CONFIG_KEY,
};

/// Governance target key for updating the QA rule registry. When a governance proposal is executed
/// with this key, the node (or operator) should replace the in-memory RuleRegistry with the
/// deserialized value. Value format: JSON-serialized RuleRegistry (see [RuleRegistry] and serde).
pub const GOVERNANCE_QA_REGISTRY_KEY: &str = "qa_registry";

/// Load a [RuleRegistry] from JSON bytes (e.g. governance proposal target_value).
/// Use this when applying an executed governance proposal to update QA rules.
pub fn rule_registry_from_json(bytes: &[u8]) -> Result<RuleRegistry, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::CONTRACT_DEPLOY_INIT_CODE_MARKER;

    #[test]
    fn reject_init_marker_without_body() {
        let r = check_contract_deploy(
            &[CONTRACT_DEPLOY_INIT_CODE_MARKER],
            None,
            None,
            DEFAULT_MAX_BYTECODE_SIZE,
        );
        assert!(matches!(r, QaResult::Reject(ref rej) if rej.rule_id.0 == RuleId::MALFORMED_BYTECODE));
    }

    #[test]
    fn allow_init_marker_with_valid_body() {
        let mut b = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
        b.push(0x00); // STOP-only init → empty runtime
        let r = check_contract_deploy(&b, None, None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Allow));
    }

    #[test]
    fn reject_empty_bytecode() {
        let r = check_contract_deploy(&[], None, None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Reject(_)));
    }

    #[test]
    fn reject_over_size() {
        let big = vec![0u8; DEFAULT_MAX_BYTECODE_SIZE + 1];
        let r = check_contract_deploy(&big, None, None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Reject(_)));
    }

    #[test]
    fn allow_small_bytecode() {
        let r = check_contract_deploy(&[0x00], None, None, DEFAULT_MAX_BYTECODE_SIZE); // STOP
        assert!(matches!(r, QaResult::Allow));
    }

    #[test]
    fn reject_invalid_opcode() {
        let r = check_contract_deploy(&[0xff], None, None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Reject(ref rej) if rej.rule_id.0 == RuleId::INVALID_OPCODE));
    }

    #[test]
    fn reject_truncated_push() {
        // PUSH2 needs 2 bytes of immediate; we only have 1
        let r = check_contract_deploy(&[0x61, 0x00], None, None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Reject(ref rej) if rej.rule_id.0 == RuleId::MALFORMED_BYTECODE));
    }

    #[test]
    fn allow_valid_push() {
        // PUSH1 0x00
        let r = check_contract_deploy(&[0x60, 0x00], None, None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Allow));
    }

    #[test]
    fn allow_valid_purpose() {
        let r = check_contract_deploy(&[0x00], Some("meme"), None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Allow));
    }

    #[test]
    fn reject_invalid_purpose() {
        let r = check_contract_deploy(&[0x00], Some("scam"), None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Reject(ref rej) if rej.rule_id.0 == RuleId::PURPOSE_DECLARATION_INVALID));
    }

    #[test]
    fn allow_missing_purpose() {
        let r = check_contract_deploy(&[0x00], None, None, DEFAULT_MAX_BYTECODE_SIZE);
        assert!(matches!(r, QaResult::Allow));
    }

    #[test]
    fn full_check_scam_pattern_reject() {
        // Pattern must be valid bytecode (opcodes or PUSH immediates)
        let pattern = vec![0x00, 0x01, 0x00]; // STOP, ADD, STOP
        let bytecode = vec![0x00, 0x00, 0x01, 0x00, 0x00]; // contains pattern
        let reg = RuleRegistry::new().with_scam_patterns(vec![pattern]);
        let r = check_contract_deploy_full(&bytecode, None, None, &reg);
        assert!(matches!(r, QaResult::Reject(ref rej) if rej.rule_id.0 == RuleId::SCAM_PATTERN_MATCH));
    }

    #[test]
    fn full_check_always_review_unsure() {
        // Use a valid category that is in always-review list
        let reg = RuleRegistry::new().with_always_review_categories(vec!["token".to_string()]);
        let r = check_contract_deploy_full(&[0x00], Some("token"), None, &reg);
        assert!(matches!(r, QaResult::Unsure));
    }

    #[test]
    fn full_check_soft_rule_other_minimal_unsure() {
        let reg = RuleRegistry::new();
        let r = check_contract_deploy_full(&[0x00], Some("other"), Some(&[0u8; 2]), &reg);
        assert!(matches!(r, QaResult::Unsure));
    }

    #[test]
    fn full_check_other_with_description_allow() {
        let reg = RuleRegistry::new();
        let r = check_contract_deploy_full(&[0x00], Some("other"), Some(&[1u8; 8]), &reg);
        assert!(matches!(r, QaResult::Allow));
    }

    #[test]
    fn full_check_metadata_too_long_reject() {
        let reg = RuleRegistry::new();
        let long_name = "a".repeat(MAX_ASSET_NAME_LEN + 1);
        let r = check_contract_deploy_full_with_metadata(
            &[0x00],
            None,
            None,
            Some(&long_name),
            None,
            &reg,
        );
        assert!(matches!(r, QaResult::Reject(ref rej) if rej.rule_id.0 == RuleId::METADATA_TOO_LONG));
    }

    #[test]
    fn full_check_content_policy_reject() {
        let reg = RuleRegistry::new().with_content_blocklist(vec!["forbidden_term".to_string()]);
        let r = check_contract_deploy_full_with_metadata(
            &[0x00],
            Some("token"),
            None,
            Some("My forbidden_term asset"),
            None,
            &reg,
        );
        assert!(matches!(r, QaResult::Reject(ref rej) if rej.rule_id.0 == RuleId::CONTENT_POLICY_VIOLATION));
    }

    #[test]
    fn full_check_content_policy_allow_when_no_match() {
        let reg = RuleRegistry::new().with_content_blocklist(vec!["forbidden".to_string()]);
        let r = check_contract_deploy_full_with_metadata(
            &[0x00],
            Some("token"),
            None,
            Some("Clean Asset Name"),
            Some("SYM"),
            &reg,
        );
        assert!(matches!(r, QaResult::Allow));
    }
}
