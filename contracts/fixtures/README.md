# Shared cross-language fixtures

There is deliberately no generated global fixture census. Retained normative fixtures are named
by their owning executable contract; the independent v3 claim and negative-fixture transaction
lives under `contracts/acceptance/relational-fabric-v3/`.

Normative KATs are immutable owner-reviewed protocol authority. Generators and gates
must never write them. Candidate answers may be emitted only to an isolated review
directory and need a versioned entry in `CHANGELOG.md` plus owner review before a
human edits a normative corpus. Differential/property cases deliberately store no
expected digests. Tests copy stateful inputs to a per-test temporary directory.
