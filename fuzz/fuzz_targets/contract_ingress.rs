#![no_main]

use codefabric::contracts::catalog::NativeFormat;
use codefabric::contracts::compiler::replay_bounded_ingress;
use libfuzzer_sys::fuzz_target;

const MAX_FUZZ_INPUT_BYTES: usize = 64 * 1024;

fuzz_target!(|input: &[u8]| {
    let Some((&selector, source)) = input.split_first() else {
        return;
    };
    let format = match selector % 6 {
        0 => NativeFormat::Markdown,
        1 => NativeFormat::Json,
        2 => NativeFormat::Jsonl,
        3 => NativeFormat::Yaml,
        4 => NativeFormat::Proto,
        _ => NativeFormat::Ebnf,
    };
    let _ = replay_bounded_ingress(format, source, MAX_FUZZ_INPUT_BYTES);
});
