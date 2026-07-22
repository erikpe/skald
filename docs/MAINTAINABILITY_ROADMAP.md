# Compiler Maintainability Roadmap

Status: planned; CQ0 is next.

This roadmap turns the 2026 code-quality audit into small, reviewable cleanup
pull requests. Its purpose is to reduce the cost and risk of future language
work through clearer responsibilities, smaller implementation units, explicit
contracts, and maintainable tests.

CQ0-CQ16 should complete before substantial polymorphism implementation begins;
design work that does not touch the compiler can proceed independently. Tasks
remain ordered to avoid moving the same code repeatedly. Completed tasks should
be checked here; when the roadmap is complete, move the whole document to
`docs/archive/` and remove it from the active-plan index.

## 1. Scope and constraints

The cleanup includes:

- reproducible local quality gates that external automation can invoke;
- an enforced compiler/runtime ABI contract;
- responsibility-oriented decomposition of MIR verification, resolution, type
  checking, and MIR lowering;
- an explicit policy for the compiler crate's public surface;
- smaller resolved, HIR, and MIR model modules behind stable facades;
- shared internal utilities where the same invariant is already duplicated;
- smaller test suites and less hand-built MIR boilerplate;
- generative robustness testing at hostile-input boundaries;
- concise living documentation that describes current behavior without
  historical milestone vocabulary.

The cleanup excludes:

- new language syntax or semantics;
- changes to specified diagnostics, dump formats, generated assembly, runtime
  output, or process status, except for the explicit runtime ABI guard;
- merging phase-specific AST, resolved IR, HIR, or MIR models;
- a general compiler framework, arena, query system, or one-file-per-type
  layout;
- removing verification from any current trust boundary;
- new production dependencies without a separate demonstrated need;
- repository-owned CI configuration; external infrastructure already runs
  `make check` regularly from clean checkouts;
- edits to completed roadmaps other than moving this roadmap into the archive
  when all tasks are complete.

Every task must:

1. preserve public Rust paths unless the task explicitly changes API policy;
2. use private submodules and explicit facade re-exports;
3. split by responsibility rather than line count;
4. keep phase-boundary matches exhaustive;
5. update living documentation in the same PR when a contract or layout moves;
6. pass `make check` and `git diff --check` before completion.

Each CQ item is intended to fit one PR. If review reveals that an item is too
large, split it along its listed responsibilities and retain the same final
acceptance criteria; do not combine adjacent CQ items merely to reduce PR
count.

## 2. Progress summary

- [ ] CQ0 — Declare and expose the supported toolchain checks
- [ ] CQ1 — Enforce the runtime ABI at link time
- [ ] CQ2 — Establish the MIR verifier facade and shared foundations
- [ ] CQ3 — Extract MIR program and declaration verification
- [ ] CQ4 — Extract MIR callable-body and instruction verification
- [ ] CQ5 — Extract MIR call, argument, place, and cleanup verification
- [ ] CQ6 — Decompose class declaration collection in resolution
- [ ] CQ7 — Decompose callable statement checking
- [ ] CQ8 — Decompose expression and initialization checking
- [ ] CQ9 — Decompose HIR-to-MIR body lowering
- [ ] CQ10 — Define the compiler library API policy
- [ ] CQ11 — Complete shared typed ID-table storage
- [ ] CQ12 — Split the resolved IR model by responsibility
- [ ] CQ13 — Split the HIR model by responsibility
- [ ] CQ14 — Split the MIR model by responsibility
- [ ] CQ15 — Introduce a small test-only MIR fixture vocabulary
- [ ] CQ16 — Migrate and divide oversized test suites
- [ ] CQ17 — Add generative frontend and MIR robustness tests
- [ ] CQ18 — Align living documentation and close the cleanup

## 3. PR-sized tasks

### CQ0 — Declare and expose the supported toolchain checks

**Purpose:** Make the existing quality standard reproducible locally and by the
external infrastructure, while preventing accidental increases to the minimum
Rust version.

- [ ] Determine and declare the minimum supported Rust version with
      `workspace.package.rust-version`.
