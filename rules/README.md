# Structural governance rules

This directory is the repository-wide `ast-grep scan` rule root. Boundary rules land with
the build domains and generated contracts they govern; `no-pyrefly-public-api` protects the
sidecar's unstable-library seam from the first Pyrefly-linked packet.

Only rules that protect a live build, process, wire, provider, storage, or security boundary belong
here. Generated sources are excluded by the scan recipe and are validated by their owning contract
or interoperability checks. Superseded model/ontology authority rules are deleted with the code
they governed; a structural rule must not keep a retired subsystem conceptually live.

Delta construction is intentionally distributed across the successor fabric's typed state owners.
`deltalake-boundary-only` keeps those APIs inside `src/fabric/**`; there is no synthetic single-file
handle factory after the relational cutover. Provider-native syntax relations and raw-kind policy are
part of the Tree-sitter/Ruff adapter boundary, but only application-owned records leave that seam.
