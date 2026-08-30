# Target-Independent Whole-World Reachability Discoveries

Status: active companion to the planned
[target-independent whole-world reachability roadmap](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md).
No follow-up discovery is currently recorded.

This document records maintainability improvements, precision opportunities,
and broader optimization work found while implementing the frozen
[reachability design](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
when those findings are not required to satisfy the active roadmap's frozen
contracts.

Each future entry should state:

- the concrete problem and implementation evidence;
- why it is outside the current roadmap task;
- the likely compiler owner;
- priority and expected impact;
- a bounded first implementation step; and
- dependencies on unfinished roadmap work.

Expected but not pre-approved topics include declaration and metadata
compaction, rapid-type-analysis precision, call-site function-value points-to
analysis, reachability-preservation proofs, broader interprocedural summaries,
and target metadata reduction beyond what verified sparse definitions require.
Recording a topic here does not make it part of WRR0 through WRR8.
