# Compiler Implementation Cleanup Roadmap

Status: in progress; R0–R11 are complete and R12 is the next implementation task.

This roadmap turns the post-T7 implementation audit into small, reviewable
refactoring tasks. Its goal is to make future language work easier to add and
safer to review without changing Skald's specified behavior, generated-code
semantics, public runtime ABI, or supported target.

The tasks prefer explicit, narrow abstractions over framework building. Every
task must leave the repository buildable and pass the complete local quality
gate before its checkbox is marked complete.

## 1. Design Constraints

### Included

- safer compiler artifact publication;
- clearer dependency direction for stable program identities;
- removal of duplicated control-flow analysis;
- smaller, responsibility-focused parser, type-checker, backend, and test
  modules;
- bounded parsing of recursively nested syntax;
- shared typed-table and test-support utilities where repetition is already
  established;
- clearer diagnostics, implementation comments, and debugging helpers;
- preservation or improvement of current unit, runtime, CLI, and golden
  coverage.

### Explicitly excluded

- new language syntax or semantics;
- SSA, optimization, register allocation, incremental compilation, or a query
  system;
- splitting every compiler phase into a separate crate;
- merging AST, resolved IR, HIR, or MIR representations;
- removing verification at lowering, pass-pipeline, or backend trust
  boundaries;
- changing the C runtime ABI or bootstrap output formats;
- adding third-party dependencies without a concrete need;
- continuous integration. The fast local make check workflow remains the
  required gate for this roadmap.

## 2. Refactoring Principles

1. Preserve externally visible source behavior, diagnostics, assembly
   determinism, runtime output, and process status unless a task explicitly
   corrects a documented defect.
2. Separate mechanical moves from semantic changes whenever that improves
   reviewability.
3. Add common utilities only when they encode an established invariant.
4. Keep phase-specific data models separate. Shared IDs and containers must not
   erase which phase owns a semantic decision.
5. Keep phase-boundary matches exhaustive so a new enum variant identifies
   every required extension point at compile time.
6. Assert the narrowest meaningful test boundary. Reserve native execution for
   behavior that phase-level tests cannot establish.
7. Complete each task with make check and git diff --check.

## 3. Progress Summary

- [x] R0 — Make artifact publication atomic and protect source inputs
- [x] R1 — Move stable program identities to a neutral module
- [x] R2 — Consolidate dense and sparse ID-indexed tables
- [x] R3 — Compute structured return flow once
- [x] R4 — Restructure the type checker around per-function context
- [x] R5 — Split the parser by grammar responsibility
- [x] R6 — Bound recursive syntax nesting
- [x] R7 — Decompose x86-64 instruction and call lowering
- [x] R8 — Introduce shared compiler test support
- [x] R9 — Split oversized Rust test modules by behavior
- [x] R10 — Split the runtime ABI harness by responsibility
- [x] R11 — Correct diagnostics, comments, and small formatting duplication
- [ ] R12 — Complete the cleanup audit and final quality gate

Milestone checkboxes below should be marked as implementation progresses. A
task is complete only when its acceptance criteria and relevant tests pass.

## 4. PR-Sized Tasks

### R0 — Make artifact publication atomic and protect source inputs

**Purpose:** Prevent compilation from destroying source input or leaving a
partially written artifact after interruption or failure.

- [x] Reject an explicit output path that resolves to the input source file.
- [x] Extract one temporary-file-and-rename publication utility owned by the
      driver/toolchain boundary.
- [x] Use atomic publication for textual assembly as well as executables.
- [x] Preserve an existing destination when compilation, linking, writing, or
      publication fails.
- [x] Keep temporary files beside the destination and remove them through RAII.

**Tests:** Input/output alias rejection; successful assembly publication;
existing-output preservation after simulated failure; temporary cleanup; and
existing executable-link tests.

**Acceptance criteria:** A successful compile publishes exactly one complete
artifact, a failed compile does not damage an existing artifact, and no valid
CLI invocation can overwrite its source file.

### R1 — Move stable program identities to a neutral module

**Purpose:** Remove the conceptual dependency from HIR, MIR, and backend code
to the resolver implementation.

- [x] Create a neutral module for FunctionId, ParameterId, LocalId, and
      BindingId.
- [x] Move construction, ownership checks, ordering, and display formatting
      without changing observable dump spelling.
- [x] Update AST-independent phases to import IDs from the neutral module.
- [x] Keep resolution responsible for assigning identities, not owning their
      type definitions.
- [x] Update architecture documentation with the corrected dependency
      direction.

**Tests:** Existing exact resolved/HIR/MIR dumps, owner-mismatch verifier tests,
and the complete compiler suite.

**Acceptance criteria:** Later phases no longer import stable IDs from resolve,
all dump output remains byte-identical, and resolution remains the only phase
that selects source names.

### R2 — Consolidate dense and sparse ID-indexed tables

**Purpose:** Replace repeated declaration/definition table bookkeeping with a
small utility that enforces the same invariant consistently in every IR.

