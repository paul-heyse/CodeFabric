# Protobuf generation authority

`generate.py` invokes the adapter-locked `grpc_tools.protoc` exactly once per
generation root. That invocation emits the Python modules and
`wave0-probe-descriptor.pb`. The Rust generator decodes those same descriptor bytes,
round-trips them as an integrity check, and passes the resulting
`FileDescriptorSet` to `tonic_prost_build::Builder::compile_fds`.

`descriptor-census.json` is the normalized semantic view used for review and
compatibility checks. `compatibility-baseline.json` is deliberately not rewritten by
`proto-gen`: changing it accepts wire-history decisions and therefore requires explicit
review. Compatible additive schema changes update generated outputs and the census while
leaving the baseline intact. Field and enum removal requires reserving both the old name
and number.

`just proto-check` proves checked-in drift, exact package intent, negative compatibility
cases, Python descriptor equivalence, and Rust descriptor decoding. `just
proto-repro-check` repeats the complete generation in two isolated roots and compares
bytes. Reproducible Protobuf binary bytes are deterministic evidence only; they are not a
canonical serialization format.
