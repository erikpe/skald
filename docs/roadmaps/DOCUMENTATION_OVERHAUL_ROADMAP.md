# Documentation Overhaul Roadmap

Status: planned; DOC14 is next.

This roadmap replaces Skald's organically grown documentation with a small,
explicit information architecture. The result should let a reader distinguish
language semantics, compiler support, implementation architecture, target and
runtime contracts, development guidance, future design, and historical plans
without reconstructing those roles from one large draft.

The migration is a verified rewrite, not a mechanical file split. Current
claims must be checked against their implementation and tests; planned claims
must be checked against active design decisions; contradictions must be
resolved or recorded rather than copied into the new structure.

## 1. Scope and invariants

The overhaul includes:

- one concise language overview with focused semantic documents by subject;
- one authoritative feature-status matrix separating implemented, frozen,
  exploratory, and unresolved behavior;
- one exact implemented grammar without type-system, IR, ABI, or rollout
  narration;
- separate compiler, backend, runtime ABI, driver, testing, and debugging
  documentation;
- verification of every migrated current-behavior claim against code, tests,
  dumps, goldens, headers, build rules, or runtime harnesses as appropriate;
- explicit resolution or follow-up recording for incorrect, contradictory, or
  unsupported existing claims;
- removal of duplicated inventories, historical milestone prose, and stale
  compatibility documents after their useful content has moved;
- updated links in source comments, living documentation, and archived
  roadmaps without rewriting archived implementation history;
- a repository-local documentation check invoked by `make check`, so existing
  external clean-checkout automation validates the new structure without a
  repository CI job.

The overhaul preserves these invariants:

1. Each fact has one authoritative home. Other documents may summarize it only
   briefly and link to that authority.
2. Language documents describe source-visible semantics. They do not prescribe
   Rust modules, stable IDs, HIR/MIR nodes, target registers, object layouts,
   hidden parameters, C symbols, or implementation algorithms.
3. Compiler documents describe responsibilities and durable boundaries rather
   than duplicating language rules or maintaining exhaustive file inventories.
4. Target and runtime details remain outside the language specification unless
   they are source-observable language guarantees.
5. Current behavior, frozen design, exploratory direction, and open questions
   are visibly different states. A sketch never reads as an implemented or
   normative feature.
6. Implemented behavior is verified before publication. Existing prose is
   evidence to investigate, not proof that its claim is correct.
7. Documentation-only work does not silently change language or compiler
   behavior. A discovered behavior defect or semantic choice that requires
   code changes is recorded in the documentation discoveries backlog or the
   relevant feature roadmap.
8. Archived roadmaps remain historical records. Only paths and links needed to
   reach their living authorities may be repaired.
9. The superseded draft and grammar monoliths are removed after migration;
   Git history, not another stale archive copy, preserves their old text.
10. Every task keeps navigation and links valid at its merge boundary.

The overhaul does not include:

- new language features or deliberate changes to accepted source behavior;
- compiler, backend, or runtime refactors unrelated to documentation accuracy;
- freezing an unresolved feature design owned by another roadmap;
- a generated specification system, documentation website, or custom markup
  format;
- a broad prose-style linter or new network-fetched documentation dependency;
- repository-owned CI configuration.

## 2. Verification and authority model

Every migrated claim must first be classified as one of:

- **implemented contract** — behavior accepted or emitted by the current
  compiler and covered by current implementation/tests;
- **frozen design** — source or semantic behavior deliberately settled for an
  active implementation plan but not yet fully implemented;
- **exploratory direction** — useful design constraints or examples that are
  explicitly non-normative;
- **open question** — a missing choice that must be resolved before
  implementation;
- **implementation detail** — compiler, target, runtime, driver, or test
  behavior that belongs outside the language specification;
- **history** — rollout narrative retained only in archived roadmaps or Git.

Verification uses the authority closest to the behavior:

| Claim | Verification evidence |
|---|---|
| Accepted tokens and syntax | lexer/parser code, syntax tests, exact AST dumps, compile-failure goldens |
| Name and type behavior | resolver/type-checker code, focused diagnostics, resolved/HIR dumps |
| Evaluation and ownership | HIR/MIR lowering, verifier rules, deterministic dumps, native lifecycle goldens |
| Compiler API and phase boundaries | public API tests, facade exports, architecture-owning modules |
| Target ABI and layout | backend code, legality/layout tests, accepted assembly, native call-pressure tests |
| Runtime ABI | public C header, runtime implementation, Makefile, direct C harnesses, link-marker tests |
| Driver and artifacts | CLI/pipeline/artifact/toolchain code and tests |
| Frozen future design | active roadmap decisions and the focused language-design document |

