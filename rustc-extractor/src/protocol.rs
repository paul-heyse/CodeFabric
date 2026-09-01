//! Released protocol bindings shared with the stable daemon.

#[allow(clippy::all, clippy::pedantic)]
pub(crate) mod generated {
    pub(crate) mod codefabric {
        pub(crate) mod provider {
            pub(crate) mod v1 {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../src/generated/codefabric.provider.v1.rs"
                ));
            }
        }
        pub(crate) mod rustc {
            pub(crate) mod v1 {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../src/generated/codefabric.rustc.v1.rs"
                ));
            }
        }
    }
}

/// Released append-only `RUST_MIR` capability allocation on the provider-control wire.
///
/// This is a compatibility constant, not a generated semantic registry or runtime authority.
pub(crate) const RUST_MIR_CAPABILITY_CODE: u32 = 120;