- [ ] Add a repository toolchain file selecting stable Rust with the rustfmt
      and Clippy components; keep the MSRV declaration authoritative.
- [ ] Add a locally runnable `make msrv-check` target that compiles the complete
      workspace and all Rust test targets with the declared MSRV; it must not
      install toolchains or mutate contributor configuration.
- [ ] Keep `make check` as the complete current-stable Rust, golden, and C
      runtime gate used by contributors and external infrastructure.
- [ ] Document the one-time local prerequisite for installing the declared
      MSRV toolchain.
- [ ] Document that scheduling, clean-checkout execution, and failure reporting
      belong to existing external infrastructure rather than this repository.
- [ ] Document the supported Rust, C compiler, archiver, and platform contract
      in the README.

**Acceptance criteria:** Contributors can run `make check` and
`make msrv-check` locally, external infrastructure can invoke the same targets
from clean checkouts, and the repository contains no CI job configuration.

### CQ1 — Enforce the runtime ABI at link time

**Purpose:** Make a stale or incompatible runtime archive fail deterministically
instead of relying on a version value that the compiler never checks.

- [ ] Add a version-specific runtime marker symbol for the current ABI.
- [ ] Make every generated executable reference that marker without changing
      Skald source semantics or successful program output.
- [ ] Keep `ska_rt_abi_version` only if runtime inspection remains useful.
- [ ] Add a driver/toolchain test using an archive with the marker missing or
      deliberately mismatched.
- [ ] Update the runtime ABI and repository-architecture documentation with the
      exact compatibility mechanism.

**Acceptance criteria:** The current runtime links normally, an incompatible
archive fails with a structured toolchain error, and the emitted-assembly
change is limited to the marker reference while source diagnostics and runtime
observations remain unchanged.

### CQ2 — Establish the MIR verifier facade and shared foundations

**Purpose:** Prepare the verifier for incremental extraction while removing
duplicated low-level invariants.

- [ ] Turn `mir::verify` into a concise facade owning the public verification
      entry point and error types.
- [ ] Introduce one private verifier context and one ordered error sink shared
      by all verifier submodules.
- [ ] Move MIR place ancestry and overlap predicates to one private owner used
      by structural and cleanup verification.
- [ ] Move source-identifier validation to a neutral lexical-policy utility
      shared by the lexer and verifier without making MIR depend on lexer
      implementation state.
- [ ] Add focused unit tests for the shared predicates before deleting their
      duplicate implementations.

**Acceptance criteria:** Public verifier paths and error rendering are
unchanged, duplicate predicates are gone, and the facade makes the planned
verification responsibilities visible.

### CQ3 — Extract MIR program and declaration verification

**Purpose:** Give program-wide metadata and declaration invariants a dedicated
implementation home.

- [ ] Extract entry-point, function declaration, definition-slot, and linkage
      checks from the central verifier.
- [ ] Extract class, field, initializer, method, destructor, copy-capability,
      and destruction-plan metadata checks.
- [ ] Keep table traversal and error emission in the existing deterministic
      order.
- [ ] Keep declaration verification independent of callable-body control flow.
- [ ] Move or split corruption tests so they sit with the responsibility they
      exercise.

**Acceptance criteria:** Program and declaration verification can be read and
tested without entering callable-body verification, with identical accepted
MIR and error ordering.

### CQ4 — Extract MIR callable-body and instruction verification

**Purpose:** Separate callable structure and instruction dispatch from
program-wide metadata.

- [ ] Extract signature, receiver, return storage, parameter storage, local
      storage, value, block-ID, entry-block, and terminator checks.
- [ ] Replace the large `verify_block` body with a short ordered instruction
      loop delegating to responsibility-specific helpers.
- [ ] Extract assignment, store, initialization, copy, cleanup, and
      full-expression instruction checks.
- [ ] Preserve block-local transient-value rules and all unreachable-block
      structural checks.
- [ ] Keep instruction error accumulation deterministic and non-short-circuiting
      where it is today.

**Acceptance criteria:** Callable structure and ordinary instruction
verification have clear module owners, and no single verification function
mixes every instruction category.

### CQ5 — Extract MIR call, argument, place, and cleanup verification

