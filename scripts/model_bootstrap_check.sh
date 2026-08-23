#!/usr/bin/env bash
# Prove the handwritten model compiler builds without any production generated output.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
task_tmp_base="${TMPDIR:-/tmp}"
task_tmp_base="${task_tmp_base%/}"
sandbox_root="$(mktemp -d "$task_tmp_base/codefabric-model-bootstrap.XXXXXX")"

cleanup() {
  case "$sandbox_root" in
    "$task_tmp_base"/codefabric-model-bootstrap.*)
      rm -rf -- "$sandbox_root"
      ;;
    *)
      printf 'refusing to remove unexpected bootstrap path: %s\n' "$sandbox_root" >&2
      ;;
  esac
}
trap cleanup EXIT

fail() {
  printf 'model bootstrap check failed: %s\n' "$1" >&2
  exit 1
}

copy_current_tree() {
  while IFS= read -r -d '' path; do
    [ -e "$repo_root/$path" ] || [ -L "$repo_root/$path" ] || continue
    mkdir -p "$sandbox_root/$(dirname "$path")"
    cp -Pp "$repo_root/$path" "$sandbox_root/$path"
  done < <(git -C "$repo_root" ls-files -co --exclude-standard -z)
}

omit_generated_outputs() {
  local plan
  local output
  plan="$("$repo_root/scripts/model_exec.sh" plan "$repo_root")"
  jq -e '.output_count == (.output_paths | length)' <<<"$plan" >/dev/null \
    || fail 'model plan did not provide its exact output census'
  while IFS= read -r output; do
    case "$output" in
      ''|/*|../*|*/../*|*/..)
        fail "catalog output is not a safe repository-relative path: $output"
        ;;
    esac
    rm -f -- "$sandbox_root/$output"
  done < <(jq -r '.output_paths[]' <<<"$plan")
}

model_bootstrap_has_no_generated_or_production_library_edge() {
  local source_hits tree
  source_hits="$(rg -n 'include(_bytes)?!\s*\(|\bcodefabric::|extern\s+crate\s+codefabric|#\s*\[\s*path\s*=.*generated|\b(use|mod)\s+(crate::)?generated\b' \
    "$sandbox_root/src/bin/codefabric_model" || true)"
  [ -z "$source_hits" ] || fail "model binary source reaches a generated or production library surface: $source_hits"

  tree="$(cd "$sandbox_root" && cargo tree --locked --edges normal \
    --no-default-features --features model-compiler --prefix none)"
  if printf '%s\n' "$tree" | rg -q \
    '^(arrow($|-)|parquet |datafusion($|-)|deltalake($|-)|object_store |pyo3($|-)|rusqlite |tonic($|-)|prost |rayon |tree-sitter($|-)|ruff_python_|ruff_source_file |ruff_text_size )'; then
    fail 'model-compiler dependency graph contains a production, runtime, or provider family'
  fi
}

model_bootstrap_builds_without_generated_outputs() {
  local rustc_identity lock_identity source_identity target_triple build_key target_dir executable
  rustc_identity="$(rustc -vV)"
  lock_identity="$(shasum -a 256 "$repo_root/Cargo.lock" | awk '{print $1}')"
  source_identity="$(
    {
      shasum -a 256 "$repo_root/Cargo.toml"
      find "$repo_root/src/bin/codefabric_model" -type f -name '*.rs' -print \
        | LC_ALL=C sort \
        | while IFS= read -r source; do
            shasum -a 256 "$source"
          done
    } | shasum -a 256 | awk '{print $1}'
  )"
  target_triple="$(printf '%s\n' "$rustc_identity" | sed -n 's/^host: //p')"
  build_key="$(printf '%s\n' "$rustc_identity" "$lock_identity" "$source_identity" \
    'features=model-compiler' 'profile=dev' "target=$target_triple" | shasum -a 256 | awk '{print $1}')"
  target_dir="$repo_root/target/model-builds/$build_key"

  (
    cd "$sandbox_root"
    CARGO_TARGET_DIR="$target_dir" cargo test --locked --no-default-features \
      --features model-compiler --bin codefabric-model model_control::tests
    CARGO_TARGET_DIR="$target_dir" cargo build --locked --no-default-features \
      --features model-compiler --bin codefabric-model
  )

  executable="$target_dir/debug/codefabric-model"
  [ -x "$executable" ] || fail "model compiler executable is absent: $executable"
  "$executable" --identity | jq -e '
    .generator_id == "codefabric-model"
    and .generator_revision == "model-compiler-v1"
  ' >/dev/null || fail 'model compiler identity is invalid'
}

cd "$repo_root"
copy_current_tree
omit_generated_outputs
model_bootstrap_has_no_generated_or_production_library_edge
model_bootstrap_builds_without_generated_outputs

printf 'model bootstrap check passed\n'