- [x] Introduce narrowly scoped typed dense and sparse ID-indexed containers.
- [x] Preserve dense-ID validation, optional definition slots, deterministic
      iteration, and exact-size iteration where currently promised.
- [x] Migrate resolved, HIR, and MIR declaration and definition tables without
      exposing raw vectors.
- [x] Keep phase-specific declaration and definition structures separate.
- [x] Avoid a general arena, interning framework, or trait hierarchy beyond
      what the existing tables require.

**Tests:** Container invariant tests; missing, foreign, and non-dense ID cases;
existing table lookup tests; and exact dumps.

**Acceptance criteria:** Table behavior and public iteration remain unchanged,
while dense/sparse indexing and count maintenance have one implementation.

### R3 — Compute structured return flow once

**Purpose:** Eliminate duplicate definite-return algorithms before more
control-flow constructs arrive.

- [x] Define a small structured flow result such as FallsThrough and Terminates.
- [x] Compute it while checking typed blocks and conditionals.
- [x] Use the same result for missing-return diagnostics and MIR join decisions.
- [x] Remove both recursive block-guarantees-return implementations.
- [x] Document how loops, divergence, and exceptions can extend the result
      without duplicating analysis.

**Tests:** Exhaustive and non-exhaustive conditionals; nested blocks; code after
returns; conditional joins; absence of unreachable MIR blocks; and exact
missing-return diagnostics.

**Acceptance criteria:** Return behavior and MIR dumps remain unchanged, and
there is one authoritative structured-flow computation.

### R4 — Restructure the type checker around per-function context

**Purpose:** Reduce argument plumbing and give declarations, statements,
expressions, literals, and diagnostics clear implementation homes.

- [x] Introduce a FunctionChecker holding the current program, declaration,
      definition, return type, and diagnostic sink.
- [x] Move statement, conditional, expression, call, binding, and literal
      checking into focused modules or implementation sections.
- [x] Keep program-level entry and external-declaration validation separate.
- [x] Preserve all-or-nothing HIR construction and diagnostic accumulation.
- [x] Avoid changing type rules or diagnostic codes during the structural
      refactor.

**Tests:** Existing type-checker unit and golden tests, exact HIR dumps, and a
focused test proving independent errors still accumulate.

**Acceptance criteria:** No central type-checking function mixes unrelated
program, statement, expression, and literal responsibilities, and recursive
calls no longer thread the same context parameters manually.

### R5 — Split the parser by grammar responsibility

**Purpose:** Make grammar changes local without replacing the recovering
recursive-descent design.

- [x] Retain one parser state object while moving implementation blocks into
      declaration, statement, expression, and recovery modules.
- [x] Centralize token-to-TypeKind conversion.
- [x] Express unit acceptance as a caller-visible type-context decision rather
      than duplicating every primitive match.
- [x] Preserve synchronization points, diagnostics, AST spans, and source
      spellings exactly.
- [x] Update parser-facing documentation when module ownership changes.

**Tests:** Exact AST dumps; every parser recovery golden; parameter, local, and
result type parsing; operator precedence; calls; and conditionals.

**Acceptance criteria:** Parser behavior remains stable, while declarations,
statements, expressions, and recovery can be changed without editing one
near-thousand-line file.

### R6 — Bound recursive syntax nesting

**Purpose:** Turn pathological nesting into a structured source diagnostic
instead of process stack exhaustion.

- [x] Define and document a generous implementation nesting limit.
- [x] Track recursive entry for grouped and unary expressions, postfix calls,
      and nested blocks through one guard mechanism.
- [x] Emit one focused diagnostic and recover without cascades.
- [x] Ensure rejected depth never reaches recursive downstream traversals.
- [x] Keep ordinary source programs allocation- and branch-light.

**Tests:** Inputs immediately below, at, and above the limit for expressions and
blocks; recovery to later declarations; and an end-to-end no-panic test.

**Acceptance criteria:** Arbitrarily nested source cannot overflow the compiler
stack, practical nesting is unaffected, and excessive nesting produces a
stable diagnostic.

### R7 — Decompose x86-64 instruction and call lowering

**Purpose:** Make new MIR operations and ABI types straightforward to add
without enlarging one instruction-selection function.

- [x] Separate assignment/rvalue selection from instruction dispatch.
- [x] Extract integer and floating unary/binary operation selection.
- [x] Isolate loads, stores, canonicalization, calls, and terminators behind
      focused helpers.
- [x] Preserve the ABI, frame, legality, machine-model, and emission boundaries.
- [x] Keep exhaustive MIR operation matches and structured backend errors.

**Tests:** Assembly-shape tests for every primitive operation; mixed
register/stack calls; u8 canonicalization; boolean results; native floating
execution; and assembler acceptance.

**Acceptance criteria:** Assembly remains deterministic and semantically
identical, and each MIR operation has one obvious instruction-selection home.

### R8 — Introduce shared compiler test support

