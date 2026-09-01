# Reachability-Gated Static Lifecycle Discoveries

Status: active follow-up record for the completed
[reachability-gated static lifecycle roadmap](../archive/REACHABILITY_GATED_STATIC_LIFECYCLE_ROADMAP.md).

This document records maintainability improvements, missing abstraction
boundaries, precision opportunities, and broader language or optimization work
found while implementing the frozen
[design](../archive/REACHABILITY_GATED_STATIC_LIFECYCLE_DESIGN_PROPOSAL.md) when those
findings are not required by the currently reviewed roadmap task.

Each future entry must state:

- the concrete problem and repository evidence;
- why it is outside the active task or frozen contract;
- the likely owner;
- priority and expected correctness, optimization, or maintainability impact;
- a bounded first implementation step; and
- dependencies or decisions that must precede it.

## Function-value closure mechanics now have two solver owners

**Problem and evidence.** Target-independent final reachability and semantic
static activation intentionally solve different root policies and produce
different facts, but both now maintain the same exact-function-type coupled
state: reached callable-address formations add candidates, reached indirect
sites consume only those candidates, and either discovery order must complete
the same fixed point. The small state machine currently appears in both
`passes::reachability::solve` and `passes::static_lifecycle::activation::solve`,
including canonical formation replacement and late candidate/site coupling.
Shared dependency extraction prevents target-resolution drift, but it does not
prevent these two consumers from evolving the coupling mechanics differently.

**Why deferred.** Extracting a reusable solver component requires a narrow
callback or event API that can preserve each consumer's distinct edge,
witness, runtime-entity, and error ownership. Designing that boundary while
landing the first activation solver would have enlarged the roadmap task and
risked obscuring the semantic closure being validated.

**Likely owner.** `passes::reachability`, as the neutral owner of callable-
address formations, indirect-call sites, exact function types, and possible-
target selection.

**Priority and impact.** Medium maintainability and correctness priority before
adding a third whole-program consumer or changing function-value semantics.
There is no current precision or artifact regression; focused cross-consumer
tests pin the present rule.

**Bounded first step.** Introduce a private neutral coupled-target worklist that
accepts reached execution-node events and returns newly selected indirect
execution edges in canonical order. Migrate final reachability first, then
activation, retaining their existing witnesses and result models outside the
shared component.

Expected but not pre-approved topics also include explicit eager or module
initialization, runtime lazy statics, activation narrowing after more precise
target analysis, declaration or static-slot identity compaction, and broader
effect or alias proofs. Recording a topic here does not expand the active
roadmap or revise the frozen language contract.
