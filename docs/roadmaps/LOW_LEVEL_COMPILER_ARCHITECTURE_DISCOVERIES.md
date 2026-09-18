# Low-Level Compiler Architecture Discoveries

Status: one independent follow-up and one native-integration prerequisite,
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

**Priority:** high before native selected integration. **Owner:** shared selected
ABI/context/descriptor checking. **Boundary:** fix validation scope before the
[native selected-schema task](TARGET_SELECTION_PHYSICAL_REALIZATION_ROADMAP.md#np07--concrete-scalar-selected-payload-and-verifier)
and freeze its contract before native call consumers. This is an in-roadmap
correctness prerequisite, not an independent post-adoption optimization.

At the committed publication endpoint `2cbd7ffd`, `SelectionContext` owns one `AbiAreas`
with representation vectors for incoming/outgoing/results. `require_abi_binding`
checks each slot against those context-wide vectors; selected descriptor checking
uses it for call bindings. `SelectedProgramBuilder` also requires receipts from
the same selection context. A seven-integer signature needs integer slot zero;
a nine-floating signature needs floating slot zero. Both occupy the same eight
bytes but have different representations. A single context cannot presently
validate both entry shapes or both outgoing call shapes, even though the native
ABI legitimately reuses that slot at different boundaries.

The native owner regression `stack_slot_shapes_belong_to_each_signature_boundary`
proves distinct checked signatures produce those distinct valid plans. Existing
synthetic tests do not establish production support for heterogeneous boundary
shapes. Scope slot validation to the relevant callable entry or call signature/
site, while preserving component roles, slot bounds, width/bank checking and exact
context/snapshot authority. Do not relax representation checks globally or solve
this by changing per-callable contexts without reconciling program authority.
Add selected-publication tests for heterogeneous incoming signatures and multiple
call sites using slot zero with different types, plus wrong-area/out-of-range/
wrong-type negatives. If the fix exceeds the selected-schema PR, split an explicit
prerequisite task and amend the owning contract before its consumers.
