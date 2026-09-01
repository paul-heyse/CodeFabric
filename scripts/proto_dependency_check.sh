#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf 'proto dependency check: %s\n' "$1" >&2
  exit 1
}

manifest=codefabric-cpg-mcp/pyproject.toml
lock=codefabric-cpg-mcp/uv.lock

rg -q '^  "grpcio==1\.83\.0",$' "$manifest" || fail 'grpcio runtime pin drifted'
rg -q '^  "protobuf==7\.36\.0",$' "$manifest" || fail 'protobuf runtime pin drifted'
rg -q '^  "grpcio-tools==1\.83\.0",$' "$manifest" || fail 'grpcio-tools build pin drifted'
rg -q 'name = "grpcio", specifier = "==1\.83\.0"' "$lock" || \
  fail 'locked grpcio intent drifted'
rg -q 'name = "protobuf", specifier = "==7\.36\.0"' "$lock" || \
  fail 'locked protobuf intent drifted'
rg -q 'name = "grpcio-tools", specifier = "==1\.83\.0"' "$lock" || \
  fail 'locked grpcio-tools intent drifted'

if rg -n '\borjson\b' "$manifest" "$lock" codefabric-cpg-mcp/src codefabric-cpg-mcp/tests; then
  fail 'orjson remains in the adapter manifest, lock, runtime, or tests'
fi
if rg -n '(import grpc_tools|from grpc_tools)' codefabric-cpg-mcp/src; then
  fail 'grpcio-tools is imported by adapter runtime code'
fi
if rg -n 'protoc-bin-vendored' Cargo.toml Cargo.lock tooling/proto src tests; then
  fail 'the second Rust protoc toolchain remains active'
fi

printf 'proto dependency check: exact pins and single compiler authority verified\n'
