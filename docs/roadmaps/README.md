# Active and Planned Roadmaps

This directory contains implementation roadmaps and discovery records that are
planned, active, or still actionable. Completed roadmaps and resolved discovery
records are listed in the [archive](../archive/README.md). Current language and
compiler behavior is documented by the [living documentation](../README.md),
not by roadmap history.

## Implementation roadmaps

| Roadmap | Status | Purpose | Next task | Dependencies |
| --- | --- | --- | --- | --- |
| [Cleanup architecture retrospective](CLEANUP_RETROSPECTIVE_ROADMAP.md) | In progress; R01–R02 complete | Verify delivered frontend cleanup outcomes, settle publication acceptance, and identify prerequisites for further improvements | R03 — Close targeted verification gaps | Delivered A08–A12; supporting A07, A34, and A43 evidence in the cleanup audit |

| [Resolver publication ownership](PUBLICATION_OWNERSHIP_ROADMAP.md) | Planned | Replace manual rollback with owned selection and exhaustive assembly | P01 — Partition candidate ownership | Retrospective R02 decision and R03 characterization |

## Pending discovery and planning records

| Record | Status | Purpose | Next step | Dependencies |
| --- | --- | --- | --- | --- |
| [Cleanup retrospective review](CLEANUP_RETROSPECTIVE_REVIEW.md) | In progress | Trace original frontend cleanup promises to source, named tests, limitations, and proposed dispositions | Verify rejection contracts through R03 | Cleanup retrospective roadmap |
| [Cleanup retrospective discoveries](CLEANUP_RETROSPECTIVE_DISCOVERIES.md) | Actionable follow-up | Track capability dependency guarding and publication ownership | Implement the publication ownership roadmap and the separate capability guard follow-up | Existing phase-boundary guard; reviewed neutral-service contract |
| [Auric port feasibility](AURIC_PORT_FEASIBILITY.md) | Actionable investigation | Assess a silent Oric emulator in Skald, including CPU/VIA/AY keyboard behavior, video, tape loading, and comparison with Doom | Select firmware/game images and validate CPU/bus throughput plus the shared framebuffer bridge | Existing byte arithmetic, arrays and aliases; proposed shared-array native bridge; external firmware and game images |
| [Doom port feasibility](DOOM_PORT_FEASIBILITY.md) | Actionable investigation | Assess a Skald engine port with a small native platform library, including scope, language gaps, and division-helper evidence | Settle a minimal shared-array handle bridge and numerical helpers, then build the host/framebuffer spike | Existing arrays, I/O, primitive interop; optional 32-bit integer support |
| [Codebase cleanup audit](CODEBASE_CLEANUP_AUDIT.md) | Actionable audit | Rank repository-wide robustness, ownership, maintainability, and efficiency improvements | Review delivered frontend contracts through the retrospective before dependent cleanup; independent bounded fixes may proceed | Dependencies are recorded per finding; frontend acceptance is tracked in the retrospective roadmap |
| [Optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) | Living inventory | Track implemented and plausible optimizations, placement, effort, value, prerequisites, and risks | Use measurements and architecture evidence to promote a candidate into a design or roadmap | [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) and candidate-specific measurements or contracts |
| [Optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md) | Three constraints remain unplanned | Record architectural limits on modular target-independent and target-specific optimization and their sequencing | Design a remaining constraint only when a measured optimization need justifies its prerequisite work | Existing pipeline, reachability, normalization, and MIR identity foundations |

The status matrix and focused language documents own feature maturity and open
language questions. The optimization catalog owns candidate lifecycle status.
The archive preserves completed decisions and delivery history without
duplicating them here.
