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
