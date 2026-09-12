# Cleanup Retrospective Discoveries

Status: resolved and archived after the
[retrospective roadmap](CLEANUP_RETROSPECTIVE_ROADMAP.md). Evidence was recorded
during R01 at revision `64b6da73b41ed2ec6afe0e1401b3735484847e6d`.

## Guard the neutral capability service's dependencies

**Priority:** P2. **Owner:** compiler phase-boundary integration tests.
**Status:** resolved (2026-09-12). **Blocking:** resolved; future capability
refactoring is protected by the explicit service policy.

**Evidence:** [phase policies](../../crates/skald-compiler/tests/phase_boundaries.rs)
enumerate compiler phase roots but omit `type_capabilities` as a scanned owner.
Consequently a future HIR/type-check dependency inside that service would not
be inspected by the forward-pipeline test. Current production source inspected
in R01 uses resolved facts; this is a prevention gap, not an observed reverse
dependency. See the [review](CLEANUP_RETROSPECTIVE_REVIEW.md).

**Bounded follow-up:** explicitly represent the neutral service's allowed
dependencies in the existing guard, preserving legitimate resolution consumers
and the distinction between phase roots and supporting semantic services.
Decide the policy before changing the scanner; avoid a general Rust dependency
analysis framework or blind inclusion of every utility module.

**Validation:** a synthetic `type_capabilities` dependency on HIR/type checking
must fail; its legitimate resolved/source inputs and phase consumers must pass.
Retain existing exception and scanner tests, document the coverage limit, then
run `make check` and `make msrv-check`. This policy extension is a separate
behavioral test change, rather than incidental R01 documentation cleanup.

**Delivered:** the existing source guard now distinguishes compiler phases
from governed supporting semantic services and scans `type_capabilities` as an
owner. Its policy permits source and resolved inputs; the resolution and type-
checking policies explicitly permit the service as a consumer dependency.
Focused synthetic tests reject service imports of HIR, type-check, MIR, passes,
and backend roots, verify the two allowed consumers, and require exactly one
policy for every governed root. The living phase and test contracts document
both the boundary and the scanner's direct-source coverage limit.

**Validation completed (2026-09-12):** all 10 focused `phase_boundaries`
integration tests passed. The full `make check` repository gate and Rust 1.82
`make msrv-check` gate also passed before this record was archived.


## Complete candidate publication ownership

**Priority:** P1. **Owner:** resolver publication. **Status:** resolved
(2026-09-12); implementation and acceptance are recorded in the
[archived publication ownership roadmap](PUBLICATION_OWNERSHIP_ROADMAP.md).
**Blocking:** resolved. Future fields and rejection changes use the implemented
owned-product and exhaustive-assembly boundary.

R02's [field inventory and decision](CLEANUP_RETROSPECTIVE_REVIEW.md#r02--publication-acceptance-decision)
show that centralized rollback still requires a manually maintained field list.
Retained error evidence can reference rejected declarations, so the output
must not be mistaken for a closed executable program. This is an architectural
follow-up, not a demonstrated new correctness defect.

Publication now validates borrowed products, selects class/hierarchy and
executable products together, selects interfaces against that class-selected
view, and exhaustively assembles `ResolvedProgram` once. Saved ordinary
snapshots move into rejected output without restoration clones. Existing
partial error output, identity allocation, class-family rejection, diagnostics,
and independent class survival remain covered by the publication regressions.


## Prevent range probing from consuming absent class declarations

**Priority:** P0. **Owner:** resolver semantic range completion and provisional
body environments. **Status:** resolved (2026-09-12); full repair gates passed.
**Blocking:** resolved; R03 verification resumed after this repair. This was a
demonstrated panic, distinct from the then-pending manual rollback work.

The new `class_rejection_preserves_canonical_string_fields_and_literal_bytes`
test in [publication tests](../../crates/skald-compiler/src/resolve/resolver/program/specialization/publication_tests.rs)
loads the canonical standard library and resolves:

```ska
class Owner<T> { value: shared T; }
fn use(ref value: Owner<i64>) -> unit {}
fn main() -> i64 { "publication evidence"; return 0; }
```

Run `cargo test --locked -p skald-compiler class_rejection_preserves_canonical_string_fields_and_literal_bytes --lib`.
The intended result is one unsatisfied requirement diagnostic and retained
canonical string/literal evidence. Instead it panics with `resolved object
place must reference a class` in body member selection. The backtrace reaches
`semantic_range_requests::discover_semantic_range_requests` through class-body
resolution, before final publication. The test remains enabled with its intended
assertions, not converted into an expected panic or hidden with an ignore.

**Separately scoped repair:** trace how provisional specialization failure and
range-bearing standard-library bodies produce lookup identities absent from the
probe's class/hierarchy inputs. Make the probe consume a consistent declaration
view or skip unavailable work while preserving authoritative diagnostics and
fixed-point discovery. Do not merely replace the assertion or suppress all body
resolution after any error. First pin the failure mechanism with the retained
fixture, then implement the smallest consistent-environment repair. Preserve
valid range work and unrelated diagnostic owners.

**Exit criteria/tests:** the retained regression passes without panic and with
its exact intended diagnostic; existing nested/generated range and isolation
suites pass; ordinary publication tests pass. Run `make check` and
`make msrv-check` before unblocking R03. This repair is independent of replacing
publication rollback and must not be absorbed into that representation change.


**Repair delivered:** generated-family materialization failure now prevents
range probing and authoritative body analysis against incomplete declarations.
Structural requirement diagnostics still run; declaration-dependent capability
queries are deferred while reserved class identities lack declarations. This
also prevents the lifecycle-query panic exposed after bypassing the first
probe failure. The canonical string regression and its valid control pass.
Body-derived metadata is intentionally absent on early materialization failure;
this is documented in the phase contract and review. Final gate results are
recorded in the review's repair section.
