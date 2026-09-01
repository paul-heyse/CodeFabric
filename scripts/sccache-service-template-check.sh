#!/usr/bin/env bash
# Prove the generated Linux user unit has secure lifecycle semantics without installing it.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
scratch_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-sccache-contract.XXXXXX")"
trap 'rm -rf -- "$scratch_root"' EXIT HUP INT TERM
unit="$($repo_root/scripts/sccache-service.sh render-systemd)"

printf '%s\n' "$unit" | grep -Fxq 'UMask=0077'
printf '%s\n' "$unit" | grep -Fq 'sccache-service-entrypoint.sh'
printf '%s\n' "$unit" | grep -Fxq 'Restart=on-failure'
printf '%s\n' "$unit" | grep -Fxq 'WantedBy=default.target'
socket="/tmp/codefabric-sccache/server.sock"
printf '%s\n' "$unit" | grep -Fxq "Environment=\"SCCACHE_SERVER_UDS=$socket\""
printf '%s\n' "$unit" | grep -Eq "^ExecStart=.*sccache-service-entrypoint\.sh .*sccache $socket$"
if printf '%s\n' "$unit" | grep -Eq '^After=default\.target$|^RuntimeDirectory='; then
  printf 'sccache user unit reintroduced runtime-directory endpoint drift\n' >&2
  exit 1
fi

if SCCACHE_SERVER_UDS=/run/user/999/codefabric/sccache/server.sock \
  "$repo_root/scripts/sccache-service-entrypoint.sh" /bin/true "$socket" \
  >/dev/null 2>&1; then
  printf 'sccache entrypoint accepted divergent argument and environment sockets\n' >&2
  exit 1
fi

fake_rustc="$scratch_root/rustc"
fake_sccache="$scratch_root/sccache"
compiler_log="$scratch_root/compiler.log"
sccache_log="$scratch_root/sccache.log"

{
  printf '%s\n' '#!/usr/bin/env bash'
  printf '%s\n' 'printf '\''%s\n'\'' "$*" >>"$CODEFABRIC_FAKE_COMPILER_LOG"'
} >"$fake_rustc"
{
  printf '%s\n' '#!/usr/bin/env bash'
  printf '%s\n' 'printf '\''%s\n'\'' "$*" >>"$CODEFABRIC_FAKE_SCCACHE_LOG"'
} >"$fake_sccache"
chmod 700 "$fake_rustc" "$fake_sccache"

# Cargo compiler-discovery queries must bypass both the cache binary and its service
# state. A failing SCCACHE_PATH makes an accidental cache invocation immediately visible.
for query in -vV -V --version --print --print=cfg; do
  CODEFABRIC_FAKE_COMPILER_LOG="$compiler_log" \
    CI=true SCCACHE_PATH=/bin/false \
    "$repo_root/scripts/sccache-wrapper.sh" "$fake_rustc" "$query"
done
[ "$(wc -l <"$compiler_log")" -eq 5 ]

# A compile-producing invocation must still traverse sccache.
CODEFABRIC_FAKE_SCCACHE_LOG="$sccache_log" \
  CI=true SCCACHE_PATH="$fake_sccache" \
  "$repo_root/scripts/sccache-wrapper.sh" "$fake_rustc" \
  --crate-name codefabric_sccache_contract input.rs --crate-type rlib --emit=link
grep -Fq -- '--crate-name codefabric_sccache_contract' "$sccache_log"

healthy_probe_cache="$scratch_root/healthy-rustc-info.json"
poisoned_probe_cache="$scratch_root/poisoned-rustc-info.json"
printf '%s\n' \
  '{"outputs":{"1":{"success":true,"status":"","code":0,"stdout":"rustc 1.98.0","stderr":""}}}' \
  >"$healthy_probe_cache"
printf '%s\n' \
  '{"outputs":{"1":{"success":false,"status":"exit status: 1","code":1,"stdout":"","stderr":"CodeFabric: mandatory sccache service is unavailable"}}}' \
  >"$poisoned_probe_cache"
"$repo_root/scripts/sccache-service.sh" cargo-probe-cache-check "$healthy_probe_cache"
if "$repo_root/scripts/sccache-service.sh" cargo-probe-cache-check "$poisoned_probe_cache" \
  >"$scratch_root/poison.stdout" 2>"$scratch_root/poison.stderr"; then
  printf 'sccache Cargo probe-cache check accepted a cached wrapper failure\n' >&2
  exit 1
fi
grep -Fq "rm -- $poisoned_probe_cache" "$scratch_root/poison.stderr"
grep -Fq 'Do not run `cargo clean`' "$scratch_root/poison.stderr"

printf 'sccache service, wrapper-query, and Cargo probe-cache contracts passed\n'