Archived roadmaps may explain why a behavior exists, but they are not the
authority for current behavior. If evidence disagrees:

1. correct plainly stale or false prose when implementation and tests establish
   the intended current contract;
2. add or strengthen a focused regression test when a documented current
   guarantee is real but insufficiently protected;
3. record ambiguity requiring a language or implementation choice in
   `docs/roadmaps/DOCUMENTATION_OVERHAUL_DISCOVERIES.md` rather than deciding it
   incidentally;
4. leave code behavior unchanged unless a separately reviewed task explicitly
   authorizes a behavior fix.

## 3. Target information architecture

```text
docs/
├── README.md
├── language/
│   ├── README.md
│   ├── STATUS.md
│   ├── GRAMMAR.md
│   ├── TYPES_AND_VALUES.md
│   ├── FUNCTIONS_AND_CONTROL_FLOW.md
│   ├── CLASSES_AND_LIFECYCLE.md
│   ├── ALIASES_AND_OWNERSHIP.md
│   ├── POLYMORPHISM.md
│   ├── MODULES_AND_INTEROP.md
│   └── ERRORS.md
├── compiler/
│   ├── README.md
│   ├── PHASES_AND_IR.md
│   ├── BACKEND.md
│   ├── RUNTIME_ABI.md
│   └── DRIVER_AND_ARTIFACTS.md
├── development/
│   ├── README.md
│   ├── TESTING.md
│   └── DEBUGGING.md
├── roadmaps/
└── archive/
```

The authority split is:

| Fact | Authoritative owner |
|---|---|
| Language meaning | focused document under `docs/language/` |
| Exact accepted syntax | `docs/language/GRAMMAR.md` |
| Current support and design maturity | `docs/language/STATUS.md` |
| Compiler phase contracts | `docs/compiler/PHASES_AND_IR.md` |
| Target ABI, layout, and legality | `docs/compiler/BACKEND.md` |
| Runtime C contract | `docs/compiler/RUNTIME_ABI.md` |
| CLI and artifact behavior | `docs/compiler/DRIVER_AND_ARTIFACTS.md` |
| Testing and debugging workflows | `docs/development/` and concise test-local guides |
| Feature order and dependencies | active roadmap index and focused roadmaps |
| Historical implementation sequence | archived roadmaps and Git history |

Semantic filenames deliberately avoid numeric prefixes so later reordering does
not churn links. A focused document begins with its maturity and authority; it
does not repeat the complete feature-status matrix.

## 4. Progress

- [x] DOC0 — Establish documentation authority and verification tooling
- [x] DOC1 — Create the language overview and status authority
- [x] DOC2 — Rewrite the implemented grammar
- [x] DOC3 — Rewrite types, values, and expressions
- [x] DOC4 — Rewrite functions and control flow
- [x] DOC5 — Rewrite classes, initialization, and object places
- [x] DOC6 — Rewrite copying, destruction, and object lifetimes
- [x] DOC7 — Rewrite aliases and ownership
- [x] DOC8 — Establish the polymorphism design document
- [x] DOC9 — Rewrite modules and foreign interoperation
- [x] DOC10 — Rewrite errors and prune premature feature sketches
- [x] DOC11 — Rewrite compiler architecture and phase contracts
- [x] DOC12 — Separate backend and target documentation
- [x] DOC13 — Establish the runtime ABI authority
- [ ] DOC14 — Rewrite driver, artifact, and workflow documentation
- [ ] DOC15 — Consolidate testing and debugging guidance
- [ ] DOC16 — Update entry points and remove superseded monoliths
- [ ] DOC17 — Verify, close, and archive the overhaul

## 5. PR-sized implementation sequence

### DOC0 — Establish documentation authority and verification tooling

**Purpose:** Create the rules and checks that make every later migration
consistent and reviewable.

- [x] Inventory every living-document heading, incoming repository link, and
      source-code documentation reference; map each retained claim to its
      intended authoritative destination.
- [x] Rewrite `docs/README.md` with the authority, maturity, linking, and
      historical-document rules from this roadmap.
