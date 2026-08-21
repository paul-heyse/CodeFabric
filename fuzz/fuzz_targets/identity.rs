#![no_main]

use codefabric::identity::{
    CaseSensitivityMode, IdentityDomain, PlatformCode, WorkspacePath, decode_public_id,
    decode_record, encode_record,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    match data[0] % 3 {
        0 => {
            if let Ok(record) = decode_record(&data[1..]) {
                assert_eq!(encode_record(&record).as_deref(), Ok(&data[1..]));
            }
        }
        1 => {
            let components = data[1..]
                .split(|byte| *byte == 0)
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>();
            if let Ok(path) = WorkspacePath::from_components(
                [0; 16],
                if data[0] & 0x08 == 0 {
                    PlatformCode::Unix
                } else {
                    PlatformCode::MacOs
                },
                if data[0] & 0x10 == 0 {
                    CaseSensitivityMode::Sensitive
                } else {
                    CaseSensitivityMode::Insensitive
                },
                &components,
            ) {
                assert_eq!(
                    path.decoded_components().as_deref(),
                    Ok(components.as_slice())
                );
                assert_eq!(path.ordering_key().1, path.raw_relative_path_bytes);
                assert!(!path.canonical_uri().contains('='));
            }
        }
        _ => {
            if let Ok(value) = std::str::from_utf8(&data[1..]) {
                let _ = decode_public_id(IdentityDomain::Workspace, None, value);
                let _ = decode_public_id(IdentityDomain::Entity, Some("callable"), value);
                let _ = decode_public_id(IdentityDomain::AnalysisContext, None, value);
            }
        }
    }
});
