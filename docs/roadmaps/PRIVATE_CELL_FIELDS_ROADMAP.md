# Private Cell Fields Roadmap

Status: in progress; CFI1 is complete and CFI2 is next.

This roadmap implements the frozen
[private cell field language contract](../language/CLASSES_AND_LIFECYCLE.md#private-cell-field-direction)
and its
[compiler representation contract](../compiler/PHASES_AND_IR.md#private-cell-field-representation).
It adds contextual private field metadata and one verified whole-field write
authorization through read-only object places while preserving ordinary
types, assignment families, lifecycle, aliases, layout, calling conventions,
and runtime ABI.

The confirmed decisions are preserved in the
[design proposal](PRIVATE_CELL_FIELDS_DESIGN_PROPOSAL.md). Tasks below
implement those decisions without reopening them.

## Scope and invariants

- Accept only contextual `private cell name: T;` instance fields. Preserve
  ordinary fields named `cell` or `private`, static fields named `cell`, and
  every unaffected identifier position.
- Keep one ordinary private `FieldId`, declaration order, stored type, layout,
  containment edge, and lifecycle role plus an explicit cell marker and exact
  modifier span.
- Authorize only complete replacement of the selected cell field through a
  read-only object place from a callable lexically owned by the field's exact
  declaring class.
- Do not upgrade the complete receiver or projection path to mutable access.
  Nested field mutation, mutable method calls, array or optional payload
  mutation, shared-pointee mutation granted only by cell, and `mut ref`
  forwarding remain rejected through read-only access.
- Preserve every operation already available through a genuinely mutable
  root. Declaring a field `cell` removes no ordinary mutable capability.
- Apply the field type's existing scalar, exact-class, optional, shared-owner,
  and array assignment behavior, including evaluation order, self-assignment,
  ownership, displacement, failure, and full-expression cleanup.
- Treat cell state normally in initialization, synthesized and user
  lifecycle, containment, generic requirements, copying, assignment, and
  destruction. Do not add transient or automatically invalidated cache state.
- Preserve ordinary declaring-class privacy, inheritance, closed generic
  specialization, virtual/interface read-only method signatures, produced
  read-only receivers, and non-exclusive call-scoped aliases.
- Reuse existing array-backing and shared-owner anchors plus optional presence
  guards. A cell write must never bypass an active lifetime protection or an
  unavailable assignment capability.
- Carry explicit authorization into typed HIR and MIR. Independent MIR
  verification must prove the field marker, exact endpoint, callable owner,
  receiver access, assignment family, liveness, guards, anchors, ownership,
  and cleanup without forging mutable receiver access.
- Preserve deterministic diagnostics, identities, dumps, native behavior,
  target layout, internal ABI, public C runtime surface, and runtime ABI
  version 9.
- Do not add public or static cells, a `Cell<T>` library type, properties,
  escaping references, atomicity, synchronization, volatility, thread-local
  storage, mutation effects in callable types, or purity guarantees for `fn`.
- Keep the cached `std::str::Str` hash migration out of this roadmap. The
  string language item remains its current exact three-field descriptor until
  a separate follow-up changes compiler-created literal initialization.

## Progress

- [x] CFI0 — Represent contextual private cell declarations
- [x] CFI1 — Authorize typed whole-field replacement
- [ ] CFI2 — Preserve and verify cell writes in MIR
- [ ] CFI3 — Prove lifecycle-bearing and alias-safe assignment families
- [ ] CFI4 — Harden composition and publish the implemented contract

Every Rust implementation task runs focused owner tests followed by
`make check`, `make msrv-check`, and `git diff --check`. The closing task also
runs the repository's extended and independent-process determinism coverage
through the documented Makefile interfaces. This roadmap adds no repository
CI.

## PR-sized implementation sequence

### CFI0 — Represent contextual private cell declarations

**Purpose:** Establish exact source and cross-phase field metadata before any
read-only write is authorized against it.

- [x] Add an explicit cell modifier and span to instance-field AST metadata.
      Parse it only in `private cell name: T;`, keeping both words contextual
      and preserving `cell: T;`, `private: T;`, `private cell: T;`, and
      `private static cell: T;` with their existing ordinary meanings.
- [x] Diagnose missing private visibility, static/method/lifecycle use,
      duplicates, reordering, missing field names or punctuation, and malformed
      recovery at exact modifier/member spans.
- [x] Preserve one ordinary `FieldId`, visibility, type, declaration order,
      and explicit cell marker through resolved declarations, class tables,
      source and resolved dumps, generic template analysis, and closed
      specialization.
- [x] Carry the marker through HIR and MIR field declarations and deterministic
      dumps so later verification can inspect it. Keep privacy source-owned
      and retain only the durable cell capability after authorization.
- [x] Extend syntax/resolution consistency tests to reject public or static
      modifier forms, and declaration-lowering/verifier tests to catch missing
      or mismatched durable cell metadata, without changing field layout or
      synthesized lifecycle selection.
- [x] Keep assignment through a read-only root rejected during this task. A
      parsed cell marker alone must not silently grant an unverified write.
- [x] Update living documentation only for the declaration syntax and phase
      representation that actually ship in this task; retain frozen or staged
      status for executable cell writes.

**Tests:** Lexer/parser success, contextual disambiguation, recovery, span, and
syntax-dump tests; resolution declaration, privacy, collision, inheritance,
generic specialization, and dump tests; HIR/MIR declaration-lowering and
verifier mutation tests; existing field-layout and lifecycle regressions;
then the documented complete Rust-task gates.

**Exit criteria:** Valid cell declarations flow deterministically through
syntax, resolution, specialization, HIR, and verified MIR as ordinary private
fields with one exact capability marker, malformed forms stop with focused
diagnostics, layout and lifecycle remain unchanged, and read-only assignment
is still rejected.

Completed 2026-08-16. Focused cross-phase tests, `make check`,
`make msrv-check`, documentation validation, and `git diff --check` passed.

### CFI1 — Authorize typed whole-field replacement

**Purpose:** Define one centralized type-checking decision for the new
permission without widening receiver or nested-place access.

- [x] Add one explicit typed field-write authorization that distinguishes an
      ordinary mutable place from an exact declaring-class cell write. Keep
      the concrete representation cohesive across the existing assignment
      families rather than duplicating access predicates per type.
- [x] Authorize a read-only destination only when it ends at one selected cell
      field and the current callable's lexical class owner equals that field's
      declaring class. Preserve existing privacy diagnostic precedence.
- [x] Admit complete scalar, exact-class, optional, shared-owner, and array
      field replacements through that decision, selecting precisely the same
      type compatibility, generic requirement, assignment capability, source
      evaluation, and HIR operation as an ordinary mutable field assignment.
- [x] Keep initializer writes and synthesized lifecycle on their existing
      trusted initialization/mutation paths. Do not classify uninitialized
      construction as interior mutation.
- [x] Reject cell-derived nested field or element assignment, mutable method
      receivers, optional payload mutation, and `mut ref` arguments while
      retaining all such operations through a genuinely mutable root.
- [x] Cover `self`, `ref` parameters, authorized checked views, grouping,
      canonical base projection, static methods with explicit object aliases,
      and other existing read-only field-place roots without introducing an
      escaping place category.
- [x] Expose the narrow authorization in HIR dumps and diagnostics. Until MIR
      independently verifies it in the next task, keep complete compilation
      behind an explicit phase-owned gate rather than emitting unsupported
      lower IR.

**Tests:** Focused type-checking and HIR tests for every assignment family,
lexical owner, read-only root shape, mutable-root preservation, direct versus
nested endpoint, private-access precedence, generic requirement inference,
malformed or unavailable assignment capabilities, and deterministic dumps;
compile-failure coverage for the temporary lower-phase gate; then the complete
Rust-task gates.

**Exit criteria:** Type checking has one auditable rule that accepts exactly
whole selected cell replacement and emits typed authorization without changing
the receiver's access, every neighboring mutation remains correctly accepted
or rejected, and no authorized write reaches unverifiable MIR.

Completed 2026-08-16. Focused type-checking, HIR, and driver tests,
`make check`, `make msrv-check`, documentation validation, and
`git diff --check` passed.

### CFI2 — Preserve and verify cell writes in MIR

**Purpose:** Make cell authorization an independently verified target-
independent fact before native execution relies on it.

- [ ] Lower typed cell authorization into an explicit MIR write capability
      carried by every applicable store, copy assignment, optional operation,
      shared-owner replacement, and array replacement path. Do not encode the
      permission by changing a receiver or source place to mutable.
- [ ] Extend preliminary and final MIR verification to prove the selected
      endpoint is the declared cell field, the enclosing definition is owned
      by its exact declaring class, the receiver access is compatible, and the
      operation's type and assignment family match the field.
- [ ] Reject forged authorization on ordinary fields, nested projections,
      different declaring classes, initialization operations, static fields,
      mismatched assignment families, dead places, or malformed receiver
      origins.
- [ ] Preserve existing optional presence, shared ownership, array backing,
      lifetime, cleanup, and view verification rather than creating a bypass
      around their path-state analyses.
- [ ] Lower verified operations through ordinary backend place addressing and
      assignment machinery. Erase only the authorization evidence no longer
      needed after the final verifier; add no target instruction or runtime
      call.
- [ ] Remove the temporary executable gate and add a minimal source-to-native
      primitive-optional cache that proves a read-only method can populate and
      reuse one cell.
- [ ] Make MIR and assembly dumps deterministic and prove no field offset,
      class size, callable ABI, symbol, or runtime ABI change beyond ordinary
      program code for the assignment.

**Tests:** HIR-to-MIR lowering for each assignment carrier; structural,
lifetime, ownership, optional, array, and cleanup verifier mutation tests;
backend place/store and layout regressions; one focused successful native
cache golden plus invalid ordinary/nested field goldens; then the complete
Rust-task gates.

**Exit criteria:** Every accepted cell write reaches native execution only
through explicit verified MIR evidence, forged or over-broad permissions fail
independently, the minimal cache behaves observably, and runtime/layout/ABI
contracts remain unchanged.

### CFI3 — Prove lifecycle-bearing and alias-safe assignment families

**Purpose:** Validate that interior replacement composes with displaced
owning state and active aliases instead of being safe only for scalar caches.

- [ ] Exercise exact-class cell replacement through synthesized and user copy
      assignment, including unavailable capability, self-assignment,
      lifecycle-visible source order, old-state displacement, and failures.
- [ ] Exercise primitive, class, nested, array, shared-owner, optional-owner,
      and optional-box cell optionals through ordinary injection, presence
      changes, copy/transfer, cleanup, and recursive lifecycle plans.
- [ ] Exercise shared-owner cell replacement with stable, replaceable, and
      produced sources, last-owner destruction, polymorphic views, and hidden
      anchors that keep an aliased old pointee alive.
- [ ] Exercise inline-array cell replacement, nested and lifecycle-bearing
      elements, produced-backing adoption, overlapping aliases, and detached-
      backing anchors that preserve old element storage through the call.
- [ ] Prove active optional payload guards terminate before a re-entrant cell
      replacement can clear, replace, or destroy the guarded container; prove
      ordinary still-present payload mutation remains governed by existing
      access rules.
- [ ] Prove ordinary initialization, object copy construction, object copy
      assignment, and destruction copy or destroy current cell state exactly
      like an ordinary field, with no implicit reset, omission, or transient
      behavior.
- [ ] Harden every affected path-state verifier and full-expression cleanup
      edge with mutation tests and native failure/success observations.

**Tests:** Focused type, HIR, MIR, ownership, optional-guard, array-anchor,
lifetime, cleanup, and backend tests; successful and failing native goldens
for class, optional, shared, and array cell fields; allocation and destruction
observations; then the complete Rust-task gates.

**Exit criteria:** Every currently legal instance-field storage family uses
its ordinary assignment and lifecycle contract through cell authorization,
active aliases remain lifetime-safe under existing anchors and guards, and no
raw-store or scalar-only shortcut remains.

### CFI4 — Harden composition and publish the implemented contract

**Purpose:** Close cross-feature gaps, publish only verified behavior, and
leave the general feature ready for later standard-library adoption.

- [ ] Cover base-declared private cells, inherited method calls, derived
      privacy rejection, exact declaring-class ownership, checked class views,
      produced read-only method receivers, virtual overrides, interface
      witnesses, and function-value calls to cell-writing read-only methods
      where existing callable eligibility permits them.
- [ ] Cover generic cell fields, closed specialization identity, contextual
      assignment requirements, repeated specializations, generic bases,
      optional/array/shared substitutions, module qualification, and
      deterministic IDs and symbols.
- [ ] Complete syntax/resolved/HIR/MIR dump, diagnostics, malformed-source,
      pipeline determinism, native success/failure, and regression matrices;
      audit all field assignment families and all access-propagating consumers
      for accidental permission widening.
- [ ] Promote grammar, class/access, compiler phase, testing, debugging,
      backend, and status documentation from frozen planned behavior to the
      implemented contract. Remove rollout vocabulary and roadmap codes from
      living code, tests, and documentation.
- [ ] Confirm there is no atomicity, synchronization, volatile access,
      `Cell<T>` library abstraction, public/static cell, callable effect,
      purity guarantee, runtime service, or ABI revision.
- [ ] Keep `std::str::Str` at its exact current descriptor contract. Record its
      cached-hash migration as separate follow-up work rather than expanding
      this closing task.
- [ ] Run closure review from an artifact-free snapshot, resolve high-priority
      maintainability findings within the feature boundary, and place any
      lower-priority actionable findings in a separately indexed discoveries
      document.

**Tests:** Complete focused and golden matrices; independent-process compiler
and full-pipeline determinism; malformed-source robustness; documentation link
and index validation; `make check`, `make check-long`, `make msrv-check`, and
`git diff --check` from the documented clean-snapshot workflow.

**Exit criteria:** Private cell fields are an implemented source-to-native
contract across all frozen compositions, every exclusion retains a focused
diagnostic or explicit status boundary, living documentation contains no
stale rollout language, all repository gates pass, and the roadmap is ready to
archive.

## Ordering and dependencies

CFI0 establishes a stable declaration capability before any consumer can rely
on it. CFI1 centralizes the only new source authorization while deliberately
gating execution. CFI2 then makes that authorization independently verifiable
and executable for a minimal scalar cache. CFI3 expands proof across the
ownership and alias mechanisms that make whole-field replacement nontrivial.
CFI4 covers inheritance, dispatch, generics, determinism, and publication only
after the core write and lifetime contracts are executable.

The roadmap depends on the implemented declaring-class privacy, receiver-
access propagation, type-directed field assignment, generic specialization,
call-scoped alias, optional presence-guard, shared-owner anchor, array-backing
anchor, lifecycle, verified MIR, and x86-64 backend contracts. No active
roadmap blocks CFI0. The separate `Str` cached-hash migration depends on this
roadmap's completion but does not block it.