- [x] Create `DOCUMENTATION_OVERHAUL_DISCOVERIES.md` for contradictions,
      behavior defects, unresolved choices, and unrelated cleanup discovered
      during migration.
- [x] Add a dependency-free repository documentation check for relative files,
      local anchors, and required index entries; expose it as `make docs-check`
      and include it in `make check`.
- [x] Document that existing external infrastructure picks up the check through
      clean-checkout `make check`; add no CI configuration.

**Tests:** Unit-test the documentation checker with valid, missing-file,
missing-anchor, encoded-path, and archive-link fixtures; run `make docs-check`,
`make check`, and `git diff --check`.

**Exit criteria:** Every current document has a planned disposition, migration
discrepancies have a separate owner, and broken repository-local Markdown links
fail through the standard Makefile gate.

### DOC1 — Create the language overview and status authority

**Purpose:** Give readers a reliable entry point and a single answer to what is
implemented, designed, exploratory, or unresolved.

- [x] Create `docs/language/README.md` as a concise broad specification of
      language purpose, terminology, safety direction, value/object model, and
      document navigation.
- [x] Create `docs/language/STATUS.md` as the sole feature maturity and compiler
      support matrix, with links to semantic documents and active roadmaps.
- [x] Verify every implemented-status claim against current source, phase tests,
      public API tests, goldens, runtime tests, and target availability.
- [x] Keep unresolved and future features brief; link to their roadmap or open
      questions instead of embedding speculative implementation plans.
- [x] Add the language directory to the documentation index while retaining
      legacy links until their content has migrated.

**Tests:** Check the status matrix against `make help`, current golden discovery,
runtime targets, and compiler target registry; run `make docs-check`,
`make check`, and `git diff --check`.

**Exit criteria:** A reader can determine current support and design maturity
without consulting the old draft, root README feature inventory, or archived
roadmaps.

### DOC2 — Rewrite the implemented grammar

**Purpose:** Make accepted source syntax precise without mixing in typing,
ownership, IR, ABI, or implementation history.

- [x] Create `docs/language/GRAMMAR.md` from verified lexical rules, keywords,
      contextual words, EBNF, precedence, associativity, source-observable
      recovery, and nesting limits.
- [x] Check every token and production against lexer/parser dispatch and exact
      syntax tests, including lifecycle classification and assignment-shaped
      semantic forms.
- [x] Move semantic restrictions to their focused language owners and compiler
      recovery strategy to the compiler phase document.
- [x] Update lexer/parser code comments, syntax tests, and living links to the
      new grammar authority.
- [x] Keep `grammar/README.md` temporarily only if unmigrated links still need
      it; do not maintain two grammar authorities.

**Tests:** Run lexer and syntax tests, exact AST dump tests, representative
compile-failure goldens, `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** The new grammar exactly describes the accepted source shape,
and no grammar section explains HIR, MIR, backend lowering, or feature rollout.

### DOC3 — Rewrite types, values, and expressions

**Purpose:** Give core value semantics one concise language-owned home.

- [x] Create `TYPES_AND_VALUES.md` covering implemented primitive and exact
      nominal types, `unit`, literal typing/ranges, expressions, operators,
      equality availability, conversions, and value-versus-place terminology.
- [x] Separate implemented exact-type rules from future optional, array,
      string, function-value, and conversion directions through explicit
      maturity links rather than interleaved profiles.
- [x] Verify literal, numeric, boolean, field-value, grouping, operator, and
      unsupported-context claims against type checking, raw-bit dumps, and
      goldens.
- [x] Move target representations, C mappings, register classes, and parsing
      algorithms to compiler/backend documentation.
- [x] Remove duplicated type summaries from the old draft as each fact gains
      its new owner.

**Tests:** Run literal, primitive-expression, field-expression, diagnostic, HIR
dump, and numeric golden tests; run `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** Core type and expression behavior has one verified semantic
description independent of parser, IR, runtime, and x86 implementation.

### DOC4 — Rewrite functions and control flow

**Purpose:** Consolidate callable and statement semantics around source-visible
behavior and evaluation order.

- [x] Create `FUNCTIONS_AND_CONTROL_FLOW.md` covering declarations, parameter
      categories by reference, calls, results, scopes, locals, blocks,
      conditionals, return analysis, call statements, and evaluation order.