**Purpose:** Isolate the verifier's most coupled semantic checks behind narrow
interfaces.

- [ ] Extract direct call, method call, initializer, hidden receiver, result,
      and return-destination verification.
- [ ] Extract value, alias-place, and owned-place argument checking, including
      access, ownership, overlap, and exact type rules.
- [ ] Extract place-base and projection validation with one returned verified
      place descriptor.
- [ ] Keep cleanup liveness as a distinct dataflow analysis using the shared
      place predicates from CQ2.
- [ ] Divide call/place/cleanup corruption tests by contract and retain exact
      error messages.

**Acceptance criteria:** The verifier facade coordinates small declaration,
body, instruction, call, place, and cleanup modules; no substantial algorithm
remains in the facade.

### CQ6 — Decompose class declaration collection in resolution

**Purpose:** Make fields, ordinary members, and lifecycle declarations
independently understandable without changing two-pass resolution.

- [ ] Keep one source-ordered class-member traversal as the coordination point.
- [ ] Extract field, initializer, copy constructor, copy assignment,
      destructor, and method collection helpers.
- [ ] Introduce a small class-collection state object for symbols, declarations,
      work items, and lifecycle slots.
- [ ] Preserve ID allocation, duplicate precedence, recovery, and diagnostic
      order exactly.
- [ ] Keep top-level collection separate from callable-body resolution.

**Acceptance criteria:** `collect_class` communicates the traversal at a glance,
while each member category owns its validation and output construction.

### CQ7 — Decompose callable statement checking

**Purpose:** Make statement-level type rules local and reduce the size of the
callable checker dispatch.

- [ ] Keep one exhaustive `ResolvedStatement` match in the callable checker.
- [ ] Extract local declaration, return, call statement, conditional, nested
      block, field assignment, and object assignment helpers.
- [ ] Keep structured `BlockFlow` composition in the statement/block owner.
- [ ] Preserve diagnostic accumulation after independently invalid operands or
      statements.
- [ ] Keep copy and initializer-specific policy in their existing cohesive
      submodules rather than moving it back into the dispatcher.

**Acceptance criteria:** Statement dispatch is short, every statement family
has one clear implementation owner, and HIR plus diagnostics remain unchanged.

### CQ8 — Decompose expression and initialization checking

**Purpose:** Separate primitive expression typing, calls, places, and object
initialization policy.

- [ ] Keep one exhaustive `ResolvedExpression` dispatch entry point.
- [ ] Extract binding/literal, unary/binary, direct-call, method-call, field
      read, grouping, and excluded construction-expression helpers.
- [ ] Split field assignment checking into primitive assignment, direct field
      construction, copy construction, copy assignment, and liveness
      transitions.
- [ ] Keep object-place capability checks in the existing place submodule and
      copy selection in the copy submodule.
- [ ] Preserve source evaluation order and exact diagnostic labels and notes.

**Acceptance criteria:** Expression and initialization rules can evolve without
editing one broad matcher, while exhaustive dispatch still exposes new syntax
variants at compile time.

### CQ9 — Decompose HIR-to-MIR body lowering

**Purpose:** Make statement, expression, control-flow, object-value, and cleanup
lowering independently readable.

- [ ] Keep `BodyLowerer` as the shared state owner for storage, values, blocks,
      and cleanup state.
- [ ] Replace the large block statement match with helpers for locals, returns,
      calls, assignments, construction, copying, nested blocks, and
      conditionals.
- [ ] Split scalar expression lowering from call lowering and object-value
      materialization.
- [ ] Preserve left-to-right evaluation, block allocation order, value IDs,
      cleanup order, and exact MIR dumps.
- [ ] Keep `lower.rs` or `lower/mod.rs` as a concise facade over private
      responsibility modules.

**Acceptance criteria:** MIR output remains byte-identical and lowering control
flow is visible without reading object-value or cleanup implementation details.

### CQ10 — Define the compiler library API policy

**Purpose:** Let internal representations evolve without implying an accidental
stable external API.

- [ ] Define `skald-compiler` as an unpublished internal compiler crate and set
      `publish = false` in its manifest.
