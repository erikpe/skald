# Low-Level Compiler Architecture Discoveries

Status: one independent follow-up and one frame-integration prerequisite;
signature-boundary ABI prerequisite resolved,
2026-09-18. Findings from the [architecture program](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md)
are tracked here. Independent maintenance work stays outside active task scope;
contract gaps must be resolved before their dependent consumers.

## Golden artifact ownership across concurrent invocations

**Priority:** medium. **Owner:** golden planning/compiler execution, independently
of low-level compiler phases. **Boundary:** one focused runner change with
cross-process artifact-ownership regressions.

Two overlapping full checks in one checkout demonstrated a compile-stage failure
for `operators/booleans::short_circuit_selected_failure`: the compiler exited zero
with empty stderr, but `assembly.s` was missing. That run recorded 649 passes and
one cancellation; the overlapping run passed all 650 observations. This overlap
was introduced during validation, not by the compiler changes.

The [CLI](../../crates/skald-golden/src/cli/mod.rs) uses a shared
`build/golden/cases` root. The [planner](../../crates/skald-golden/src/plan/builder.rs)
assigns deterministic case directories under it, and
[compiler execution](../../crates/skald-golden/src/compile/invoke.rs) removes the
same `assembly.s`/repeat paths before each compilation. Concurrent invocations can
therefore remove or overwrite each other's outputs. Linked executable paths also
have stale-output removal in
[native execution](../../crates/skald-golden/src/execute/sequential.rs). Private
native run sandboxes do not isolate these build artifacts.

Give each invocation exclusive build-artifact ownership: choose either a unique
invocation directory containing stable case directories, or a root lock with a
clear concurrent-run rejection. Preserve deterministic case identities,
diagnostic normalization and failed-artifact reporting/retention. Add a controlled
two-process regression using existing fake-compiler support; check assembly and
linked executable isolation, including success/failure cleanup. Avoid relying on
scheduling delays alone to reproduce the race.

Until addressed, run golden/full checks serially within a checkout. The model
task's final gate was rerun serially; this candidate requires no compiler-phase
change or rollback.

## ABI slot shapes must be local to the signature boundary

**Status:** resolved by NP07; task baseline `5369ea69`, implementation committed
with NP08 as `e35be34f`. **Owner:** shared selected ABI/context/descriptor checking.

`SelectionContext` now stores ABI areas by checked signature. Entry/return and
symbolic ABI object validation use the owner/declared signature; call bindings
and operand slots use the called signature. Component, area, index, width and
bank checks and exact context/snapshot authority remain strict. No per-callable
context bridge or globally weakened slot check remains.

[Shared publication regression](../../crates/skald-compiler/src/backend/selected/tests/verification/boundaries.rs)
publishes two call sites using outgoing slot zero with integer and floating
representations in one graph/context. Native selection regressions publish
seven-integer and nine-floating entry signatures together, and reject wrong
areas, indices and types. Native classifier pressure tests retain their
independent per-signature plans.

## Symbolic ABI area extent needs target slot layout

**Priority:** high before native ABI-area objects/frame planning. **Owner:** shared
selected ABI area records and native symbolic frame planning. **Boundary:** settle
slot extent/alignment metadata with NP13, before native area objects are published.

The shared selected object checker currently computes an ABI area's extent by
summing representation byte widths. Native SysV stack slots occupy eight bytes
even for U8/Bool; incoming/outgoing byte offsets and outgoing sixteen-byte rounding
are target layout facts. A spilled byte has representation width one byte but
cannot certify an eight-byte physical area's extent with the current shared sum.
Scalar selection introduces no ABI-area objects or physical offsets, so its
per-signature shape checks are unaffected. This is a prerequisite for the later
frame consumer, not a reason to weaken shape checks now.

Give signature-local areas explicit checked slot layout/extent authority, or
separate symbolic component-shape authority from target-verified area layout.
Preserve independent portable shared checks and reject forged sizes/alignments.
Add byte/boolean stack-pressure and mixed-bank area-object/frame tests, including
outgoing padding, incoming extents and overflow. Avoid inferring physical slot
stride from representation width or special-casing x86 inside shared checking.