- [x] State implemented single-file and restricted-external boundaries clearly
      while linking alias and object-value details to their semantic owners.
- [x] Verify scope, shadowing, definite return, receiver/argument order,
      object-result sequencing, and cleanup-before-return claims against
      resolver, type-checker, MIR, and native tests.
- [x] Move ABI placement, hidden results, phase data structures, and CFG
      strategy to compiler/backend documentation.
- [x] Reduce loop and function-value material to concise maturity entries until
      their designs are implementation-ready.

**Tests:** Run binding, control-flow, call, return, object-result, MIR CFG, and
conditional golden tests; run `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** Callable and control-flow semantics are complete for the
implemented subset and do not prescribe compiler control-flow representation.

### DOC5 — Rewrite classes, initialization, and object places

**Purpose:** Replace chronological object-profile prose with one conceptual
class and subobject model.

- [x] Begin `CLASSES_AND_LIFECYCLE.md` with nominal class declarations, member
      namespaces, receiver access, inline fields, containment, construction,
      initialization liveness, object places, and nested projections.
- [x] Reconcile the old restricted object and class-typed-field profiles into
      current semantic rules without stage names or implementation history.
- [x] Verify member classification, exact initializer requirements,
      containment-cycle behavior, field initialization, nested access, receiver
      mutability, and diagnostic precedence against source and phase tests.
- [x] Keep field offsets, empty-object target size, hidden receivers, stable
      compiler identities, and phase-node shapes out of the language document.
- [x] Retain explicit exclusions and links for inheritance, shared sources,
      exceptional initialization, and other unimplemented extensions.

**Tests:** Run syntax/resolution/type-check object declaration, construction,
containment, place, receiver, dump, and corresponding golden tests; run
`make docs-check`, `make check`, and `git diff --check`.

**Exit criteria:** Readers can understand classes, complete subobjects, and
initialization state without consulting any historical object-profile section.

### DOC6 — Rewrite copying, destruction, and object lifetimes

**Purpose:** Give exact-class value and lifetime behavior one source-oriented
semantic narrative.

- [x] Complete `CLASSES_AND_LIFECYCLE.md` with lifecycle declaration slots,
      synthesis capability, copy construction, assignment, destruction,
      value parameters/results, return storage semantics, temporaries,
      full-expression boundaries, and permitted elision.
- [x] Verify operation selection, self-assignment, field order, registration,
      cleanup order, normal exits, result transfer, grouping-sensitive
      materialization, and exactly-once behavior against HIR/MIR and native
      lifecycle coverage.
- [x] Separate language-required destination and cleanup behavior from MIR
      instructions, frame homes, hidden pointers, and backend recursion.
- [x] Resolve plainly incorrect old prose and record any semantic ambiguity or
      implementation discrepancy in the documentation discoveries backlog.
- [x] Replace broad repeated exclusion lists with focused boundary links.

**Tests:** Run lifecycle/capability, destruction, value-parameter/result,
temporary, cleanup-verifier, determinism, and object-value golden tests; run
`make docs-check`, `make check`, and `git diff --check`.

**Exit criteria:** The complete implemented object lifetime is specified once,
is backed by current tests, and contains no rollout or target-lowering prose.

### DOC7 — Rewrite aliases and ownership

**Purpose:** Separate source-level access and lifetime guarantees from their
current and future storage implementations.

- [x] Create `ALIASES_AND_OWNERSHIP.md` covering binding modes, access,
      non-exclusivity, non-escape, forwarding, supported exact-class sources,
      and their interaction with object places.
- [x] Verify the implemented alias subset against syntax, resolution, type
      checking, MIR verification, mixed-ABI native tests, and exclusions.
- [x] Describe shared ownership and borrow anchors only at their present design
      maturity, separating semantic guarantees from headers, allocation
      layout, reference-count algorithms, and internal calling conventions.
- [x] Reduce local aliases, optionals, arrays, and shared-source aliases to
      explicit open constraints until focused designs freeze them.
- [x] Move pointer ABI details to backend documentation and runtime ownership
      mechanics to the runtime ABI document.

**Tests:** Run alias phase tests, access/overlap/forwarding diagnostics, MIR
corruption tests, native alias call-pressure cases, `make docs-check`,
`make check`, and `git diff --check`.

**Exit criteria:** Alias and ownership semantics have one authority, and future
shared behavior cannot be mistaken for current compiler support.

### DOC8 — Establish the polymorphism design document

**Purpose:** Give the active polymorphism roadmap a durable language-design
destination without prematurely deciding its open profile choices.

- [x] Create `POLYMORPHISM.md` covering currently agreed single-inheritance,
      base, lifecycle-composition, virtual, interface, `Obj`, slicing, upcast,
      type-test, and narrowing direction.
- [x] Classify every migrated rule as frozen or open against the active
      polymorphism roadmap; do not infer missing syntax, representation, or
      failure behavior from Niflheim.
- [x] Remove compiler metadata, dispatch-table layout, complete-object pointer
      ABI, target offsets, and runtime allocation details from language prose.
- [x] Preserve source-visible access, ownership, slicing, lifecycle, and
      dispatch constraints needed by the later profile-design task.
- [x] Update the status matrix and roadmap links so polymorphism profile design
      edits this document rather than the legacy draft.

**Tests:** Review each implemented-baseline claim against current object and
alias tests, check every future rule against the active roadmap, then run
`make docs-check`, `make check`, and `git diff --check`.

**Exit criteria:** Polymorphism has one clearly non-implemented design authority
whose open decisions are explicit and whose structure is ready for profile
freeze.

### DOC9 — Rewrite modules and foreign interoperation

**Purpose:** Separate planned module semantics, current single-file behavior,
and source-visible FFI rules from linkage implementation.

- [x] Create `MODULES_AND_INTEROP.md` covering the current compilation-unit
      boundary, declaration namespaces, entry point, exact-symbol external
      declarations, trusted-ABI behavior, and explicit unsupported forms.
- [x] Mark imports, exports, multiple files, visibility, separate compilation,
      coalescing, packages, and broader FFI as exploratory or open rather than
      current syntax.
- [x] Verify entry-point, duplicate declaration, external signature, linker
      symbol, and source diagnostic claims against resolver/type-checker/driver
      tests and goldens.
- [x] Move System V classification, C widths, internal symbol spelling, link
      marker, and tool invocation to backend/runtime/driver documents.
- [x] Remove module syntax sketches that have no frozen parser or semantic
      contract, retaining only constraints useful to future design.

**Tests:** Run declaration, external-call, entry-point, toolchain, symbol, and
compile-failure tests; run `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** Current interoperation is precise, future modules are clearly
non-implemented, and no language document owns target linkage mechanics.

