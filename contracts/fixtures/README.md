# Shared cross-language fixtures

`contracts/manifests/fixture-oracles.json` classifies every data fixture as
`normative-kat`, `differential`, `property`, `negative-class`, or
`generated-example` and records its origin, owner, version, and change record.

Normative KATs are immutable owner-reviewed protocol authority. Generators and gates
must never write them. Candidate answers may be emitted only to an isolated review
directory and need a versioned entry in `CHANGELOG.md` plus owner review before a
human edits a normative corpus. Differential/property cases deliberately store no
expected digests. Tests copy stateful inputs to a per-test temporary directory.
