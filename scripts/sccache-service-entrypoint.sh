#!/usr/bin/env bash
# Remove only the exact stale UDS endpoint, then become the supervised sccache server.

set -euo pipefail
umask 077

[ "$#" -eq 2 ] || { printf 'usage: %s <sccache-binary> <socket-path>\n' "$0" >&2; exit 64; }
sccache_bin="$1"
socket_file="$2"
socket_dir="$(dirname "$socket_file")"

# The supervisor and entrypoint must name one identical endpoint. A stale unit once
# passed /tmp here while exporting an XDG-runtime socket, which left an apparently
# healthy service that sandboxed compiler requests could not reach.
if [ "${SCCACHE_SERVER_UDS:-}" != "$socket_file" ]; then
  printf 'sccache service socket contract mismatch: argument=%s environment=%s\n' \
    "$socket_file" "${SCCACHE_SERVER_UDS:-missing}" >&2
  exit 1
fi

[ ! -L "$socket_dir" ] || {
  printf 'refusing symlinked sccache socket directory: %s\n' "$socket_dir" >&2
  exit 1
}
mkdir -p "$socket_dir"
chmod 700 "$socket_dir"

if [ -S "$socket_file" ]; then
  unlink "$socket_file"
elif [ -e "$socket_file" ]; then
  printf 'refusing to replace non-socket sccache endpoint: %s\n' "$socket_file" >&2
  exit 1
fi

exec "$sccache_bin"
