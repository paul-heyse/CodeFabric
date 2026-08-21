# Fixture oracle change record

## 2026-08-20 initial oracle classification

- Classified the owner-reviewed JCS, projection, Protobuf wire, and compatibility
  answers as `normative-kat`.
- Classified checksum failures as `negative-class`, fuzz seeds as `property`, and
  compiler-owned descriptor views as `generated-example`.
- Added output-free cross-language `differential` inputs. Candidate KAT generation is
  isolated from this directory and cannot accept changes into normative paths.
