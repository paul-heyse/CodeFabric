# Protobuf generation authority

The committed `production-descriptor.pb` and released Python stubs are compatibility
inputs at this independent tooling boundary. The Rust generator decodes those exact
descriptor bytes, round-trips them as an integrity check, and passes the resulting
`FileDescriptorSet` to `tonic_prost_build::Builder::compile_fds`. It has no dependency
on the retired repository model compiler.

`descriptor-census.json` is the normalized semantic view used for review and
compatibility checks. `compatibility-baseline.json` is acceptance, not a routine
generator output: changing it accepts wire-history decisions and therefore requires
explicit owner review. Compatible additive schema changes update generated outputs and
the census while leaving the baseline intact. Field and enum removal requires reserving
both the old name and number.

The released descriptor is the shared wire IR consumed by both language boundaries.
Output paths and compatibility allocations remain reviewed release decisions; Protobuf
generation does not become semantic data-fabric authority.

`cargo check --locked --no-default-features --features proto-tooling --bin
codefabric-proto-gen` proves the independent Rust generator still compiles. The
cross-language Protobuf tests prove shared-wire interoperability. Reproducible Protobuf
binary bytes are deterministic evidence only; they are not a canonical serialization
format.
