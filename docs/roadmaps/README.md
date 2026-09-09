# Active and Planned Roadmaps

This directory contains implementation roadmaps and discovery records that are
planned, active, or still actionable. Completed roadmaps and resolved discovery
records are listed in the [archive](../archive/README.md). Current language and
compiler behavior is documented by the [living documentation](../README.md),
not by roadmap history.

## Implementation roadmaps

There are no implementation roadmaps currently planned or in progress. When a
roadmap becomes active, this section records its status, purpose, next task,
and material dependencies.

## Pending discovery and planning records

| Record | Status | Purpose | Next step | Dependencies |
| --- | --- | --- | --- | --- |
| [Codebase cleanup audit](CODEBASE_CLEANUP_AUDIT.md) | Actionable audit | Rank repository-wide robustness, ownership, maintainability, and efficiency improvements | Select an open finding and either implement its bounded first change or create a focused roadmap when its scope requires staged delivery | Dependencies are recorded per finding |
| [Optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) | Living inventory | Track implemented and plausible optimizations, placement, effort, value, prerequisites, and risks | Use measurements and architecture evidence to promote a candidate into a design or roadmap | [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) and candidate-specific measurements or contracts |
| [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) | Three constraints remain unplanned | Record architectural limits on modular target-independent and target-specific optimization and their sequencing | Design a remaining constraint only when a measured optimization need justifies its prerequisite work | Existing pipeline, reachability, normalization, and MIR identity foundations |

The status matrix and focused language documents own feature maturity and open
language questions. The optimization catalog owns candidate lifecycle status.
The archive preserves completed decisions and delivery history without
duplicating them here.