### DOC10 — Rewrite errors and prune premature feature sketches

**Purpose:** Make current failure semantics clear while replacing pseudo-specs
for unresolved features with concise design boundaries.

- [x] Create `ERRORS.md` covering compile-time diagnostics at a language level,
      unrecoverable runtime failures, deterministic cleanup obligations, and
      the current absence of recoverable exceptions.
- [x] Retain only the checked-exception constraints already required by object
      lifetime; move lowering options out and mark syntax/type rules open.
- [x] Audit optionals, arrays, strings, loops, iteration, statics, closures,
      generics, and standard-library sketches; keep only settled language
      direction and actionable open questions in the appropriate owner/status
      entry.
- [x] Verify current runtime-failure claims against native/runtime tests and
      avoid promising exact status or diagnostics not enforced by tests.
- [x] Remove the legacy resolved-decisions appendix after each retained
      decision is verified and moved to its semantic owner.

**Tests:** Run diagnostic rendering, fatal runtime, relevant compile-failure,
cleanup, and robustness tests; run `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** No exploratory sketch reads as a usable feature, and every
current failure guarantee is precise, tested, and owned once.

### DOC11 — Rewrite compiler architecture and phase contracts

**Purpose:** Preserve durable compiler boundaries while removing feature-level
semantic duplication and fragile module inventories.

- [x] Create `docs/compiler/README.md` with architecture principles, repository
      roles, pipeline, extension policy, and compiler crate API policy.
- [x] Create `PHASES_AND_IR.md` for source/diagnostics, lexer, syntax,
      resolution/identities, HIR, MIR, verification, passes, deterministic
      dumps, and trust boundaries.
- [x] Verify every public path, phase product, facade, verifier boundary, and
      test-only hook claim against the current Rust modules and public API
      tests.
- [x] Describe responsibilities and invariants rather than feature rollout,
      object semantics, exhaustive file trees, or exact private helper names.
- [x] Move target, runtime, driver, testing, and debugging detail to their new
      owners and update source-level architecture links.

**Tests:** Run public API, phase dump, MIR verification, determinism, and
compiler unit tests; run `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** Compiler architecture is navigable and accurate without
duplicating language semantics or freezing replaceable private organization.

