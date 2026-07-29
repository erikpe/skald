# Cyclic Module Imports Roadmap

Status: planned; C0 is next.

This roadmap permits mutually dependent modules without weakening import
visibility, semantic-cycle validation, deterministic identity allocation, or
whole-program diagnostics. It then uses that support to let `std::str` call
the canonical `std::error::panic` function instead of manufacturing an array
bounds failure.

## Scope and invariants

- Two or more reachable modules may form a directed import cycle. Every module
  in that component is parsed and contributes declarations through the same
  whole-program passes as an acyclic module.
- A direct self-import remains invalid. It adds no reachability and creates a
  confusing second path to declarations already owned by the importing
  module.
- Imports remain explicit name-availability edges. Cycles do not create
  transitive bindings, re-exports, package relationships, visibility
  exceptions, or a module execution order.
- Canonical module-path order continues to determine `ModuleId`, `SourceId`,
  declaration identity, diagnostics, dumps, and emitted output. Discovery,
  import, cycle, and selected-entry order do not affect those products.
- Resolution collects all top-level declarations and public surfaces before
  resolving imports, signatures, hierarchy, or bodies. It must not use
  recursive module descent, import-order evaluation, or an unbounded semantic
  fixed point.
- Cyclic imports do not legalize inheritance cycles, recursive inline
  containment, invalid interface relationships, incompatible external ABI
  declarations, or invalid ownership state. Those remain independently
  diagnosed by their owning semantic phases.
- Explicit imports and compiler-owned string-literal dependencies follow the
  same cycle policy.
- Missing, ambiguous, unreadable, malformed, or wrong-case modules still fail
  during graph construction before semantic resolution.
- The source-text convenience API remains provider-less and does not gain
  module discovery.
- No strongly connected component identity is added to semantic IR until a
  concrete consumer needs it. The existing canonical graph and flat
  whole-program phase ordering are sufficient for this profile.
- Modules still have no executable top-level initialization. Any future
  top-level state must separately define initialization ordering or prohibit
  initialization cycles; this roadmap does neither.

## Progress

- [ ] C0 — Admit cyclic module graphs
- [ ] C1 — Prove semantic resolution across cycles
- [ ] C2 — Migrate string failures and harden the feature

## PR-sized implementation sequence

### C0 — Admit cyclic module graphs

**Purpose:** Change the source and loader contract from rejecting every cycle
to accepting multi-module cycles while preserving deterministic graph
construction and a focused self-import error.

- [ ] Update the language module contract, compiler module contract, phase
      documentation, and status matrix to permit multi-module import cycles,
      reject direct self-imports, and keep initialization ordering absent.
- [ ] Remove complete-cycle rejection from graph finalization and update
      `ModuleGraph` ownership documentation so cyclic direct edges are an
      ordinary valid graph shape.
- [ ] Replace the old cycle-chain diagnostic machinery with a direct
      self-import check and one exact diagnostic owned by graph construction.
- [ ] Preserve iterative reachable-closure loading, one parsed module instance
      per canonical path, canonical dense identities, and unchanged dependency
      evidence in graph dumps.
- [ ] Convert the existing multi-module compile-failure golden into a
      successful cyclic-import fixture without weakening missing, ambiguous,
      malformed, or case-sensitive provider failures.
- [ ] Remove obsolete cycle-finder tests and add graph tests for two-module,
      longer, selected-entry, and string-literal dependency cycles plus direct
      self-import rejection.
- [ ] Retain a bounded deep-cycle regression that proves graph loading and
      finalization do not depend on recursive stack growth.

**Tests:** Focused module graph and driver tests, the converted golden fixture,
deterministic graph-dump assertions, `make docs-check`, and
`cargo test --locked -p skald-compiler module::graph::tests`.

**Exit criteria:** Multi-module cycles produce one deterministic loaded graph,
direct self-import still fails exactly, all other provider and parsing errors
retain their phase ownership, and repository tests no longer encode general
cycle rejection.

