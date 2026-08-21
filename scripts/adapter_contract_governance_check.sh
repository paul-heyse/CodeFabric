#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for retired in \
  contracts/adapter/fastmcp-input.schema.json \
  contracts/adapter/fastmcp-output.schema.json \
  contracts/adapter/fastmcp-public-meta.schema.json \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/_contract_index.py; do
  if [[ -e "$repository_root/$retired" ]]; then
    echo "retired independent adapter authority remains: $retired" >&2
    exit 1
  fi
done

if rg -n 'TypeAdapter\(|create_model\(|model_json_schema\(' \
  "$repository_root/codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py" \
  "$repository_root/codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon"; then
  echo "dynamic model/schema construction reached a handler or daemon hot path" >&2
  exit 1
fi

if rg -n 'orjson|SerializeAsAny|model_construct\(|experimental_allow_partial|Provider\(' \
  "$repository_root/codefabric-cpg-mcp/src" \
  -g '*.py'; then
  echo "prohibited adapter contract bypass is present" >&2
  exit 1
fi

output_count="$(jq '[.derivations[] |
  select(.derivation_id == "codefabric.derivation.adapter-models") |
  .outputs[]] | length' \
  "$repository_root/contracts/manifests/suite-manifest.json")"
if [[ "$output_count" != "3" ]]; then
  echo "adapter model compiler must own exactly three catalog outputs" >&2
  exit 1
fi

echo "adapter model/schema authority and hot-path zero-state passed"
