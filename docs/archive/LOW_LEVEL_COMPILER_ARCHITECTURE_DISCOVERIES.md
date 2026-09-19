# Low-Level Compiler Architecture Discoveries

Status: resolved and archived on 2026-09-19. Findings from the
[architecture program](../roadmaps/LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md)
are preserved here with their resolutions.

## Golden artifact ownership across concurrent invocations

**Status:** resolved on 2026-09-19. The CLI now acquires a nonblocking,
process-lifetime advisory lock for the planned artifact root after compiler
discovery and before it prints the execution header or mutates case outputs. A
contending invocation exits with status 1, identifies the live owner when
available, and asks the caller to wait. The operating system releases ownership
when the process exits, while the persistent lock file avoids a deletion race.
Read-only inspection and allowed empty selections remain artifact-free.

The cross-process CLI regression holds the root in the parent, places sentinels
at the planned `assembly.s` and `program` paths, and launches an independent
runner process against the same root. It proves the contender is rejected
before either artifact is removed and that ownership can be reacquired after
the parent guard is dropped. Focused and full runner tests retain deterministic
case identities, normalized diagnostics and failure-artifact behavior.

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

Repository gates should still run serially because Cargo target outputs are
shared. Golden-specific overlap now fails explicitly rather than corrupting the
active owner's artifacts. This resolution required no compiler-phase change or
rollback.

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

**Status:** resolved by NP13; task baseline `1d248659`. Explicit signature-local
`AbiAreaLayout`/`AbiSlotLayout` metadata is validated independently of logical
shapes. Native verification reclassifies shapes and checks canonical word stride
and padding. ABI objects require exact extent/alignment; frame plans overlay
their protocol areas. Regressions cover forged extents/alignment, overflow,
byte/boolean stack pressure and synthetic memory-result areas.

**Priority:** high before native ABI-area objects/frame planning. **Owner:** shared
selected ABI area records and native symbolic frame planning. **Boundary:** settle
slot extent/alignment metadata with NP13, before native area objects are published.

The shared selected object checker previously computed an ABI area's extent by
summing representation byte widths. Native SysV stack slots occupy eight bytes
even for U8/Bool; incoming/outgoing byte offsets and outgoing sixteen-byte rounding
are target layout facts. A spilled byte has representation width one byte but
could not certify an eight-byte physical area's extent with the old shared sum.
Scalar selection introduces no ABI-area objects or physical offsets, so its
per-signature shape checks are unaffected. This is a prerequisite for the later
frame consumer, not a reason to weaken shape checks now.

Original acceptance requirement: give signature-local areas explicit checked
slot layout/extent authority, or
separate symbolic component-shape authority from target-verified area layout.
Preserve independent portable shared checks and reject forged sizes/alignments.
Add byte/boolean stack-pressure and mixed-bank area-object/frame tests, including
outgoing padding, incoming extents and overflow. Avoid inferring physical slot
stride from representation width or special-casing x86 inside shared checking.
