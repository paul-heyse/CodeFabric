# FastMCP 4 expectation release r3 independent review

Date: 2026-09-02  
Reviewer: `codex-wp43-r3-independent-contract-falsification-reviewer`  
Candidate commit: `0664fb351b0d76c190cfdf84a3f50d895edcc911`  
Decision: **accepted**

## Result

No substantive contract defect was found. The immutable r3 candidate is accepted as the active WP43 expectation release. All 16 claims have claim-specific accepted dispositions, the corrected resource-denial expectations are statically grounded, the release mechanics are internally consistent, and the independent review is bound to the exact candidate commit and reviewed artifact bytes.

This is expectation-contract acceptance, not production implementation or runtime-behavior certification.

## Review authority and method

The review compared only the candidate release artifacts with the accepted FastMCP presentation-boundary design and amendment, WP43/WP48 plan contract, released CPG query-service Protobuf, and version-pinned grpcio 1.83.0 and Tonic 0.14.6 daemon references. FastMCP 4.0.0/Pydantic 2.13.4 reference material was used to check the presentation-boundary vocabulary and framework-versus-application extension distinction.

Expected values were evaluated from those static authorities. Production modules were not imported, candidate execution was not used, predecessor expected values were not used as authority, and target output was not used as an expected-value source.

## Claim dispositions

| Claim | Disposition | Falsification conclusion |
|---|---|---|
| RFV5-FM4-001 | accepted | Exact successor identity, pins, protocol era, and bridge state are static and fault-discriminated. |
| RFV5-FM4-002 | accepted | Modern admission and typed legacy pre-dispatch rejection preserve the adapter/daemon boundary. |
| RFV5-FM4-003 | accepted | The exact catalog permits only the empty framework UI advertisement and no application extensions. |
| RFV5-FM4-004 | accepted | Outer schemas remain presentation facts while semantic request authority remains daemon-owned. |
| RFV5-FM4-005 | accepted | Guarded input preserves daemon-owned fields/choices, safe presentation, and exactly-once admission. |
| RFV5-FM4-006 | accepted | ValidateQuery purity and the closed StartQuery outcome set encode atomic start. |
| RFV5-FM4-007 | accepted | Completion is bounded, authorized, advisory, and revalidated by the daemon. |
| RFV5-FM4-008 | accepted | Resource handles and read authority remain daemon-owned and range-bounded. |
| RFV5-FM4-009 | accepted | Cancellation is forwarded once; reconnect resumes the accepted query without resubmission. |
| RFV5-FM4-010 | accepted | Two-agent causality preserves one daemon and isolates processes, channels, principals, and STDIO. |
| RFV5-FM4-011 | accepted | Logical input capacity is separate from transport decode capacity; invalid maximum and past-end offset have distinct outer and typed mappings. |
| RFV5-FM4-012 | accepted | Public errors remain allowlisted and the fault exposes redaction and STDOUT failures. |
| RFV5-FM4-013 | accepted | The adapter authority census is an exact executable zero-state contract. |
| RFV5-FM4-014 | accepted | Live-scope zero state remains distinct from immutable, non-selectable predecessor history. |
| RFV5-FM4-015 | accepted | The active r3 path is registered and the candidate-neutral performance method is inherited byte-identically. |
| RFV5-FM4-016 | accepted | Six release artifacts and eight immutable inputs form exact, fail-closed hash sets. |

## Changed-claim adjudication

RFV5-FM4-011 correctly distinguishes three boundaries:

- a semantic request above the released logical limit but below the encoded gRPC transport decode limit maps to `RESOURCE_EXHAUSTED`;
- an invalid requested `maximum_bytes` above `EffectiveLimits.maximum_resource_chunk_bytes` maps to outer `INVALID_ARGUMENT` and typed `SAFE_ERROR_CODE_INVALID_REQUEST`;
- an otherwise valid range whose offset is one byte past the resource extent maps to outer `OUT_OF_RANGE` and typed `SAFE_ERROR_CODE_RANGE_NOT_SATISFIABLE`.

The negative fixture swaps both resource-denial status/code pairs and also changes the before-bytes/dispatch properties, so the corrected literals are causally discriminated rather than merely recorded. Every claim has a non-empty causal input and observation delta and an independently rejected negative fault.

RFV5-FM4-015 names the active r3 release path while retaining the preregistered performance method byte-for-byte from r1. RFV5-FM4-016 now reports 16 reviewed claims, zero pending handoff claims, six frozen artifacts, and eight immutable source inputs. The r1 and r2 release trees remain unchanged relative to the candidate commit.

## Hash binding

| Artifact | SHA-256 |
|---|---|
| `causal-fixtures.yaml` | `753cf58067d344bbb946d340de0ae5f9fc95d32a01b068c6ffe84dde60d7f3a0` |
| `expectations.yaml` | `ec5caeed0532f8109dd4519c3d05a10190a8480293ed7b5a12da048a6cf3acf2` |
| `independent-review.yaml` | `0cbcfef1b8ba3c2d3d74d66555abbf01ca13b8e362773714c5fab7f5896ee512` |
| `issuance.yaml` | `07a2db5ed0a636e343f7186c1ea1ed061c74adaf12b684001b140fb0c281376a` |
| `negative-fixtures.yaml` | `aa21f762eb9b60edfe667c2cf0bcf377fcf19cca9a8141330aeb40ad95dbfb28` |
| `performance-method.yaml` | `ceb48efae08732a452bbbafa9642f1130eb81cffefcd4e7b2869925d2be5c6df` |

`performance-method.yaml` is byte-identical across r1, r2, and r3. A scoped Git comparison also confirmed no r1 or r2 byte change relative to candidate commit `0664fb351b0d76c190cfdf84a3f50d895edcc911`.

## Validation

`PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest -q tooling/ci/test_fastmcp4_successor_expectations.py` passed: **60 passed in 2.02s**.

An initial invocation of the same test without the repository root on `PYTHONPATH` failed during collection with `ModuleNotFoundError: tooling`; it did not execute tests. The corrected repository-root invocation above is the recorded focused result. The primary reviewer will rerun the four WP43 selectors and remaining style/hash checks.

## Limitations

This was a static contract review. No production source, candidate behavior, runtime output, or WP48 observer/evidence content was inspected.

One broad filename search accidentally exposed only the filename `tests/integration/daemon/wp48_observer.rs` and claim, base-case, and fault labels. The file body, behavior, expected or observed values, runtime output, and WP48 evidence content were not inspected. Those incidental labels were excluded from expected-value adjudication, so `target_execution_used: false` remains accurate.
