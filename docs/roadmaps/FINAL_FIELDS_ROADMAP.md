# Final Fields Roadmap

Status: in progress; FFI3 is next.

This roadmap implements the frozen
[final field language contract](../language/CLASSES_AND_LIFECYCLE.md#frozen-final-field-direction),
its [final static contract](../language/STATIC_FIELDS.md#frozen-final-static-field-direction),
and the
[compiler representation contract](../compiler/PHASES_AND_IR.md#frozen-final-field-representation).
It adds shallow final instance and class-owned static fields while preserving
ordinary construction, mutable complete-value replacement, lifecycle,
ownership, aliases, layout, calling conventions, and runtime ABI.

The confirmed decisions are preserved in the
[design proposal](FINAL_FIELDS_DESIGN_PROPOSAL.md). Tasks below implement those
decisions without reopening them.

## Scope and invariants

- Accept only contextual canonical `final name: T;`,
  `private final name: T;`, `final static name: T = expression;`, and
  `private final static name: T = expression;` field forms. Preserve ordinary
  fields named `final`, `static`, `private`, or `cell` and every unaffected
  identifier position.
- Diagnose reordered, duplicated, incomplete, unsupported, or cross-category
  modifiers. The canonical order is visibility, finality, then storage kind;
  `final` and `cell` are mutually exclusive.
- Keep each final declaration's ordinary `FieldId` or `StaticFieldId`,
  declaring class, name, visibility, type, order, specialization identity,
  layout, containment, ownership, and lifecycle role plus one explicit final
  marker and exact modifier span.
- Initialize every direct instance field, final or mutable, exactly once under
  the existing ordinary and copy-constructor rules. Preserve straight-line
  incomplete-object bodies, base-first construction, and declaration-order
  synthesized copying.
- Reject every independent post-construction replacement of a final instance
  field. Mutable receiver access, declaring-class privacy, a destructor, a
  helper, inheritance, or nesting does not by itself authorize a write.
- Preserve assignment of mutable complete class values, including classes
  whose representation contains final fields. Keep destination and source
  evaluation, self-assignment, selected operation, live lifetime, failure,
  and cleanup unchanged.
- Permit only the exact declaring class's selected user or synthesized copy
  assignment to update its own direct final fields. User assignment retains
  locals, control flow, calls, zero or repeated writes, arbitrary supported
  sources, and arbitrary supported subsets of state.
- Do not propagate final-write authorization into helper calls, base or nested
  projections, derived lifecycle bodies, ordinary methods, destructors, or
  unrelated callables. Base and nested complete assignment must invoke their
  own selected lifecycle.
- Require every final static field to have one explicit eager initializer.
  Generated publication is its sole root write; reject zero-default final
  statics and every later source assignment while retaining normal reverse
  shutdown.
- Keep finality shallow. It pins one field or static slot without recursively
  freezing inline state, array elements, optional payload state, or separately
  allocated shared pointees reachable through existing mutable access.
- Preserve ordinary reads, visibility, produced read-only roots, call-scoped
  aliases, optional guards, shared-owner and detached-array anchors, generic
  specialization, inheritance, dispatch, function values, and module
  qualification.
- Carry explicit final metadata and exact complete-value-assignment evidence
  through typed HIR and MIR. Preliminary and final verification must reject
  forged marker, endpoint, owner, directness, family, liveness, initialization,
  guard, anchor, ownership, and cleanup facts.
- Reuse ordinary target addressing, copy, assignment, static publication,
  destruction, and cleanup. Add no wrapper type, target instruction, runtime
  state, public symbol, runtime service, or ABI revision.
- Exclude immutable local storage or `let`, final parameters/results/elements,
  final classes or methods, deep constness, instance declaration initializers,
  lazy or concurrent final storage, and optimizer guarantees.

## Progress

- [x] FFI0 — Represent contextual final declarations
- [x] FFI1 — Enforce instance construction and direct-write semantics
- [x] FFI2 — Authorize exact complete-value assignment
- [ ] FFI3 — Integrate final statics with eager lifecycle
- [ ] FFI4 — Prove shallow cross-feature composition
- [ ] FFI5 — Adopt public final primitive-box payloads and close

Every Rust implementation task runs focused owner tests followed by
`make check`, `make msrv-check`, and `git diff --check`. The closing task also
runs the repository's extended determinism, robustness, and clean-snapshot
coverage through the documented Makefile interfaces. This roadmap adds no
repository CI.

## PR-sized implementation sequence

### FFI0 — Represent contextual final declarations

**Purpose:** Establish exact source, identity, and cross-phase metadata before
any final write restriction or exceptional assignment authorization can depend
on it.

- [x] Extend class-member parsing with one cohesive modifier classifier for
      ordinary, private-cell, final instance, static, and final-static fields.
      Keep the parser facade concise; if modifier lookahead and recovery become
      a substantial independent responsibility, give them a descriptive
      private submodule rather than growing one monolithic class parser.
- [x] Accept only the four canonical final forms and preserve contextual
      declarations named `final`, `static`, `private`, and `cell`. Diagnose
      reordering, duplicates, missing names/types/punctuation, final/cell
      combinations, unsupported declaration categories, instance
      initializers, and initializer-free final statics at exact spans.
- [x] Add final marker and modifier-span metadata to instance and static AST,
      resolved, HIR, and MIR declarations without changing ordinary IDs,
      visibility, stored types, declaration order, initializer identity, or
      class layout inputs.
- [x] Preserve the marker through generic template analysis and closed
      specialization, including per-application static identities and
      deterministic source/resolved/HIR/MIR dumps.
- [x] Extend declaration consistency and MIR verification to reject missing,
      mismatched, empty, out-of-declaration, or cross-kind final metadata.
      Do not infer finality from names or visibility.
- [x] Put complete compilation of final-bearing programs behind one explicit
      phase-owned executable gate. Parsing and metadata alone must never make
      ignored finality executable; all non-final programs remain unchanged.
- [x] Update living documentation only for declaration and representation
      portions that actually ship in this task, retaining frozen or staged
      status for executable semantics.

**Tests:** Parser success, contextual disambiguation, canonical-order,
recovery, exact-span, and syntax-dump tests; resolution identity, collision,
privacy, inheritance, template, specialization, and dump tests; HIR/MIR
declaration lowering and verifier mutation tests; unchanged layout and static
identity regressions; explicit executable-gate coverage; then the documented
complete Rust-task gates.

**Exit criteria:** Both final declaration kinds retain deterministic ordinary
identity plus exact final evidence through verified declaration products,
malformed forms stop with focused diagnostics, non-final behavior is
unchanged, and no final-bearing program can execute before write semantics
exist.

Completed 2026-08-16. Focused syntax, resolution, specialization, HIR/MIR,
verification, layout, and driver-gate tests, `make check`, `make msrv-check`,
documentation validation, and `git diff --check` passed.

### FFI1 — Enforce instance construction and direct-write semantics

**Purpose:** Make final instance fields executable for construction and reads
while rejecting every independent post-construction slot replacement.

- [x] Centralize the instance-field write decision around declaration
      finality, current lifecycle kind, exact endpoint, and construction state.
      Preserve declaring-class privacy diagnostic precedence and the existing
      private-cell decision as a separate capability.
- [x] Treat ordinary and copy-constructor writes to the exact class's direct
      final fields as ordinary incomplete-storage initialization. Reuse the
      current exact-once, direct-`self`, base-first, no-control-flow, definite-
      initialization, type, ownership, and cleanup rules.
- [x] Preserve synthesized copy construction and construction capability for
      final primitive, class, optional, shared-owner, array, and function-
      valued fields whenever the equivalent mutable field supports it.
- [x] Reject direct final-field replacement in ordinary methods, mutable
      methods, static methods through explicit objects, helpers, destructors,
      derived and unrelated bodies, and the declaring class outside its exact
      copy-assignment lifecycle. Cover public and private fields uniformly.
- [x] Reject direct inherited and nested final-field destinations while
      preserving reads, read-only aliases, produced-field reads, and shallow
      mutation of nested state wherever ordinary access already permits it.
- [x] Recognize an exact user or synthesized copy-assignment write as a
      deferred lifecycle candidate rather than misdiagnosing it as an ordinary
      direct write. Keep that candidate behind the operation-owned executable
      gate until FFI2 defines and verifies its durable authorization.
- [x] Lower accepted construction and reads through ordinary HIR/MIR/backend
      paths. Keep whole-object assignment involving final-bearing classes
      behind a narrow operation-owned gate until FFI2 provides independently
      verifiable final-update evidence.
- [x] Add deterministic diagnostics and dumps for declaration finality,
      construction initialization, rejected direct writes, and the temporary
      complete-assignment gate.

**Tests:** Focused type-checking, HIR, MIR, verifier, layout, and backend tests
for every stored family; ordinary/copy/synthesized construction; exact-once and
base ownership; public/private direct-write failures from every callable kind;
read and shallow nested-mutation preservation; minimal native construction and
read goldens; whole-assignment gate regression; then the complete Rust-task
gates.

**Exit criteria:** Final instance fields construct, copy-construct, read, and
destroy exactly like ordinary fields, every independent post-construction
replacement is rejected at its semantic owner, no layout/runtime change is
present, and unverifiable complete assignment remains non-executable.

Completed 2026-08-16. Centralized typed field-write policy now gives finality
precedence after privacy, preserves construction and private-cell capabilities,
rejects independent replacement with `TYP043`, and records exact user
copy-assignment candidates without making them executable. Final instance
construction, synthesized and user copy construction, reads, shallow nested
mutation, destruction, unchanged layout, and native emission pass through the
ordinary pipeline. Recursive complete-value assignment is temporarily stopped
after verified MIR with `MIR003`; final statics remain independently gated by
`MIR002`. Focused type-checking, HIR/MIR verification, driver, layout, and
native tests plus the documented complete Rust-task gates passed.

### FFI2 — Authorize exact complete-value assignment

**Purpose:** Preserve mutable complete-value replacement by giving only the
selected exact copy-assignment lifecycle explicit, independently verified
permission to update its own direct final representation.

- [x] Extend the centralized typed field-write authorization with one exact
      declaring-class final-assignment reason. Require the current callable to
      be that class's selected user copy assignment and the destination to end
      at one of its direct final fields.
- [x] Preserve the full existing user `assign(ref source: T)` body model:
      locals, nested blocks, conditionals, loops, calls, returns, zero or
      repeated final writes, arbitrary supported source expressions, and any
      supported subset of direct fields.
- [x] Keep permission lexical and exact. Reject final writes in helpers called
      by assignment, derived assignment bodies targeting inherited final
      state, direct writes into nested final state, and ordinary methods or
      destructors. Complete base and nested field assignment must select their
      respective lifecycle operations.
- [x] Add equivalent explicit evidence to synthesized assignment capability
      plans for final direct fields while preserving base-first and declaration-
      order processing, selected nested operations, capability availability,
      self-assignment, and lifecycle-visible effects.
- [x] Carry user and synthesized final-update evidence through every scalar,
      exact-class, optional, shared-owner, and array HIR/MIR assignment carrier.
      Do not clear the final marker or upgrade an entire receiver/class body.
- [x] Extend preliminary and final MIR verification to prove exact field,
      lifecycle owner, directness, selected operation, assignment family,
      liveness, guards, anchors, ownership transitions, displacement, failure,
      and cleanup. Add mutation tests for every forged or over-broad case.
- [x] Remove the complete-assignment gate and execute local, parameter, mutable
      field, mutable static, array-element, optional-payload, and other existing
      complete class destinations through the unchanged whole-object source
      contract.

**Tests:** User-assignment type/HIR/MIR tests with zero, one, repeated,
conditional, and loop-carried final writes; helper/inherited/nested rejection;
synthesized plan and capability tests for every stored family and unavailable
nested operation; preliminary/final verifier mutations; fresh-source and
self-assignment native goldens; outer and nested complete replacement; failure
and destruction order; then the complete Rust-task gates.

**Exit criteria:** Every mutable complete class destination remains assignable
when its selected operation is available, final representation changes only
under exact verified user or synthesized lifecycle evidence, neighboring
writes remain rejected, and the temporary gate is removed.

Completed 2026-08-16. Typed HIR now grants final replacement only to the exact
selected declaring-class user copy assignment, while synthesized assignment
plans carry the exact ordered set of direct final fields. Separate final-update
evidence crosses every scalar, class, optional, shared-owner, and array MIR
carrier without widening receiver access or conflating private-cell permission.
Preliminary and final verification prove the endpoint, direct receiver,
selected operation, family, declaration plan, and all existing lifetime,
guard, anchor, displacement, failure, and cleanup invariants. The `MIR003`
gate and its recursive classifier are removed; ordinary whole-value lowering
now executes direct, inherited, nested, fresh-source, and self-assignment cases.
Focused HIR/MIR mutation tests, backend-native tests, failure and destruction
goldens, `make check`, `make msrv-check`, and `git diff --check` passed.

### FFI3 — Integrate final statics with eager lifecycle

**Purpose:** Add immutable-after-publication class-owned roots without
weakening the existing effect-certified eager initialization and reverse
shutdown pipeline.

- [ ] Require one explicit initializer for every final static declaration and
      reject the zero-default route at the declaration's final or missing-
      initializer span. Preserve all existing explicitly initialized stored
      types, expression selection, privacy, ownership, and full-expression
      semantics.
- [ ] Reject every source assignment whose root is a final static, including
      assignment from declaring-class bodies and every scalar, class,
      optional, shared-owner, array, and function-value carrier. Preserve
      reads, `ref` borrowing, and shallow nested mutation available through the
      equivalent initialized mutable static.
- [ ] Carry final-static metadata through preliminary lifecycle definitions,
      effect extraction, dependency solving, deterministic plan schema,
      certificates, coordinator synthesis, final MIR, and dumps.
- [ ] Independently verify that a final static has exactly one explicit
      planned publication, no zero-default publication, no later root write,
      correct ownership/effects/dependencies, and ordinary reverse shutdown.
      Distinguish source assignment from destruction or release.
- [ ] Preserve one target-private writable slot used by generated startup and
      shutdown code. Add no read guard, read-only section requirement, exported
      symbol, runtime flag, write barrier, or ABI change.
- [ ] Cover inherited selection and exact closed-generic application-owned
      final statics without duplicating storage or final metadata.

**Tests:** Syntax and missing-initializer diagnostics; type/HIR failures for
every root assignment carrier and declaring context; explicit initializer
matrix; preliminary/planned/final lifecycle dumps; effect, plan, certificate,
publication, ownership, and shutdown verifier mutations; generic/inherited
identity and dependency cycles; backend symbol/layout/order checks; native
read, shallow mutation, dependency, and cleanup goldens; then the complete
Rust-task gates.

**Exit criteria:** Every final static is explicitly and eagerly published once,
cannot be source-reassigned afterward, participates normally in certified
dependency ordering and reverse cleanup, and changes neither static layout nor
the runtime ABI.

### FFI4 — Prove shallow cross-feature composition

**Purpose:** Close composition gaps after the core instance, assignment, and
static contracts execute, with particular attention to alias safety and exact
ownership of exceptional writes.

- [ ] Cover final fields across direct bases, inherited reads, derived write
      rejection, base lifecycle delegation, checked class views, produced
      read-only roots, virtual/interface calls, and eligible capture-free
      function values.
- [ ] Cover generic final instance/static fields, contextual requirements,
      repeated specialization, generic bases, optional/array/shared/function
      substitutions, module qualification, imports, and deterministic IDs and
      symbols.
- [ ] Prove shallow behavior for final inline classes, arrays, optionals,
      shared owners, function values, and nested final-bearing values. Preserve
      every ordinary nested mutation and reject every slot-rebinding alias or
      projection not already authorized.
- [ ] Exercise active optional guards, shared-owner anchors, detached-array
      anchors, overlapping paths, produced sources, self-assignment, last-owner
      destruction, displacement, failure, and full-expression cleanup during
      enclosing complete-value replacement.
- [ ] Audit all field/static assignment carriers, lifecycle capability
      builders, specialization copies, access-propagating consumers, static
      effect walkers, MIR verifiers, and backend inputs for missing final
      evidence or accidental permission widening.
- [ ] Complete syntax/resolved/HIR/preliminary-MIR/planned-MIR/final-MIR dumps,
      malformed-product tests, diagnostics, native success/failure matrices,
      cross-process determinism, and frontend robustness.
- [ ] Promote only behavior that is fully implemented at this stage. Record
      lower-priority actionable findings in a separately indexed final-fields
      discoveries document rather than expanding this task.

**Tests:** Complete focused phase and verifier matrices; successful and failing
native goldens across inheritance, dispatch, generics, ownership, aliases,
produced values, nested state, modules, and statics; deterministic diagnostics,
phase products, assembly, output, status, and robustness; then the complete
Rust-task gates.

**Exit criteria:** The frozen final-field contract composes with every current
field/static storage and object-access family, aliases and owning state remain
lifetime-safe, all permission boundaries are explicit and independently
verified, and no scalar-only or raw-store shortcut remains.

### FFI5 — Adopt public final primitive-box payloads and close

**Purpose:** Deliver the motivating zero-getter standard-library surface and
publish final fields as one complete supported language feature.

- [ ] Change `BoxF64`, `BoxI64`, `BoxU64`, `BoxU8`, and `BoxBool` from private
      payload fields to public final `value` fields. Update their constructors,
      exact equality, and domain-separated mixed hashes without adding getters,
      compiler exceptions, implicit boxing, or runtime support.
- [ ] Add native standard-library coverage for direct payload reads, rejected
      writes, construction, copies, mutable complete box assignment, interface
      dispatch, generic `Equatable`/`Hashable` use, exact boundary equality,
      and unchanged per-class hash domains.
- [ ] Complete modifier diagnostics, dump determinism, layout/ABI regressions,
      standard-library resolution, and source-to-native documentation examples.
- [ ] Promote grammar, class lifecycle, static fields, compiler phases,
      compiler overview, testing, debugging, backend/runtime boundary, standard
      library, and status documentation from frozen planned behavior to the
      implemented contract. Remove rollout language and task codes from living
      code, tests, and documentation.
- [ ] Confirm the exclusions remain explicit: no local `let`, final
      parameters/results/elements/classes/methods, deep constness, combined
      final cells, runtime immutability service, concurrency semantics,
      optimizer promise, public runtime symbol, or ABI revision.
- [ ] Run closure review from an artifact-free snapshot, resolve high-priority
      maintainability findings within the feature boundary, and place any
      lower-priority actionable findings in the separately indexed discoveries
      document.
- [ ] Mark the roadmap complete, move it and the frozen proposal to the
      archive, update both indexes and every incoming link, and retain only
      current behavior in living documentation.

**Tests:** Focused primitive-box and final-field phase/native matrices;
documentation link and index validation; independent-process compiler and
full-pipeline determinism; malformed-source robustness; `make check`,
`make check-long`, `make msrv-check`, and `git diff --check` from the documented
clean-snapshot workflow.

**Exit criteria:** All five primitive boxes expose a public final payload with
unchanged equality and hashing, final instance and static fields are a complete
documented source-to-native contract, all exclusions remain enforced, all
repository gates pass, and the roadmap and design record are archived.

## Ordering and dependencies

FFI0 establishes stable declaration evidence and deliberately gates execution
before ignored finality could become observable. FFI1 reuses the existing
incomplete-object boundary to make construction and reads executable while
keeping complete assignment gated. FFI2 then introduces the only exceptional
post-construction instance writes and proves them for both user and synthesized
lifecycle. FFI3 applies the separate no-enclosing-value rule to static roots
inside the existing certified eager lifecycle. FFI4 broadens proof across
ownership, aliases, dispatch, generics, and modules only after each core write
kind is independently verifiable. FFI5 adopts the feature in ordinary standard
library source and closes documentation and validation.

The roadmap depends on the implemented contextual private/private-cell/static
parser, declaring-class privacy, exact-once construction, separate copy
capabilities, mutable complete-value assignment, generic specialization,
produced reads, call-scoped aliases, optional presence guards, shared-owner and
array-backing anchors, verified MIR, certified static lifecycle, x86-64
backend, and primitive-box contracts. No active roadmap blocks FFI0.
