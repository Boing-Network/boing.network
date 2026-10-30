#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let (pat_len, rest) = data.split_first().unwrap();
    let pat_len = (*pat_len as usize).min(64).min(rest.len());
    let (pattern, bytecode) = rest.split_at(pat_len);
    if pattern.is_empty() {
        return;
    }
    let mut reg = boing_qa::RuleRegistry::new().with_scam_patterns(vec![pattern.to_vec()]);
    let _ = reg.validate();
    let _ = boing_qa::check_contract_deploy_full(bytecode, Some("dapp"), None, &reg);
});
