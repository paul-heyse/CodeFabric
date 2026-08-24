#!/usr/bin/env bash
# Build and execute the model compiler from an identity-isolated Cargo target root.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

compiler_source_identity() {
  (
    cd "$repo_root"
    {
      shasum -a 256 Cargo.toml | awk '{print $1 "  Cargo.toml"}'
      shasum -a 256 scripts/model_exec.sh \
        | awk '{print $1 "  scripts/model_exec.sh"}'
      find src/bin/codefabric_model -type f -name '*.rs' -print \
        | LC_ALL=C sort \
        | while IFS= read -r source; do
            shasum -a 256 "$source" | awk -v path="$source" '{print $1 "  " path}'
          done
    } | shasum -a 256 | awk '{print $1}'
  )
}

rustc_identity="$(rustc -vV)"
lock_identity="$(shasum -a 256 "$repo_root/Cargo.lock" | awk '{print $1}')"
source_identity="$(compiler_source_identity)"
target_triple="$(printf '%s\n' "$rustc_identity" | sed -n 's/^host: //p')"
build_key="$(
  printf '%s\n' "$rustc_identity" "$lock_identity" "$source_identity" \
    'features=model-compiler' 'profile=dev' "target=$target_triple" \
    | shasum -a 256 | awk '{print $1}'
)"
target_dir="$repo_root/target/model-builds/$build_key"
executable="$target_dir/debug/codefabric-model"
encoded_rustflags="--remap-path-prefix=$repo_root=/codefabric"

CARGO_INCREMENTAL=0 CARGO_ENCODED_RUSTFLAGS="$encoded_rustflags" \
  CARGO_TARGET_DIR="$target_dir" cargo build --quiet --locked --no-default-features \
  --features model-compiler --bin codefabric-model --manifest-path "$repo_root/Cargo.toml"

[ -x "$executable" ] || {
  printf 'model executable is absent after build: %s\n' "$executable" >&2
  exit 1
}
export CODEFABRIC_MODEL_COMPILER_SOURCE_IDENTITY="sha256:$source_identity"
export CODEFABRIC_MODEL_CARGO_LOCK_IDENTITY="sha256:$lock_identity"
export CODEFABRIC_MODEL_RUSTC_IDENTITY="$rustc_identity"
export CODEFABRIC_MODEL_FEATURE_SET="model-compiler"
export CODEFABRIC_MODEL_PROFILE="dev"
export CODEFABRIC_MODEL_TARGET_TRIPLE="$target_triple"
export CODEFABRIC_MODEL_CACHE_MODE="${CODEFABRIC_MODEL_CACHE_MODE:-read-write}"
exec "$executable" "$@"
