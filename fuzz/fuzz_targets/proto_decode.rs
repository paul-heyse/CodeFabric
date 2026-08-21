#![no_main]

use codefabric::rpc::generated::codefabric::cpgd::v1::StartQueryRequest;
use codefabric::rpc::generated::codefabric::provider::v1::ProviderJobSpec;
use codefabric::rpc::generated::codefabric::pyrefly::v1::Hello;
use codefabric::rpc::generated::codefabric::rustc::v1::CompilationAccepted;
use libfuzzer_sys::fuzz_target;
use prost::Message;

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn input_bytes(input: &[u8]) -> Option<Vec<u8>> {
    let Some(hex) = input.strip_prefix(b"hex:") else {
        return Some(input.to_vec());
    };
    if hex.len() % 2 != 0 {
        return None;
    }
    hex.chunks_exact(2)
        .map(|pair| Some((hex_value(pair[0])? << 4) | hex_value(pair[1])?))
        .collect()
}

fn decode_and_reencode<M: Message + Default>(bytes: &[u8]) {
    if let Ok(message) = M::decode(bytes) {
        let encoded = message.encode_to_vec();
        let _ = M::decode(encoded.as_slice()).expect("prost re-encoding must decode");
    }
}

fuzz_target!(|input: &[u8]| {
    let Some(bytes) = input_bytes(input) else {
        return;
    };
    decode_and_reencode::<StartQueryRequest>(&bytes);
    decode_and_reencode::<ProviderJobSpec>(&bytes);
    decode_and_reencode::<Hello>(&bytes);
    decode_and_reencode::<CompilationAccepted>(&bytes);
});
