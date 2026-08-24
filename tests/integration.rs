//! The repository's single external integration-test target.

mod integration {
    #[cfg(feature = "compatibility-probes")]
    mod compatibility;
    #[cfg(feature = "daemon")]
    mod coordinator;
    #[cfg(feature = "daemon")]
    mod daemon;
    #[cfg(feature = "daemon")]
    mod data_fabric_upgrade;
    #[cfg(feature = "fact-generation")]
    mod fact_generation_build;
    #[cfg(feature = "daemon")]
    mod git_state;
    #[cfg(feature = "rpc")]
    mod rpc;
}
