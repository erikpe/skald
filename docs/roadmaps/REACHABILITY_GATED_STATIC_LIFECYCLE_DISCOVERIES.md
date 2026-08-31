# Reachability-Gated Static Lifecycle Discoveries

Status: active companion record for the planned
[reachability-gated static lifecycle roadmap](REACHABILITY_GATED_STATIC_LIFECYCLE_ROADMAP.md).

This document records maintainability improvements, missing abstraction
boundaries, precision opportunities, and broader language or optimization work
found while implementing the frozen
[design](REACHABILITY_GATED_STATIC_LIFECYCLE_DESIGN_PROPOSAL.md) when those
findings are not required by the currently reviewed roadmap task.

Each future entry must state:

- the concrete problem and repository evidence;
- why it is outside the active task or frozen contract;
- the likely owner;
- priority and expected correctness, optimization, or maintainability impact;
- a bounded first implementation step; and
- dependencies or decisions that must precede it.

No additional discovery has been recorded yet. Expected but not pre-approved
topics include explicit eager or module initialization, runtime lazy statics,
activation narrowing after more precise target analysis, declaration or static-
slot identity compaction, and broader effect or alias proofs. Recording a topic
here does not expand the active roadmap or revise the frozen language contract.
