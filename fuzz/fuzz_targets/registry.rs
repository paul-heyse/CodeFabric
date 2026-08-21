#![no_main]

use codefabric::contracts::replay_bounded_registry_ingress;
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 64 * 1024;

fuzz_target!(|input: &[u8]| {
    let Some((&selector, source)) = input.split_first() else {
        return;
    };
    if source.len() > MAX_INPUT_BYTES {
        return;
    }
    replay_bounded_registry_ingress(selector, source);
});
