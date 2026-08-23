# Structural governance rules

This directory is the repository-wide `ast-grep scan` rule root. Boundary rules land with
the build domains and generated contracts they govern; `no-pyrefly-public-api` protects the
sidecar's unstable-library seam from the first Pyrefly-linked packet.

The `model-*` rules apply directly to the promoted model compiler and production consumers.
Generated sources are excluded by the scan recipe; their authorities and renderer tests prove
their allocations. No temporary transition root is part of the live policy.
