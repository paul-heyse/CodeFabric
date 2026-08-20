---
artifact: design-dossier
design_id: codefabric-local-storage-dependency-isolation
version: v1
date: 2026-08-20
status: accepted
baseline_commit: a689f1ddf712c0f8fe5cf93d9a50a559f84e4b91
working_tree_digest: 3f5f35037716ac78d5f6676e0edbcb4744e2718b021cbcc3071f41ee013dc17e
primary_scope:
  - docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md
  - Cargo.toml
  - Cargo.lock
  - deny.toml
  - scripts/stable_graph_check.sh
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---

# CodeFabric local-storage dependency isolation design

## 1. Executive decision

Keep the exact Data Fabric baseline, including delta-rs revision
`9f9223197469897ef05ae4369eb4fd1390174e65` and its released
`buoyant_kernel` 0.25.x line. Correct the local-workstation invariant from an
unachievable compiled-code claim to an operational capability boundary:

- `local-workstation` does not enable `deltalake/s3`;
- the default graph contains neither `deltalake-aws` nor the AWS SDK family;
- CodeFabric accepts and registers only local-filesystem storage under the
  `local-workstation-v1` deployment profile;
- no cloud URL scheme, credentials, endpoint, or storage options are accepted
  by that profile;
- the resolved graph may nevertheless compile latent cloud support inside
  `object_store` because the pinned kernel's `arrow-58` feature requests it.

The known advisory set forced by that exact graph is explicit, version-bound,
machine-checked, and reviewed at WP19 before the data-fabric implementation is
accepted. This design does not claim that compiled-but-unreachable code is
absent.

## 2. Problem, outcomes, and non-goals

### 2.1 Observed contradiction

Data Fabric §2.1 and implementation plan v3 claimed that the default graph
compiled no cloud dependency. Exact graph resolution disproved that claim:

```text
deltalake 9f922319
  -> buoyant_kernel 0.25.1 feature arrow-58
     -> object_store 0.13.2 features aws, azure, gcp, http
        -> quick-xml 0.39.4
```

The dependency edge is unconditional within the released kernel's
`arrow-58` feature. Cargo feature unification cannot subtract features selected
by a dependency. A direct dependency declaration with fewer features therefore
cannot repair the graph.

### 2.2 Outcomes

- State exactly what the default profile does and does not permit.
- Preserve the compile-probed Delta/Arrow/DataFusion compatibility baseline.
- Keep S3 unavailable unless the explicit `s3-storage` feature is selected.
- Keep security and maintenance exceptions narrow, reproducible, and expiring
  into a named review packet.
- Prevent CI from reporting a false no-cloud proof.

### 2.3 Non-goals

- Shipping or exercising an S3 backend in Waves 0–3.
- Maintaining a CodeFabric fork of `buoyant_kernel` or `object_store`.
- Treating compilation of latent cloud code as runtime authorization.
- Resolving licensing policy; the user explicitly excluded it from this
  implementation.

## 3. Constraints and measurable quality attributes

- The exact FAB §2.1 family pins remain aligned.
- Local storage is the only accepted storage authority through M04.
- Default-graph validation must reject `deltalake-aws` and `aws-sdk-*`.
- S3-graph validation must prove `deltalake-aws` appears only with
  `s3-storage`.
- Every ignored advisory must name an exact RustSec ID, selected package and
  version, rationale, owner packet, and review trigger in committed data.
- The deny ignore list and committed exception registry must be equal.
- WP19 cannot close until every exception is removed or deliberately renewed
  through a new design/plan decision with current reachability evidence.

## 4. Current-state evidence and architecture

| ID | Claim | Status | Evidence | Coverage / limits | Used by |
|---|---|---|---|---|---|
| E-01 | Kernel `arrow-58` activates all four `object_store` cloud features | observed | local `buoyant_kernel-0.25.1/Cargo.toml`; `cargo metadata --locked` | exact resolved macOS graph | D-01 |
| E-02 | Default graph omits the Delta S3 implementation and AWS SDK | observed | `cargo tree --locked --edges normal`; stable graph check | package-presence proof, not dead-code proof | I-01 |
| E-03 | `quick-xml 0.39.4` is selected by `object_store 0.13.2` | observed | `cargo tree -i quick-xml`; `Cargo.lock` | exact lockfile | R-01 |
| E-04 | `quick-xml 0.39.4` has RUSTSEC-2026-0194/0195; fixed line is 0.41+ | observed | `cargo audit --json`; installed advisory database | advisory database as of 2026-08-20 | R-01 |
| E-05 | `object_store 0.13.2` requires quick-xml `0.39` | observed | installed exact crate manifest | prevents a lockfile-only 0.41 update | D-01 |

## 5. Target architecture

The deployment profile, not latent dependency code, owns storage authority:

```text
local-workstation-v1
  -> local filesystem URI and local Delta namespace only
  -> no cloud handler registration or cloud configuration acceptance
  -> default Cargo feature set

s3-storage (outside Waves 0-3 runtime scope)
  -> explicit deltalake/s3 feature
  -> deltalake-aws and AWS SDK become resolved implementation dependencies
```

External library capability remains behind application-owned configuration and
provider factories. A compiled provider is not an enabled provider.

## 6. Target invariants, contracts, ownership, and flows

- **I-01 — Default provider isolation.** Default resolution contains no
  `deltalake-aws` or `aws-sdk-*`, and local runtime construction accepts only
  the filesystem provider.
