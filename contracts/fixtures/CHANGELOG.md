# Fixture oracle change record

## 2026-08-21 schema contract generation

- Added a committed mutation oracle proving a schema fingerprint change without an
  artifact-version advance is rejected as `SCHEMA_VERSION_NOT_ADVANCED`.

## 2026-08-21 controlled language and model packs

- Added owner-reviewed positive and executable-field-negative cases for the closed,
  declarative AC-G-38 model-pack schema.
- Added an output-free typed phrase-registry seed for bounded parser fuzzing.

## 2026-08-21 Wave 1 registry KATs

- Added independent code/name/slug examples for every categorical domain and fixed
  64-bit flag-word composition answers consumed by both generated language views.

## 2026-08-20 full fuzz corpus governance

- Extended fixture census governance from one fuzz target directory to every committed
  `fuzz/corpus/` seed and classified the pre-existing JCS seeds as output-free property
  inputs.

## 2026-08-20 Wave 1 identity KATs

- Added independently calculated CBEF-v1 known answers for all 16 domain recipes
  and all 13 core type codes.
- Added reversible path, macOS NFD/full-casefold collision, and canonical URI
  vectors plus canonical type-algebra term and type-ID vectors.
- Classified the bounded identity replay seed as a property corpus; production
  generators remain unable to write any normative KAT path.

## 2026-08-20 initial oracle classification

- Classified the owner-reviewed JCS, projection, Protobuf wire, and compatibility
  answers as `normative-kat`.
- Classified checksum failures as `negative-class`, fuzz seeds as `property`, and
  compiler-owned descriptor views as `generated-example`.
- Added output-free cross-language `differential` inputs. Candidate KAT generation is
  isolated from this directory and cannot accept changes into normative paths.
