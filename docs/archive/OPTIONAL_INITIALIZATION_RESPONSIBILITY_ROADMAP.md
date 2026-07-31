# Optional Initialization Responsibility Split Roadmap

Status: completed 2026-07-31.

This roadmap resolves the optional-initialization maintainability discovery by
separating path-sensitive propagation, local verification, and state
transitions behind the existing optional-verifier facade. It changes internal
ownership only, leaving MIR semantics and diagnostics intact.

## Scope and invariants

- Preserve the existing `verify_optional_initialization` entry point and its
  position in whole-body MIR verification.
- Preserve exact initialization, ownership-transfer, storage-epoch,
  path-condition, loop, disconnected-CFG, and diagnostic behavior.
- Keep the definite-initialization state private to optional verification.
- Give fixed-point propagation, diagnostic checking, and recursive field/state
  transitions cohesive private module owners.
- Do not change optional source semantics, HIR or MIR shape, backend behavior,
  runtime ABI, guards, or shared-ownership verification.

## Progress

- [x] OI0 — Split optional initialization by responsibility
- [x] OI1 — Validate contracts, document ownership, and close

## PR-sized implementation sequence

### OI0 — Split optional initialization by responsibility

**Purpose:** Replace the monolithic verifier implementation with a concise
facade and private modules whose dependencies follow the analysis phases.

- [x] Move fixed-point CFG propagation and path-condition convergence into one
      private owner.
- [x] Move instruction-local and terminator-local diagnostics into one private
      owner.
- [x] Encapsulate definite-initialization state, storage epochs, ownership
      transfer, entry seeding, and recursive optional-field initialization in
      one private owner.
- [x] Keep module visibility narrow and retain the existing verifier facade and
      call path.

**Tests:** Formatting, Clippy, focused optional and logical MIR verifier tests,
including exact malformed-MIR diagnostics.

**Exit criteria:** No substantial verifier algorithm remains in the facade,
each responsibility has one clear owner, and focused behavior is unchanged.

### OI1 — Validate contracts, document ownership, and close

**Purpose:** Confirm the structural change is behavior-neutral and leave living
documentation and discovery indexes accurate.

- [x] Document the optional-initialization verifier ownership boundary in the
      compiler and testing guides.
- [x] Remove the resolved discovery while retaining and indexing the
      shared-ownership follow-up.
- [x] Run workspace tests, ordinary repository gates, MSRV, extended
      robustness, diff hygiene, documentation checks, and an artifact-free
      final repository check.
- [x] Archive this roadmap and update the active and archive indexes.

**Tests:** `cargo test --locked --workspace`, `make check`,
`make msrv-check`, `make robustness-long`, `git diff --check`, documentation
link validation, and source-only `make check`.

**Exit criteria:** All behavior and deterministic diagnostics pass, living
documentation matches the new module ownership, the discovery is resolved,
and the completed roadmap is archived.

## Ordering and dependencies

The private state boundary is established while extracting the propagation and
checking owners so no temporary public API is needed. Contract validation and
documentation follow the complete split. Equivalent path-state compaction is
the implementation baseline; the independent shared-ownership module split
remains deferred.