- [ ] Document phase products and dumps as intentionally usable by repository
      tools and integration tests but unstable across compiler revisions.
- [ ] Retain public entry points required by `skac`, integration tests, and
      intended debugging tools; narrow implementation-only modules, helpers,
      and re-exports.
- [ ] Document that mutating exposed phase products can violate trusted
      producer invariants; keep unconditional MIR verification at every
      current trust boundary.
- [ ] Audit every `pub mod`, re-export, public field, and test-only mutation
      hook against the selected policy.
- [ ] Add compile-time API tests only for paths intentionally promised by the
      policy.

**Acceptance criteria:** Maintainers can distinguish stable interface from
trusted implementation data, and future internal moves do not widen or break
the API accidentally.

### CQ11 — Complete shared typed ID-table storage

**Purpose:** Give dense class-indexed tables the same single invariant owner as
function-indexed tables without erasing phase ownership.

- [ ] Generalize the private dense table utility over the typed ID/index
      operation actually shared by functions and classes.
- [ ] Migrate resolved, HIR, and MIR class declaration and definition wrappers.
- [ ] Preserve phase-specific public wrapper types, deterministic iteration,
      lookup validation, and test-only mutation boundaries.
- [ ] Avoid a general arena, dynamic registry, or public table abstraction.
- [ ] Remove repeated dense-ID assertions only after equivalent shared tests
      exist.

**Acceptance criteria:** Dense class/function storage has one implementation,
while callers still see phase-specific tables and unchanged public paths.

### CQ12 — Split the resolved IR model by responsibility

**Purpose:** Make the resolved representation navigable before polymorphism
adds more declarations and selected identities.

- [ ] Divide declarations/tables, callable bodies/statements, expressions, and
      object places into cohesive private modules.
- [ ] Keep `resolve/mod.rs` as the documented public facade with explicit
      re-exports.
- [ ] Preserve type names, public paths, derives, field order, and dump output.
- [ ] Keep source-name-bearing resolved data separate from HIR semantic types.
- [ ] Update architecture documentation only with stable module
      responsibilities, not a transient file inventory.

**Acceptance criteria:** No resolved model file mixes the complete declaration,
statement, and expression vocabularies, and downstream code compiles without
path churn.

### CQ13 — Split the HIR model by responsibility

**Purpose:** Give typed declarations, callable bodies, expressions, and object
ownership operations clear homes.

- [ ] Divide declaration/tables, body/control-flow, scalar expressions/calls,
      and object construction/copy/place types into cohesive private modules.
- [ ] Keep phase-owned types distinct even when their shapes resemble resolved
      IR or MIR.
- [ ] Preserve the `hir` facade, exact dump vocabulary, and exhaustive
      downstream matches.
- [ ] Keep small central types such as `Type`, `BlockFlow`, or `HirAccess` at
      the narrowest sensible shared boundary.

**Acceptance criteria:** HIR ownership and typing concepts are easy to locate,
with no public path or semantic change.

### CQ14 — Split the MIR model by responsibility

**Purpose:** Make MIR declarations, executable bodies, places, instructions,
and control flow independently navigable.

- [ ] Divide declarations/tables, callable definitions/storage, values/places,
      instructions/calls, and blocks/terminators into cohesive private modules.
- [ ] Keep `mir/mod.rs` as the explicit public facade consumed by passes and
      backends.
- [ ] Preserve all derives, constructors, identity ownership, iteration order,
      and public paths.
- [ ] Keep verifier and backend imports selective; do not replace them with
      wildcard re-exports inside implementation modules.

**Acceptance criteria:** The MIR schema is organized by responsibility without
changing its representation or making tiny one-type files.

### CQ15 — Introduce a small test-only MIR fixture vocabulary

**Purpose:** Remove repeated storage, value, body, and callable boilerplate
from verifier and backend tests while keeping semantically important details
visible.

- [ ] Inventory repeated hand-built MIR helpers across MIR and x86-64 tests.
- [ ] Add test-only builders for common one-block bodies, declarations,
      receivers, parameters, storage, values, and instructions.
- [ ] Require explicit callable IDs, types, ownership modes, and spans where
      they affect the tested contract.
