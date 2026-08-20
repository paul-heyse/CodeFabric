#![no_main]

use codefabric::contracts::jcs::canonicalize_slice;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|source: &[u8]| {
    if let Ok(canonical) = canonicalize_slice(source) {
        let replay = canonicalize_slice(&canonical).expect("canonical output must remain valid");
        assert_eq!(canonical, replay, "canonicalization must be idempotent");
    }
});
