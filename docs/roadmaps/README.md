# Active and Planned Roadmaps

This directory contains implementation roadmaps and discovery records that are
planned, active, or still actionable. Completed roadmaps and resolved discovery
records are listed in the [archive](../archive/README.md). Current language and
compiler behavior is documented by the [living documentation](../README.md),
not by roadmap history.

## Implementation roadmaps

| Roadmap | Status | Purpose | Next task | Dependencies |
| --- | --- | --- | --- | --- |
| [Complete low-level lowering migration](COMPLETE_LOW_LEVEL_LOWERING_MIGRATION_ROADMAP.md) | Active; LM01–LM11 complete | Expand the checked private native pilot to the complete supported language, helper, data and artifact surface | Lower array storage, construction, positions and anchors | Accepted frozen LA04 design; completed LA01–LA03 foundations and durable migration ownership; maintained migration coverage record |

## Pending discovery and planning records

| Record | Status | Purpose | Next step | Dependencies |
| --- | --- | --- | --- | --- |
| [Low-level compiler architecture](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md) | Accepted direction; LA04 implementation active | Establish explicit LIR phases and shared/target backend ownership, then implement register allocation as a separate final workstream | Continue complete lowering with array storage and safety protocols | Verified private pilot, accepted LA04 design and maintained coverage; adoption and allocation remain separate |
| [Complete low-level lowering migration design](COMPLETE_LOW_LEVEL_LOWERING_MIGRATION_DESIGN_PROPOSAL.md) | Accepted, frozen and promoted | Define whole-program no-fallback migration across the complete language, lifecycle, helper, data and artifact surface | Implement through the active roadmap | Completed LA01–LA03 designs and implementations; maintained migration coverage record |
| [Low-level compiler migration coverage](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md) | Preparation, executable model and private native scalar pilot complete | Maintain operation/helper coverage, phase authority, witnesses and baseline handoff through full migration and adoption | Carry the hardened pilot evidence into LA04 migration and LA05 adoption without clearing retained cost qualifications | Frozen model and archived phase design; qualified current-boundary witnesses and durable baseline; adoption cost clearance remains pending |
| [Complete lowering migration discoveries](COMPLETE_LOW_LEVEL_LOWERING_MIGRATION_DISCOVERIES.md) | One deferred performance observation | Track branch-heavy recursive optional CFG growth without weakening lifecycle semantics | Reassess after LM14 completes recursive container CFGs | LM11 optional lowering and the allocation-independent baseline placement pipeline |
| [Auric port feasibility](AURIC_PORT_FEASIBILITY.md) | Actionable investigation | Assess a silent Oric emulator in Skald, including CPU/VIA/AY keyboard behavior, video, tape loading, and comparison with Doom | Select firmware/game images and validate CPU/bus throughput plus the shared framebuffer bridge | Existing byte arithmetic, arrays and aliases; proposed shared-array native bridge; external firmware and game images |
| [Doom port feasibility](DOOM_PORT_FEASIBILITY.md) | Actionable investigation | Assess a Skald engine port with a small native platform library, including scope, language gaps, and division-helper evidence | Settle a minimal shared-array handle bridge and numerical helpers, then build the host/framebuffer spike | Existing arrays, I/O, primitive interop; optional 32-bit integer support |
| [Codebase cleanup audit](CODEBASE_CLEANUP_AUDIT.md) | Actionable audit | Rank repository-wide robustness, ownership, maintainability, and efficiency improvements | Select work using the accepted retrospective readiness table and per-finding prerequisites | Dependencies are recorded per finding; publication ownership is complete |
| [Copy-capability materialization discoveries](COPY_CAPABILITY_MATERIALIZATION_DISCOVERIES.md) | One deferred candidate | Preserve the narrower borrowed-provisional-view option after A17's measured no-go outcome | Gather representative evidence that capability-set cloning is material before promotion | [Archived experiment](../archive/COPY_CAPABILITY_MATERIALIZATION_ROADMAP.md); [measurements](../development/COPY_CAPABILITY_MATERIALIZATION_MEASUREMENTS.md) |
| [Optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) | Living inventory | Track implemented and plausible optimizations, placement, effort, value, prerequisites, and risks | Use measurements and architecture evidence to promote a candidate into a design or roadmap | [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) and candidate-specific measurements or contracts |
| [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) | Three constraints remain; phase preparation, executable LIR model and private native pilot complete | Record architectural limits on modular target-independent and target-specific optimization and their sequencing | Implement complete lowering migration; retain separate foundation adoption evidence | Existing pipeline, reachability, normalization, MIR identity and checked native-pilot foundations |

The status matrix and focused language documents own feature maturity and open
language questions. The optimization catalog owns candidate lifecycle status.
The archive preserves completed decisions and delivery history without
duplicating them here.
