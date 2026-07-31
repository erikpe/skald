# Equivalent Path-State Compaction Roadmap

Status: completed 2026-07-31.

This roadmap compacts equivalent MIR verifier alternatives without weakening
the path-condition, ownership, cleanup, loop-epoch, or malformed-MIR
contracts. It resolves the path-state performance item recorded after
short-circuit boolean implementation.

## Scope and invariants

- Preserve the distinction between a condition selected as either value and a
  condition that has not been selected in the current epoch.
- Preserve exact state conflicts, parent-active requirements, condition
  selection, condition ending, deterministic diagnostics, and disconnected
  CFG handling.
- Compact alternatives only when their verifier resource states are equal.
- Handle later loop or malformed-CFG input that updates a subset of an already
  compacted predicate without expanding unrelated dimensions.
- Keep the representation private to MIR verification. Do not change HIR,
  MIR, backend, runtime ABI, panic behavior, or source semantics.
- Do not raise the public logical-expression depth limit in this roadmap; that
  remains a separately reviewed language and compiler decision.

## Progress

- [x] PC0 — Implement compact predicate alternatives
- [x] PC1 — Integrate, stress, document, and close

## PR-sized implementation sequence

### PC0 — Implement compact predicate alternatives

**Purpose:** Replace one-state-per-concrete-predicate storage with a canonical
representation that merges complementary predicates carrying equal verifier
state while retaining exact selected-versus-missing information.

- [x] Introduce a private three-valued selected-state representation for
      active, inactive, or either selected value.
- [x] Implement deterministic cube intersection, subtraction, overlay, sibling
      compaction, selection, and condition ending.
- [x] Ensure overlapping loop updates split only the affected predicate subset
      and still apply each verifier domain's conflict merge.
- [x] Centralize state mutation and compact newly equivalent alternatives at
      deterministic dataflow merge boundaries.
- [x] Add direct unit tests for large equivalent joins, genuinely distinct
      states, selected-versus-missing behavior, condition ending, multi-axis
      compaction, and overlapping subset updates.

**Tests:** Focused `mir::verify::path_state` tests, existing path-condition and
logical verifier mutation suites, formatting, and Clippy.

**Exit criteria:** The shared path-state abstraction stores a bounded compact
predicate for equivalent truth assignments, preserves distinct resource
states and diagnostics, and safely merges a later update over a compacted
subset.

### PC1 — Integrate, stress, document, and close

**Purpose:** Prove every verifier consumer retains its contract and leave the
performance boundary and follow-up decisions accurately documented.

- [x] Migrate lifetime, object-cleanup, optional-initialization,
      shared-ownership, and array-ownership transfers to the compaction-aware
      mutation boundary.
- [x] Extend stress coverage beyond the source syntax cap at the private
      path-state boundary and retain source-level deterministic budget tests.
- [x] Confirm exact verifier diagnostics and deterministic phase products are
      unchanged for existing valid and malformed programs.
- [x] Update compiler and testing documentation to describe equivalent-state
      compaction without exposing private Rust layout.
- [x] Remove the resolved discovery item while retaining the two unresolved
      verifier-module split items and their active index entry.
- [x] Run `cargo test --locked --workspace`, `make check`, `make msrv-check`,
      `make robustness-long`, `git diff --check`, and an artifact-free final
      `make check`; then archive this roadmap and update both indexes.

**Tests:** All MIR verifier and short-circuit suites, process determinism,
goldens, documentation links, workspace tests, ordinary repository gates,
MSRV, extended robustness, and artifact-free validation.

**Exit criteria:** Equivalent path alternatives compact deterministically
across every verifier domain, all existing behavior and diagnostics pass, the
resolved discovery is removed, and the completed roadmap is archived.

## Ordering and dependencies

The predicate algebra lands before consumer migration so its invariants are
testable in isolation. Consumer integration then proves that state mutation
cannot bypass normalization. The completed short-circuit boolean roadmap and
its verified path-condition representation are the implementation baseline.
Raising the source logical-depth limit is deliberately excluded because
effectful alternatives may carry genuinely different resource states even
after equivalent-state compaction.
