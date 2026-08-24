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
        pub(crate) mod rustc {
            pub(crate) mod v1 {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../src/generated/codefabric.rustc.v1.rs"
                ));
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) mod observation_schema {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/generated/model_schema_tables.rs"
        ));
    }

    #[allow(dead_code)]
    pub(crate) mod registries {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/generated/registries.rs"
        ));
    }
}
