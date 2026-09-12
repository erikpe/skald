# Cleanup Retrospective Discoveries

Status: actionable follow-up after the
[retrospective roadmap](CLEANUP_RETROSPECTIVE_ROADMAP.md). Evidence was recorded
during R01 at revision `64b6da73b41ed2ec6afe0e1401b3735484847e6d`.

## Guard the neutral capability service's dependencies

**Priority:** P2. **Owner:** compiler phase-boundary integration tests.
**Status:** pending. **Blocking:** does not block retrospective completion or
current capability use; settle the guard before expanding neutral-service
dependencies in subsequent capability refactoring.

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


## Complete candidate publication ownership

**Priority:** P1. **Owner:** resolver publication. **Status:** planned in the
[publication ownership roadmap](PUBLICATION_OWNERSHIP_ROADMAP.md).
**Blocking:** candidate-dependent field additions and publication/rejection
changes; not retrospective closure or independent compiler work.

R02's [field inventory and decision](CLEANUP_RETROSPECTIVE_REVIEW.md#r02--publication-acceptance-decision)
show that centralized rollback still requires a manually maintained field list.
Retained error evidence can reference rejected declarations, so the output
must not be mistaken for a closed executable program. This is an architectural
follow-up, not a demonstrated new correctness defect.

Implement owned selection and exhaustive assembly under the linked roadmap,
using R03's rejection characterization. Preserve existing partial error output,
identity allocation, class-family rejection and independent class survival.
The implementation roadmap owns tasks and gates; this record tracks the risk
without duplicating that implementation plan.


## Prevent range probing from consuming absent class declarations

**Priority:** P0. **Owner:** resolver semantic range completion and provisional
body environments. **Status:** resolved (2026-09-12); full repair gates passed.
**Blocking:** resolved; R03 verification may resume. This was a demonstrated
panic, distinct from the still-pending manual rollback work.

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
