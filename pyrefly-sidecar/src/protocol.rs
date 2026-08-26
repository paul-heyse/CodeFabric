//! Model-generated protocol bindings shared with the stable daemon.

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
        pub(crate) mod pyrefly {
            pub(crate) mod v1 {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../src/generated/codefabric.pyrefly.v1.rs"
                ));
            }
        }
    }
}
