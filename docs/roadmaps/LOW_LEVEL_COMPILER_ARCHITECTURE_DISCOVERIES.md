# Low-Level Compiler Architecture Discoveries

Status: one actionable follow-up, 2026-09-17. Independent findings encountered
while implementing the [model roadmap](LOW_LEVEL_IR_MODEL_ROADMAP.md) belong here;
they do not expand its phase-model scope.

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
