# Shared Ownership Verifier Responsibility Split Roadmap

Status: completed 2026-07-31.

This roadmap resolves the final short-circuit maintainability discovery by
separating shared-ownership propagation, transitions, use validation, and
state behind the existing shared-verifier facade. It changes internal code
ownership only and preserves every MIR and diagnostic contract.

## Scope and invariants

- Preserve `verify_shared_ownership`, verifier ordering, exact diagnostics,
  join suppression, path-condition behavior, and disconnected-CFG handling.
- Preserve allocation, publication, adoption, owner provenance, transfer,
  static backing, checked-view, field, return, and full-expression semantics.
- Keep `SharedState` and allocation state private to shared verification.
- Give CFG propagation, owner/allocation transitions, and shared-place/use
  validation cohesive private owners.
- Do not change HIR or MIR shape, lowering, optional initialization, arrays,
  backend behavior, runtime ABI, or source semantics.

## Progress

- [x] SV0 — Split shared verification by state responsibility
- [x] SV1 — Validate contracts, document ownership, and close

## PR-sized implementation sequence

### SV0 — Split shared verification by state responsibility

**Purpose:** Replace the monolithic ownership analysis with a concise facade
and private modules aligned with its independent responsibilities.

- [x] Move fixed-point CFG propagation, successor selection, condition ending,
      and join diagnostics into one private owner.
- [x] Move allocation, owner, field, cast, call, return, and full-expression
      transitions into one private owner.
- [x] Move shared-pointee, provenance, checked-view, and static-owner use
      validation into one private owner.
- [x] Encapsulate shared state construction, storage reset, and compatible join
      comparison in one private owner.
- [x] Retain the existing verifier entry path with no visibility wider than
      shared ownership requires.

**Tests:** Formatting, Clippy, focused shared-owner, cast/view, field, call,
conditional-lifetime, malformed-MIR, and determinism tests.

**Exit criteria:** The facade contains only coordination and shared context,
each algorithm has one clear private owner, and focused behavior is unchanged.

### SV1 — Validate contracts, document ownership, and close

**Purpose:** Prove the structural refactor is behavior-neutral and retire the
now-empty discovery record.

- [x] Document the shared-verifier ownership boundary in compiler and testing
      guidance.
- [x] Remove the resolved discovery document and its active index entry.
- [x] Run workspace tests, ordinary repository gates, MSRV, extended
      robustness, diff hygiene, documentation checks, and an artifact-free
      final repository check.
- [x] Archive this roadmap and update the active and archive indexes.

**Tests:** `cargo test --locked --workspace`, `make check`,
`make msrv-check`, `make robustness-long`, `git diff --check`, documentation
link validation, and source-only `make check`.

**Exit criteria:** Existing behavior and deterministic diagnostics pass, living
documentation matches the new private ownership, no pending discovery remains,
and the roadmap is archived.

## Ordering and dependencies

The state and context remain private while propagation, transition, and use
methods move into sibling modules, avoiding an interim public API. Broad
validation follows the complete extraction. Equivalent path-state compaction
and the completed optional-initialization split are the implementation
baseline.