### C1 — Prove semantic resolution across cycles

**Purpose:** Demonstrate that the existing declaration-first whole-program
resolver handles useful cyclic programs without order dependence or semantic
shortcuts.

- [ ] Add qualified and selective-import tests where mutually dependent
      modules resolve functions, parameters, results, classes, interfaces, and
      shared or alias views declared on opposite sides of a cycle.
- [ ] Exercise mutually recursive function identities without requiring a
      terminating runtime recursion, and prove ordinary cross-module calls
      lower through verified MIR and native assembly.
- [ ] Verify private, missing, wrong-kind, duplicate-binding, and non-re-export
      diagnostics remain exact and deterministic inside cyclic components.
- [ ] Verify inheritance cycles, recursive inline containment, invalid
      interface relationships, and incompatible external declarations remain
      rejected by their existing semantic owners even when their modules form
      a valid import cycle.
- [ ] Prove canonical declaration identities, resolved/HIR/MIR dumps,
      diagnostics, and assembly are stable across import spelling order,
      discovery order, and different selected entries with the same reachable
      closure.
- [ ] Refactor resolver orchestration or cyclic test fixtures only where doing
      so makes the declaration-before-use phase boundary explicit and reusable;
      do not add component-specific resolution or a fixed-point engine without
      evidence that it is required.
- [ ] Update development testing guidance with cyclic graph, semantic-cycle,
      and cross-process determinism ownership.

**Tests:** Focused resolver and type-check tests, verifier/backend coverage for
one cyclic program, pipeline determinism tests in separate compiler processes,
and relevant native and compile-failure goldens.

**Exit criteria:** Useful cross-module declaration cycles resolve, type-check,
lower, verify, and emit independently of graph traversal order, while all
non-module semantic cycles and import-access errors retain their established
diagnostics.

### C2 — Migrate string failures and harden the feature

**Purpose:** Consume cyclic imports in the canonical standard library, remove
the motivating workaround, and close stale cycle assumptions across the
repository.

- [ ] Import `panic` from `std::error` in `std/std/str.ska`, replace
      `_fail_bounds_check` calls with non-returning panic statements using the
      existing frozen bounds message, and remove the helper and unreachable
      fallback returns.
- [ ] Preserve the `std::error -> std::str` signature dependency so the
      canonical standard library itself exercises an ordinary two-module
      cycle rather than a compiler-owned exception.
- [ ] Consolidate canonical standard-library test fixtures where practical so
      tests that include the real `Str` source also provide its complete
      reachable dependency closure.
- [ ] Keep exact native stderr and unsuccessful status for string byte and
      slice bounds failures, and prove valid string operations and explicit
      source panic remain unchanged.
- [ ] Test default, replacement, and disabled standard-library configurations
      so cyclic reachability does not bypass provider selection or language
      item validation.
- [ ] Audit code, tests, samples, and living documentation for stale claims
      that all import cycles are rejected or that string bounds failures are
      manufactured through array access.
- [ ] Run the complete repository gate and supported Rust-version check from
      an artifact-free snapshot, then inspect links, status, and diff hygiene
      before closeout.

**Tests:** String resolver/MIR/backend tests, exact string-failure native
goldens, panic goldens, module replacement and no-standard-library tests,
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Canonical `Str` reports bounds failures through
`std::error::panic`, cyclic imports are documented and comprehensively tested
as ordinary module behavior, no special panic or standard-library cycle
exception exists, and the full repository gate passes.

## Ordering and dependencies

C0 changes the graph and source contract before semantic consumers depend on
cyclic input. C1 then proves the resolver's existing declaration-first phases
across representative cycle shapes and keeps unrelated semantic cycles
invalid. C2 consumes the feature in the canonical standard library, removes
the bounds-check workaround, and performs final cross-layer hardening.

The roadmap has no dependency on separate compilation, package manifests,
top-level executable state, exceptions, or runtime ABI changes.