**Purpose:** Remove repeated phase-pipeline and temporary-resource helpers while
keeping tests explicit about the boundary they exercise.

- [x] Add test-only helpers for lexing, parsing, resolution, type checking, MIR
      lowering, and assembly generation.
- [x] Make each helper assert success only for phases preceding the requested
      test boundary.
- [x] Add RAII temporary directory and file helpers shared where crate
      boundaries allow it.
- [x] Remove counter-based helpers that leak resources after failed assertions.
- [x] Keep production builds free of test-support code.

**Tests:** Cleanup-on-drop and unique-path tests, followed by the complete
existing suite.

**Acceptance criteria:** Repeated setup and manual temporary cleanup are
removed, failed tests do not leave predictable artifacts, and individual tests
remain readable without hidden semantic setup.

### R9 — Split oversized Rust test modules by behavior

**Purpose:** Keep growing coverage navigable and give new features an obvious
test location.

- [x] Split MIR tests into builder, lowering, control-flow, verification, and
      dump modules.
- [x] Split x86-64 tests into ABI, instruction selection, calls, control flow,
      legality, assembler, and native-execution modules.
- [x] Split type-checker tests into declarations, expressions, literals,
      control flow, diagnostics, and dumps.
- [x] Split syntax and resolution tests where their size benefits similarly.
- [x] Preserve private-module access and avoid moving phase-unit tests into
      end-to-end goldens.

**Tests:** This task is mechanical; test names, assertions, and total test count
must remain stable, followed by make check.

**Acceptance criteria:** No phase has a catch-all test file approaching a
thousand lines, and each test has a clear behavior-oriented home.

### R10 — Split the runtime ABI harness by responsibility

**Purpose:** Isolate runtime contract, successful formatting, and fatal-error
behavior while retaining direct C ABI coverage.

- [x] Split ABI/version and platform assertions from output-record tests.
- [x] Separate successful stdout capture from child-process failure tests.
- [x] Share only small C helpers whose invariants are genuinely common.
- [x] Update the runtime Makefile to build and run every harness
      deterministically.
- [x] Keep the runtime library unchanged unless extraction exposes a defect.

**Tests:** All runtime harnesses under C11 with -Wall -Wextra -Werror, plus make
runtime-test and the golden suite.

**Acceptance criteria:** Each runtime binary has one clear purpose, every ABI
and output behavior remains covered, and failures identify the responsible
harness directly.

### R11 — Correct diagnostics, comments, and small formatting duplication

**Purpose:** Remove obsolete milestone terminology and make messages describe
current language rules accurately.

- [x] Replace production comments and diagnostics that present M1/M3/C4/T1 or
      the first vertical slice as the current language boundary.
- [x] State in unary-negation diagnostics that i64 and f64 are accepted.
- [x] Describe binary arithmetic as requiring equal numeric operand types.
- [x] Centralize supported-type list rendering where diagnostics repeat it.
- [x] Add a tiny shared dump-format helper only for identical quoting, escaping,
      indentation, or span behavior still duplicated after module splits.
- [x] Preserve historical milestone wording in roadmap and history documents.

**Tests:** Exact affected diagnostic goldens; dump snapshots; searches for
obsolete production terminology; and the complete suite.

**Acceptance criteria:** Errors describe current rules, comments do not become
stale when a roadmap finishes, and shared formatting exists only where it
improves clarity.

### R12 — Complete the cleanup audit and final quality gate

**Purpose:** Verify that cleanup achieved its goals without changing the
language or leaving transitional scaffolding.

- [ ] Re-run file-size and dependency-direction audits.
- [ ] Confirm HIR, MIR, and backend no longer import identities from resolve.
- [ ] Confirm return-flow logic has one implementation.
- [ ] Confirm production and test modules follow documented ownership.
- [ ] Confirm temporary-resource and artifact-publication failure coverage.
- [ ] Remove compatibility helpers introduced only during migration.
- [ ] Update README.md, docs/REPO_STRUCTURE.md, docs/DEBUGGING.md, and test
      documentation with the final organization.
- [ ] Run complete local quality gates from a clean build state.

**Tests:** Clean make check, explicit make golden-test, explicit make
runtime-test, and git diff --check.

**Acceptance criteria:** All R0–R12 checkboxes are complete, deterministic
behavior remains covered, audited dependency and duplication problems are
resolved, and the repository is ready for the next language slice.

## 5. Required Quality Gates

Run these for every task that touches the corresponding area. R12 runs and
records all of them from a clean build state.

- [ ] cargo fmt --all -- --check
- [ ] cargo check --workspace --all-targets
- [ ] cargo clippy --workspace --all-targets -- -D warnings
- [ ] cargo test --workspace
- [ ] make runtime-test when runtime code, headers, or harnesses change
- [ ] make golden-test when source behavior, diagnostics, MIR, backend, runtime
      linking, or golden expectations change
- [ ] git diff --check

The global checkboxes are marked only by R12. Earlier tasks run their relevant
commands without marking the final roadmap gate prematurely.
