# Local native dependency artifacts

This directory defines packaging and verification; it does not install an artifact.
Before any native source is installed or selected, an absent `manifest.json` preserves
the existing dependency baseline. Installing
an artifact does not change Cargo dependencies: the accepted pin amendment and the
first-party manifests must select its exact paths and versions separately.

An artifact contains the amended source trees, original upstream archives, and ordered
patches under `third_party/native/<sha256>/`. Its installed manifest records a complete
census of files, directories, executable bits, and internal symlink targets. The digest
is SHA-256 of canonical JSON (sorted keys, compact separators, UTF-8, final newline)
containing `schema_version`, `sources`, and `files`; it excludes its own derived
`artifact_sha256` and `artifact_path` fields. No staging-machine paths survive.

Use exact-version Cargo `[patch]` entries pointing inside that immutable directory.
Cargo.lock does not checksum path packages. The accepted artifact identity, complete
census verifier, and resolved-package path checks are therefore all required. Git
tracks the source and provenance files; nothing is written into Cargo's source cache.

After an artifact is selected, check actual Cargo resolution as well as its census:

```sh
cargo metadata --locked --format-version 1 --all-features > /tmp/codefabric-metadata.json
python3 tooling/ci/native_dependency_artifacts.py verify-resolution \
  --root "$PWD" --metadata /tmp/codefabric-metadata.json
```

The metadata must include the complete resolve graph (omit `--no-deps`). The
default requires every declared native package. For a deliberately narrow feature
graph, `--allow-subset` permits absent packages while checking every native package
that is present. Registry fallback, same-version mutable copies, duplicate native
packages, undeclared artifact packages and incomplete graphs fail closed. `--metadata -`
accepts Cargo JSON on standard input. Generate metadata again after relocating a
checkout; the source identity stays portable while resolved paths are checkout-local.

## Package a reviewed amendment

Prepare an input specification conforming to `package-spec.schema.json`. Paths in that
specification may be absolute or relative to the specification's directory. This example
uses one registry package; `native-resource-recipe.json` contains the exact native-resource amendment inputs, including the full Delta Git export:

```json
{
  "schema_version": 1,
  "sources": [
    {
      "name": "buoyant_kernel",
      "upstream": {
        "kind": "registry",
        "archive": "archives/buoyant_kernel-0.25.1.crate",
        "url": "https://github.com/rust-lang/crates.io-index",
        "name": "buoyant_kernel",
        "version": "0.25.1",
        "sha256": "1613e308b8de3a103bf6d87f60dc5218ac9b220e4e8974962416c8a28437e849"
      },
      "staged_path": "staged/buoyant_kernel",
      "patches": [
        "patches/buoyant_kernel/0001-native-resource.patch"
      ],
      "packages": [
        {
          "name": "buoyant_kernel",
          "version": "0.25.1",
          "manifest": "Cargo.toml",
          "license": "Apache-2.0",
          "license_files": [
            {
              "path": "LICENSE",
              "role": "license",
              "sha256": "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
              "origin": "upstream"
            },
            {
              "path": "NOTICE",
              "role": "notice",
              "sha256": "5939690f74b26ce550e4c6d40f2944a0103760c8ecc76efd3a7521f779e19a11",
              "origin": "upstream"
            }
          ]
        }
      ]
    }
  ]
}
```

The Git input is read with `git archive --format=tar <exact-commit>`, not from its
working tree. This follows upstream archive attributes, including `export-ignore`;
prepare the amended export from the same command. Keep upstream workspace manifests,
licenses, and internal package paths. For registry inputs, use the original `.crate`
archive and the already approved registry checksum. A Cargo unpack directory can carry
cache-only files such as `.cargo-ok`; those are not part of that archive or the staged
amendment. Preserve the upstream package manifest rather than substituting its `.orig`
version without an explicit patch.

The staged tree must equal upstream archive extraction followed by each ordered patch.
Use ordinary Git patches with `a/` and `b/` paths, including `--binary` where required.
The packager applies them only in temporary directories. It rejects undeclared edits,
wrong package/license identities, archive traversal, hardlinks, special files, and symlinks
outside a source tree. Explicit Cargo paths in every source manifest, including optional
target/workspace dependencies, must stay inside that immutable source tree. Internal
relative license symlinks are supported. Source files
are never silently reformatted or rewritten.

```bash
python3 tooling/ci/native_dependency_artifacts.py package --root . --spec /staging/native-spec.json
python3 tooling/ci/native_dependency_artifacts.py verify --root . --replay
```

Packaging publishes the complete artifact directory before atomically installing the
manifest. Repeating identical input is idempotent. Changing an existing manifest requires
the explicit `--replace-manifest` argument; the previous artifact remains intact. Changing
artifact bytes in place is always rejected. A changed artifact must receive a new digest
and the applicable approved pin/plan transition before becoming executable authority.

## Verification and assurance

`scripts/cargo` verifies installed manifest and artifact bytes before invoking rustup for
every Cargo command, including metadata and formatting. That check requires only Python's
standard library and reads every censused entry. It also checks package identities,
upstream archive checksums and Git commit markers, patch checksums/order, and descriptor
identity. A symlinked manifest is rejected. Deleting the manifest while an artifact exists
or a first-party manifest selects a native path fails before Cargo runs.

