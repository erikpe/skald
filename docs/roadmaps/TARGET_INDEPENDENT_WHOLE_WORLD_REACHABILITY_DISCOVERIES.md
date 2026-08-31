# Target-Independent Whole-World Reachability Discoveries

Status: active companion to the in-progress
[target-independent whole-world reachability roadmap](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md).
## Static-effect inference still exposes an infallible legacy facade

**Evidence.** Shared dependency extraction now returns
`MirDependencyExtractionError` for malformed function, method, function-type,
storage, field, optional, class, array, and related identities. Static-effect
inference, however, predates structured analysis failures and exposes an
infallible result API; its adapter can only assert the existing invariant that
preliminary and final MIR were verified before analysis.

**Why outside WRR1.** Changing that public and planner-facing API would require
threading a new internal compiler-error channel through static planning,
authority issuance, final realization, and their callers. WRR1 instead keeps
all established lifecycle diagnostics and result shapes exactly stable while
making the reusable extractor itself fallible.

**Likely owner.** Static-lifecycle orchestration and the verified preliminary-
MIR boundary, coordinated with the compiler driver.

**Priority and impact.** Low while every production caller honors the verified-
MIR precondition; medium for testability and defensive compiler robustness.
It would remove one remaining assertion boundary and make malformed analysis
inputs uniformly inspectable.

**Bounded first step.** Introduce a verified preliminary-MIR wrapper or an
internal `Result`-returning static-effect entry point, migrate planning and
realization callers, then retain the current convenience API only where its
verified precondition is encoded in the input type.

**Dependencies.** WRR3 now supplies the final verified-MIR reachability seal
and structured program-level analysis-failure plumbing. This remains a bounded
follow-up rather than part of later reachability roadmap tasks.

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
