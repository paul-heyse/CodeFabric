# Protobuf generation authority

The model Proto family driver discovers the governed `.proto` sources and invokes the
adapter-locked `grpc_tools.protoc` exactly once per isolated staging root. That
invocation emits the Python modules and `production-descriptor.pb`. The isolated Rust
driver decodes those same descriptor bytes, round-trips them as an integrity check, and
passes the resulting `FileDescriptorSet` to
`tonic_prost_build::Builder::compile_fds`.

`descriptor-census.json` is the normalized semantic view used for review and
compatibility checks. `compatibility-baseline.json` is acceptance, not a routine
`model-sync` output: changing it accepts wire-history decisions and therefore requires
explicit owner review. Compatible additive schema changes update generated outputs and
the census while leaving the baseline intact. Field and enum removal requires reserving
both the old name and number.

The governed source files are one model-derived compilation unit and generate Python
module triples plus Rust package files. Output paths, roles, consumers, and provenance
come from typed declarations and source/package discovery; there is no per-package
compiler invocation or filename switch.

`just model-family-check proto` proves checked-in drift, exact package intent, negative
compatibility cases, Python descriptor equivalence, Rust descriptor decoding, and staged
consumers. `just model-repro-check` repeats the complete DesiredTree in isolated roots
and compares paths and bytes. Reproducible Protobuf binary bytes are deterministic
evidence only; they are not a canonical serialization format.