### DOC12 — Separate backend and target documentation

**Purpose:** Give target legality, layout, ABI, and code generation one
implementation-owned authority.

- [x] Create `BACKEND.md` describing the backend interface, supported target
      registry, x86-64 System V legality, primitive/class layout, argument and
      result classification, frames, symbols, instruction selection, and
      assembly emission.
- [x] Verify every size, alignment, register-class, hidden-argument, stack,
      symbol, overflow, and structured-error claim against backend code and
      focused tests.
- [x] Keep language evaluation, ownership, copy, and cleanup selection as links
      to semantic authorities; document only how verified MIR is realized.
- [x] State internal versus external ABI stability explicitly and avoid
      promising exact compiler-generated textual symbols.
- [x] Update architecture, debugging, and source comments to link to the target
      authority.

**Tests:** Run backend layout, ABI, legality, instruction-selection, assembler,
call-pressure, and native execution tests; run `make docs-check`, `make check`,
and `git diff --check`.

**Exit criteria:** All current target-specific claims live in one verified
backend document and no longer appear as language rules.

### DOC13 — Establish the runtime ABI authority

**Purpose:** Make the versioned C runtime contract precise and remove its
duplication across language, architecture, README, grammar, and test prose.

- [x] Create `RUNTIME_ABI.md` covering the current marker/version mechanism,
      public header types and functions, platform requirements, exact output
      records, failure behavior, and current runtime responsibility boundary.
- [x] Verify symbols, signatures, version, platform assertions, output bytes,
      flushing, and failure behavior against the header, C implementation,
      Makefiles, link tests, and direct runtime harnesses.
- [x] Distinguish the current ABI from future allocation, reference counting,
      metadata, panic, strings, and exception support.
- [x] Keep source-level FFI use as a link to language interoperation and target
      call lowering as a link to backend documentation.
- [x] Slim `tests/runtime/README.md` to harness mechanics and links rather than
      another ABI specification.

