# Target-Independent Whole-World Reachability Discoveries

Status: active follow-up record for the completed
[target-independent whole-world reachability roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md).

This document records maintainability improvements, precision opportunities,
and broader optimization work found while implementing the frozen
[reachability design](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
when those findings were not required to satisfy its reviewed contracts.

Each future entry should state the concrete problem and evidence, why it was
outside the roadmap, the likely owner, priority and impact, a bounded first
step, and relevant dependencies.

## Lifecycle dependency extraction has one oversized implementation owner

**Evidence.** `passes/reachability/lifecycle.rs` is roughly 1,100 lines and
owns four independently understandable concerns: class copy/finalization,
recursive optional cleanup, shared-owner finalizer expansion, and array
default/copy/assignment/destruction expansion. The boundaries already have
typed helper methods, but all implementation details and imports remain in one
file.

**Why outside the roadmap.** The closing audit found no duplicate semantic
walker and all lifecycle-family tests exercise the shared owner. Splitting the
file while activation was uncovering dependency omissions would add broad
movement without changing or strengthening the contract.

**Likely owner.** The `passes::reachability` lifecycle submodule.

**Priority and impact.** Medium maintainability impact, low semantic urgency.
Smaller cohesive owners would make future lifecycle variants and reviews less
error-prone without changing analysis results.

**Bounded first step.** Turn `lifecycle.rs` into a concise facade and extract
class, optional/shared, and array dependency implementations into private
siblings, preserving the current methods, exhaustive matches, error types, and
focused test suite byte-for-byte.

**Dependencies.** None; this is a behavior-preserving internal refactor.

## Runtime-trace metadata identities can reflect pruned source bodies

**Evidence.** A profile-equivalence fixture with one dead function produces
equivalent retained machine artifacts and native behavior, but the numeric
labels of retained runtime-trace string records can differ between `none` and
the default profile. Complete trace metadata is interned before the
target-private artifact walk removes records belonging only to dead bodies.

**Why outside the roadmap.** Artifact bytes remain deterministic within each
profile, the final symbol closure is correct, and stdout, stderr, status, and
stack traces are equivalent. Cross-profile assembly identity was not part of
the language or optimization-off contract.

**Likely owner.** x86-64 runtime-trace metadata planning together with the
machine-artifact retention boundary.

**Priority and impact.** Low correctness priority, medium debugging and diff
quality. Canonical retained-only numbering would make profile comparisons
smaller and may marginally reduce pre-emission metadata work.

**Bounded first step.** After artifact closure is known, rebuild or remap only
retained trace strings, contexts, and locations into canonical semantic order;
then assert byte-identical retained trace metadata where the executable
closure itself is identical.

**Dependencies.** Keep target-independent semantic retention and
target-private generated-artifact retention as separate authorities.

## Static-effect inference still exposes an infallible legacy facade

**Evidence.** Shared dependency extraction now returns
`MirDependencyExtractionError` for malformed function, method, function-type,
storage, field, optional, class, array, and related identities. Static-effect
inference, however, predates structured analysis failures and exposes an
infallible result API; its adapter can only assert the existing invariant that
preliminary and final MIR were verified before analysis.

**Why outside the extraction milestone.** Changing that public and
planner-facing API would require threading a new internal compiler-error
channel through static planning, authority issuance, final realization, and
their callers. The shared extraction work instead kept all established
lifecycle diagnostics and result shapes exactly stable while making the
reusable extractor itself fallible.

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

**Dependencies.** Final verification supplies the reachability seal
and structured program-level analysis-failure plumbing. This remains a bounded
follow-up rather than part of the completed reachability roadmap.

Expected but not pre-approved topics include declaration and metadata
compaction, rapid-type-analysis precision, call-site function-value points-to
analysis, reachability-preservation proofs, broader interprocedural summaries,
and target metadata reduction beyond what verified sparse definitions require.
Recording a topic here does not add it retroactively to the completed roadmap.
