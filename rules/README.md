# Structural governance rules

This directory is the repository-wide `ast-grep scan` rule root. Boundary rules land with
the build domains and generated contracts they govern; `no-pyrefly-public-api` protects the
sidecar's unstable-library seam from the first Pyrefly-linked packet.

The `model-*` rules apply directly to the promoted model compiler and production consumers.
Generated sources are excluded by the scan recipe; their authorities and renderer tests prove
their allocations. No temporary transition root is part of the live policy.

`governed-datafusion-ingress-only` begins DB08 with an explicit list of pre-existing migration
surfaces. WP26 removes those exemptions as the candidate and serving paths converge on
`GovernedSession`. `domain-conformance-exhaustive` keeps the pinned DataFusion expression and
logical-plan enums compile-time exhaustive by rejecting accepting wildcard arms.
