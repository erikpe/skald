# Active and Planned Roadmaps

This directory contains implementation roadmaps and discovery records that are
planned, active, or still actionable. Completed roadmaps and resolved discovery
records are listed in the [archive](../archive/README.md). Current language and
compiler behavior is documented by the [living documentation](../README.md),
not by roadmap history.

## Implementation roadmaps

No implementation roadmap is currently active. The next architecture workstream
needs a target-selection and physical-realization design before implementation.

## Pending discovery and planning records

| Record | Status | Purpose | Next step | Dependencies |
| --- | --- | --- | --- | --- |
| [Low-level compiler architecture](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md) | Accepted direction; phase preparation complete; executable LIR model complete | Establish explicit LIR phases and shared/target backend ownership, then implement register allocation as a separate final workstream | Prepare the target-selection and physical-realization design against the completed model handoff | Verified final MIR; archived frozen phase design; existing backend/runtime contracts; AArch64 and future placement requirements |
| [Low-level compiler migration coverage](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md) | Preparation and executable model complete; native pipeline delivery pending | Maintain operation/helper coverage, phase authority, witnesses and baseline handoff through foundation adoption | Use the completed-model handoff to freeze target contracts; carry retained-artifact removal obligations into native roadmaps | Frozen model and archived phase design; qualified current-boundary witnesses and durable baseline; adoption cost clearance remains pending |
| [Low-level compiler architecture discoveries](LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md) | One actionable follow-up | Record independent maintenance findings encountered during phase-model implementation | Isolate golden build artifacts across concurrent invocations, or reject conflicting runs | Existing golden planner/compiler/native execution owners; independent of model publication |
| [Auric port feasibility](AURIC_PORT_FEASIBILITY.md) | Actionable investigation | Assess a silent Oric emulator in Skald, including CPU/VIA/AY keyboard behavior, video, tape loading, and comparison with Doom | Select firmware/game images and validate CPU/bus throughput plus the shared framebuffer bridge | Existing byte arithmetic, arrays and aliases; proposed shared-array native bridge; external firmware and game images |
| [Doom port feasibility](DOOM_PORT_FEASIBILITY.md) | Actionable investigation | Assess a Skald engine port with a small native platform library, including scope, language gaps, and division-helper evidence | Settle a minimal shared-array handle bridge and numerical helpers, then build the host/framebuffer spike | Existing arrays, I/O, primitive interop; optional 32-bit integer support |
| [Codebase cleanup audit](CODEBASE_CLEANUP_AUDIT.md) | Actionable audit | Rank repository-wide robustness, ownership, maintainability, and efficiency improvements | Select work using the accepted retrospective readiness table and per-finding prerequisites | Dependencies are recorded per finding; publication ownership is complete |
| [Copy-capability materialization discoveries](COPY_CAPABILITY_MATERIALIZATION_DISCOVERIES.md) | One deferred candidate | Preserve the narrower borrowed-provisional-view option after A17's measured no-go outcome | Gather representative evidence that capability-set cloning is material before promotion | [Archived experiment](../archive/COPY_CAPABILITY_MATERIALIZATION_ROADMAP.md); [measurements](../development/COPY_CAPABILITY_MATERIALIZATION_MEASUREMENTS.md) |
| [Optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) | Living inventory | Track implemented and plausible optimizations, placement, effort, value, prerequisites, and risks | Use measurements and architecture evidence to promote a candidate into a design or roadmap | [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) and candidate-specific measurements or contracts |
| [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) | Three constraints remain; phase preparation complete; executable LIR model complete | Record architectural limits on modular target-independent and target-specific optimization and their sequencing | Prepare the native target design against the completed LIR model; use evidence to select other prerequisite work | Existing pipeline, reachability, normalization, and MIR identity foundations |

The status matrix and focused language documents own feature maturity and open
language questions. The optimization catalog owns candidate lifecycle status.
The archive preserves completed decisions and delivery history without
duplicating them here.