- **I-02 — Explicit S3 activation.** `s3-storage` is the only CodeFabric
  feature that enables `deltalake/s3`; its graph must contain
  `deltalake-aws`.
- **I-03 — Honest compiled-surface reporting.** Graph evidence reports the
  kernel-forced `object_store` cloud features and never restates them as
  absent.
- **I-04 — Bounded exception ownership.** The advisory exception registry and
  deny configuration agree exactly; WP19 owns the mandatory review.
- **I-05 — No runtime cloud authority through M04.** No Waves 0–3 code accepts
  cloud schemes, endpoints, credentials, or storage-option maps.

Doctrine: I-01/I-05 advance Principle 8 (least privilege); I-03 maintains
Principles 27–29 (provenance, structured evidence, versioned contracts); I-04
advances Principle 31 (executable governance). No doctrine principle requires
pretending unused compiled capabilities are absent.

## 7. Library and platform decisions

- **LD-01 — Adopt exact pinned graph.** Preserve the FAB family pins and
  delta-rs revision because their public type and behavior compatibility is
  already compile-probed.
- **LD-02 — Wrap provider activation.** CodeFabric deployment/profile code,
  not `object_store` feature availability, decides which backend can be
  constructed.
- **LD-03 — Reject a local library fork.** Removing kernel-selected features
  by vendoring would create an unpublished compatibility and security patch
  burden before any product behavior exists.
- **LD-04 — Defer compatible upstream refresh.** WP19 re-evaluates supported
  delta/kernel/object_store releases and removes or renews exceptions using a
  new accepted decision; no silent lockfile drift is allowed.

## 8. Alternatives and rationale

### A. Operational isolation with explicit latent surface — selected

Preserves verified compatibility, keeps local authority narrow, and makes the
actual graph and risk visible. It has the smallest maintenance surface.

### B. Vendor or fork the kernel

Could make the literal no-cloud compile claim true, but CodeFabric would own a
custom upstream variant and its Arrow/Delta compatibility. Rejected under
KISS, YAGNI, and Principle 6 because local-only runtime policy already provides
the needed boundary.

### C. Change the Delta/kernel baseline immediately

Potentially removes the feature leak, but no replacement has passed the
existing exact-version compatibility probes. Deferred to WP19's deliberate
upgrade review rather than changing a load-bearing baseline speculatively.

## 9. Clean-sheet challenge

Without the existing seed or v3 prose, the preferred design would still place
backend authority in an application-owned deployment/profile boundary and
would report the actual dependency graph honestly. It would not fork a core
storage library solely to optimize unreachable compiled code before the S3
deployment profile is in scope.

## 10. Legacy disposition matrix

| ID | Surface | Disposition | Exit condition |
|---|---|---|---|
| L-01 | FAB §2.1 literal no-cloud-compilation claim | replace | owner text states operational boundary and latent feature fact |
| L-02 | v3 default/S3 proof wording | replace in v4 | graph validator proves the corrected package and activation invariants |
| L-03 | unstructured advisory suppressions | delete/replace | exact JSON registry and equality check are active |

## 11. Transition, rollback, and decommission

Update FAB §2.1 in place because the suite is explicitly revised in place
while in flux. Preserve v3 as immutable history; issue v4 with stable packet
IDs and only targeted corrections. Reconcile execution state to v4, reopen
WP01, and retain all already-green WP01 evidence. Rollback is deletion of the
new dossier/registry plus reversion of the owner and v4 artifacts; it does not
require changing application data.

## 12. Failure, security, lifecycle, observability, and performance

Cloud URL or option input under `local-workstation-v1` is a configuration
error before provider construction. Advisory exceptions are evidence records,
not claims of safety. The two quick-xml vulnerabilities are reachable only if
cloud XML response parsing is invoked; Waves 0–3 do not authorize that route.
Dependency graph and exception status are CI output. No performance claim is
made.

## 13. Test oracle and conformance strategy

- `cargo metadata` proves the actual `object_store` feature surface.
- Default `cargo tree` proves absence of Delta's S3 implementation and AWS SDK.
- S3-feature `cargo tree` proves explicit activation.
- A registry checker proves exact agreement among Cargo.lock, deny ignores,
  and exception metadata.
- Runtime provider-factory tests in WP19 reject cloud inputs under the local
  profile.
- `cargo deny check advisories bans sources` and `cargo audit` pass only with
  the registered bounded exceptions.

## 14. Risks, assumptions, and design-level replan triggers

- **R-01.** Advisory severity or reachability changes before WP19. Trigger:
  new RustSec data or any local-profile call into cloud XML parsing; reopen
  immediately.
- **R-02.** A compatible upstream stack remains unavailable at WP19. Trigger:
  WP19 cannot silently renew; it needs an explicit decision that can disable
  S3 support, adopt a new baseline, or accept a renewed bounded exception.
- **A-01.** Waves 0–3 provider code remains local-only. Owner: WP19. Oracle:
  provider-factory negative tests and zero cloud-configuration consumers.

## 15. Acceptance decision

**Accepted for implementation planning.** The exact resolved graph and
advisory reachability are evidenced; no load-bearing API assumption remains
unverified for this bounded correction.

## 16. Evidence ledger

E-01–E-05 above are the complete evidence set for this correction. Their
coverage is the exact committed dependency graph on 2026-08-20 plus installed
crate sources and the current RustSec database; WP19 must refresh all of them.
