# Native dependency development

Native sources are ordinary editable Git-tracked files in `third_party/native/`.
The application and native tests select the same sources through relative Cargo paths.
Source changes use normal commits, review and tests. No per-edit source artifact,
patch replay, or pre-Cargo manifest is required. Runtime execution artifacts remain
part of the product contract.

The source README records upstream origins and the frozen staging locations used for
this import. Licenses and notices remain alongside upstream source. Cargo.lock and
resolved-source checks preserve one selected package/type universe.

The former per-edit artifact packaging/replay utility and process manifests were retired on 2026-09-08. Native origins and patches remain recorded with the sources and in Git.

Native assurance harnesses under `tests/` retain distinct allocator-observation test
binaries. They are test tooling, not an additional production build domain. The joined
harness imports the application's actual source files; it must not grow private copies
of application logic. All harnesses use the native source directories above.