**Tests:** Run `make runtime-test`, runtime ABI marker/toolchain tests, relevant
native output goldens, `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** The C header and one living document define the tested
runtime ABI; all other documents summarize and link.

### DOC14 — Rewrite driver, artifact, and workflow documentation

**Purpose:** Separate compiler orchestration and contributor commands from
language and backend contracts.

- [ ] Create `DRIVER_AND_ARTIFACTS.md` covering compiler entry points, target
      selection, CLI modes, tool invocation, runtime selection, atomic
      publication, path alias rejection, and structured failures.
- [ ] Verify claims against driver code and CLI, pipeline, artifact, and
      toolchain tests.
- [ ] Create `docs/development/README.md` for prerequisites, authoritative
      Makefile interfaces, MSRV use, and external clean-checkout automation.
- [ ] Keep `make help` as the detailed command inventory and remove duplicated
      command catalogs from architecture documents.
- [ ] Add no CI job and no separate workflow whose behavior cannot be invoked
      locally.

**Tests:** Run driver/CLI/artifact/toolchain tests, `make docs-check`,
`make check`, `make msrv-check` if build files or supported Rust syntax change,
and `git diff --check`.

**Exit criteria:** CLI, artifacts, toolchain, and contributor validation have
accurate owners and remain reproducible through the Makefile.

### DOC15 — Consolidate testing and debugging guidance

**Purpose:** Document how to verify and inspect the compiler without turning
test guides into duplicate feature specifications.

- [ ] Create `development/TESTING.md` for test layers, ownership, fixture and
      corpus placement, determinism, robustness, focused commands, and when to
      add unit, integration, golden, or runtime coverage.
- [ ] Create `development/DEBUGGING.md` from the current phase-artifact table,
      dump workflow, verifier boundaries, assembly inspection, and concise
      symptom-to-owner guidance.
- [ ] Verify renderer names, public paths, test locations, commands, process
      isolation, mutation hooks, and verifier invocation points against code
      and tests.
- [ ] Slim `tests/README.md` and test-local READMEs to discovery formats,
      sidecars, harness mechanics, and links; remove exhaustive feature
      inventories.
- [ ] Keep language and ABI expectations in their authoritative documents even
      when a test guide names representative coverage.

**Tests:** Run public API and dump tests, golden expectation tests,
`make robustness-smoke`, `make docs-check`, `make check`, and
`git diff --check`.

**Exit criteria:** Contributors can select, add, and debug tests from concise
guidance whose factual paths and commands are verified and non-duplicative.

### DOC16 — Update entry points and remove superseded monoliths

**Purpose:** Make the new structure the only living documentation surface once
all content has a verified owner.

- [ ] Rewrite the root README as project identity, short capability summary,
      quickstart, target statement, history note, and links to status and
      focused documentation.
- [ ] Distribute stable extension rules, object-model sequencing, and future
      status from `NEXT_SLICE_BOUNDARIES.md` to compiler policy, status, and the
      roadmap index; remove the duplicate document.
- [ ] Remove `SKALD_DRAFT_SPEC.md`, `REPO_STRUCTURE.md`, `DEBUGGING.md`, and
      `grammar/README.md` only after every retained claim and incoming link has
      a new owner; remove an empty `grammar/` directory.
- [ ] Repair links in Rust documentation comments, samples, tests, living docs,
      roadmap indexes, and archived roadmaps without rewriting archived
      historical prose.
- [ ] Search living documentation for stage/profile codes, implementation
      diary language, stale filenames, duplicate command lists, and repeated
      ABI authorities.

**Tests:** Run repository-wide stale-path and roadmap-code searches,
`make docs-check`, `make check`, and `git diff --check`.

**Exit criteria:** Every normal entry point reaches the new structure, no
superseded monolith remains, and all repository-local links resolve.

### DOC17 — Verify, close, and archive the overhaul

**Purpose:** Prove that the new documentation is accurate, maintainable, and
complete before treating the migration as finished.

- [ ] Re-audit every implemented status row against current code and tests and
      every focused document against the authority table.
- [ ] Review remaining duplication, document size, cross-links, maturity
      labels, open questions, and public command/path references.
- [ ] Resolve all high-priority documentation discoveries; retain lower-priority
      actionable items in the indexed discoveries backlog with evidence and a
      useful implementation boundary.
- [ ] Verify that active roadmaps target the new semantic/architecture
      documents and that archived roadmaps are used only for history.
- [ ] Mark DOC0-DOC17 complete, move this roadmap to `docs/archive/`, update
      active and archive indexes, and repair all links.

**Tests:** From an artifact-free snapshot or clean checkout, run
`make docs-check`, `make robustness-smoke`, `make check`, `make msrv-check`,
and `git diff --check`; repeat repository-wide stale-link and duplicate-authority
searches.

**Exit criteria:** The documentation has one authority per fact, every current
claim has verification evidence, no high-priority contradiction remains, all
quality gates pass, and the roadmap is archived.

## 6. Ordering and dependencies

The sequence is intentionally content-first within each authority boundary:

- DOC0 establishes verification and migration rules. DOC1 then makes maturity
  explicit before detailed rewrites can accidentally present future behavior
  as current.
- DOC2-DOC10 rewrite language documentation from syntax and core values through
  object ownership and future design. The class split keeps initialization and
  places separate from the larger lifetime/copy rewrite.
- DOC0-DOC8 precede polymorphism profile design. That task should freeze its
  decisions directly in `docs/language/POLYMORPHISM.md`; it should not add more
  material to the legacy draft.
- DOC11-DOC15 can proceed after their source language facts have authoritative
  homes. Backend and runtime work remain separate so target realization does
  not become the runtime contract.
- DOC16 removes old authorities only after every preceding destination is
  complete. DOC17 is the independent accuracy and maintainability closeout.

Within the documentation overhaul, tasks remain sequential where they move the
same source material. After DOC10, compiler architecture, backend/runtime, and
testing/debugging work may proceed in parallel only when their link edits do
not conflict. Polymorphism compiler implementation remains ordered by its own
roadmap after profile design.

Every task updates its affected living documentation in the same PR, runs
focused verification plus `make docs-check`, `make check`, and
`git diff --check`, and marks checkboxes only after its exit criteria hold.
`make msrv-check` is additionally required when Rust syntax, manifests, build
rules, or supported toolchain claims change and at final closeout.

The repository gains no CI configuration. Existing external infrastructure
continues to run `make check` regularly from clean checkouts; including
`docs-check` in that target makes documentation validation local and externally
repeatable through the same interface.
