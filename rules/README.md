# Structural governance rules

This directory is the repository-wide `ast-grep scan` rule root. Boundary rules land with
the build domains and generated contracts they govern; `no-pyrefly-public-api` protects the
sidecar's unstable-library seam from the first Pyrefly-linked packet.

The `model-*` rules are initially scoped to the reviewed consumer overlays and the empty
`tooling/model-transition/live/` promotion root: their paired rule tests make the policy
executable during shadow migration without falsely requiring the pre-existing WP32 code to
conform before its dependency-closed WP11 promotion. WP11 expands their file scope to the live
production consumers as part of the atomic cutover.