- [ ] Do not expose the fixture vocabulary in production or turn it into a
      second MIR construction API.
- [ ] Prove that representative fixture dumps are unchanged before broad
      migration.

**Acceptance criteria:** Tests can express the invariant under examination
without hundreds of lines of unrelated setup, and malformed MIR remains easy
to construct deliberately.

### CQ16 — Migrate and divide oversized test suites

**Purpose:** Make test ownership and failures easier to locate without reducing
coverage.

- [ ] Migrate backend object, alias, and scalar fixtures to the CQ15 vocabulary
      one fixture family at a time.
- [ ] Move the large MIR alias fixture out of the assertion-focused test file.
- [ ] Split type-checker object tests into construction, lifecycle/copy,
      receiver/access, object-place, and dump responsibilities.
- [ ] Split resolver object tests into declarations/lifecycle, member lookup,
      object places, diagnostics, and dumps.
- [ ] Split driver tests into CLI, pipeline, artifact, and toolchain behavior.
- [ ] Keep public-API integration tests under `crates/skald-compiler/tests/` and
      phase tests beside their owning implementation.

**Acceptance criteria:** No large test module mixes unrelated compiler
responsibilities, fixtures have one owner, and the test count and coverage are
preserved or improved.

### CQ17 — Add generative frontend and MIR robustness tests

**Purpose:** Exercise combinatorial malformed input without adding production
complexity or dependencies.

- [ ] Add a fuzz or property-test target for arbitrary byte/UTF-8 input through
      source creation, lexing, parsing, and diagnostic rendering.
- [ ] Assert termination within the existing nesting/resource limits and no
      panics for hostile source input.
- [ ] Add structured mutations of valid MIR covering identities, types,
      ownership, calls, places, CFG edges, and cleanup state.
- [ ] Assert that invalid MIR yields structured verification errors and never
      reaches backend panics.
- [ ] Keep corpora and dependencies outside production crates and document fast
      local smoke commands separately from longer scheduled runs.

**Acceptance criteria:** A Makefile target runs a bounded deterministic smoke
corpus locally and under external automation, longer generative runs are
documented, and every discovered regression is retained as a focused permanent
test.

### CQ18 — Align living documentation and close the cleanup

**Purpose:** Leave current documentation concise, internally consistent, and
free of historical implementation shorthand.

- [ ] Replace milestone codes such as OBJ, DD, IOF, and OVS in active test names,
      comments, grammar notes, and architecture documentation with semantic
      descriptions.
- [ ] Keep milestone vocabulary unchanged in archived roadmaps.
- [ ] Resolve the competing `tests/compiler/` and crate integration-test
      guidance; remove an empty reserved location if it is no longer canonical.
- [ ] Verify that module, API, toolchain, runtime ABI, test, and debugging
      documentation describes the completed implementation.
- [ ] Run the complete quality gate from a clean checkout and review remaining
      large files/functions for cohesive responsibility rather than size alone.
- [ ] Mark CQ0-CQ18 complete, move this roadmap to `docs/archive/`, update the
      archive index, and remove it from active plans.

**Acceptance criteria:** Living documentation describes only current behavior
and planned direction, archived roadmaps remain historical, and the final
audit finds no unresolved high-priority maintainability hotspot.

## 4. Review order and parallel work

The default sequence is CQ0 through CQ18. Limited parallel work is safe only
where files do not overlap:

- CQ0 and CQ1 may proceed independently.
- CQ2-CQ5 are sequential because they reshape one verifier.
- CQ6-CQ9 should remain sequential unless ownership of resolver, type checker,
  and MIR lowering is clearly separated between contributors.
- CQ10 and CQ11 must precede CQ12-CQ14 so model moves implement the settled API
  and table policies once.
- CQ15 precedes CQ16; CQ17 should begin only after verifier and test-fixture
  interfaces stabilize.
- CQ18 is the final closeout task, though every earlier PR must keep affected
  living documentation current.

Do not combine cleanup tasks with polymorphism feature work. If feature work
must touch a hotspot before its cleanup task, complete the relevant CQ task
first or keep the feature PR narrowly responsible for restoring the same
module boundary.