`just native-dependency-artifacts-check` additionally extracts upstream archives and replays
all amendments, proving that they reproduce each installed source tree. This gate requires
an installed artifact; the pre-amendment baseline cannot certify native closure. The ordinary
router check validates the complete bytes previously proved by replay; it does not rerun
Git patch application on every compiler discovery call. Neither check authenticates a
claimed upstream URL: the approved revision/checksum and reviewed manifest are the trust
anchors. Neither substitutes for native behavioral, cancellation, resource, or compatibility
tests.

Root, extractor, and sidecar formatting commands select their exact first-party package.
`cargo fmt --all` includes local path dependencies and would mutate immutable third-party
sources. General spelling/governance scans exclude only `third_party/native/`; the verifier
and separately selected native amendment checks own that subtree. Upstream workspaces remain
third-party source artifacts, never members of a new first-party workspace or build domain.

## Exact native-resource recipe

`native-resource-recipe.json` is a portable preparation input, not an installed
manifest or authority declaration. It pins eleven source archives and fourteen
selected Cargo package identities: Delta at `43a0cf10a313e5077c48637ad786a05359136bbb`,
kernel 0.25.1, engine 0.25.0, derive 1.1.0, and JSON/Parquet/Arrow buffer/schema/data/array/select
59.2.0. Every registry checksum was checked against the committed stable-root lock
and the original `.crate` bytes. Its paths are relative to a separate review directory.
DataFusion remains 55.0.0; no new first-party build domain is introduced.

Freeze reviewed native source before using this recipe. Each `staged/<source>` must
be a complete ordinary source export: no `.git`, Cargo cache marker, build target,
private probe manifest, or machine-local lockfile changes. Preserve the original
registry `Cargo.toml`, `Cargo.toml.orig`, lockfile, licenses, and all unchanged files.
For Delta, both the pristine export and amended export start from the exact
`git archive --format=tar <revision>` output, including upstream archive attributes.
A temporary source-export directory may be edited; a Cargo cache or installed
artifact may not.

Prepare a directory with these four children:

- `archives/`: original `.crate` archives under the exact filenames in the recipe.
- `upstream/delta-rs.git`: a local Git repository containing the exact Delta commit.
- `staged/`: the complete final reviewed exports, one directory per source.
- `patches/`: ordered source amendments, using the exact paths listed in the recipe.

`0001-native-resource.patch` denotes the final consolidated native amendment from
the exact upstream archive, including every contributor's reviewed changes. It is
not a diff against an intermediate agent baseline. A reviewed ordered patch series
may replace that entry; the packager verifies every patch in the recorded order.
To generate a consolidated Git patch without changing either export, use a private
bare Git index seeded with the pristine export (set the three paths explicitly):

```bash
task_git=$(mktemp -d)
git init --bare "$task_git/index.git"
git --git-dir="$task_git/index.git" --work-tree="$pristine_export" add -f -A
task_tree=$(git --git-dir="$task_git/index.git" write-tree)
git --git-dir="$task_git/index.git" --work-tree="$reviewed_export" add -f -A
git --git-dir="$task_git/index.git" diff --cached --binary --no-ext-diff "$task_tree" > "$reviewed_patch"
```

Review the resulting patch and preserve its source proof. Packaging independently
re-extracts the original archive, applies the complete series, and compares the full
census against the reviewed export. Thus ignored files, newly added sources, modes,
and internal symlinks cannot silently disappear from the native amendment.

The exact engine `.crate` omits license and notice files. Its `.cargo_vcs_info.json`
pins `bf39205e2ef6c20a66a3e5659886a44a9855e16d`. The separately supplied
`patches/buoyant_kernel_engine/0002-license-and-notice.patch` adds the complete
[LICENSE](https://raw.githubusercontent.com/delta-io/delta-kernel-rs/bf39205e2ef6c20a66a3e5659886a44a9855e16d/LICENSE)
and [NOTICE](https://raw.githubusercontent.com/delta-io/delta-kernel-rs/bf39205e2ef6c20a66a3e5659886a44a9855e16d/NOTICE)
from that exact upstream commit. Apply this patch to the reviewed engine export
**after** generating its functional `0001` amendment, and copy it to the same relative
path in the review directory. It has not been applied to any live native stage by
this recipe. Arrow-array declares `Apache-2.0 AND MIT`; retain both full license files.

Copy the recipe into the review directory and run:

```bash
python3 tooling/ci/native_dependency_artifacts.py package --root "$review_repository" --spec "$review_directory/native-resource-recipe.json"
python3 tooling/ci/native_dependency_artifacts.py verify --root "$review_repository" --replay
```

Use a separate review repository until the complete native sources are frozen and
tested. Installation into the real root and exact Cargo `[patch]` selection remain
a synchronized integration step. The recipe does not perform that step, update
Cargo locks, rewrite a current suite/plan, or claim native behavioral completion.

Every package now declares its exact Cargo license expression plus full-file
`path`, `role`, `sha256`, and `origin` records. `origin: "upstream"` must match the
exact original archive during replay; a supplement records a reviewed HTTPS origin
and is reproduced by an ordered patch. Each source's LICENSE/NOTICE/COPYING file
census must be fully declared across its package records, including notices from
unselected packages retained in a full upstream workspace export. The checks prove
identity and byte preservation, not an independent assessment of license terms.
