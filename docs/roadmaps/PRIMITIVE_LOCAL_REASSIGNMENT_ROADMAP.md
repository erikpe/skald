# Primitive Local Reassignment Roadmap

Status: planned; PLR0 is next.

Primitive locals are already initialized, read, shadowed, and carried through
typed HIR and verified MIR, while the assignment grammar and scalar MIR store
operation already exist for other destinations. This roadmap makes an
initialized primitive `var` binding replaceable without extending the
language's place model or creating a second backend store path.

## Scope and invariants

- Permit `local = expression;` for initialized `var` bindings of type `i64`,
  `u64`, `u8`, `f64`, or `bool`.
- Treat grouping around the complete destination as semantically transparent,
  so `(local) = expression;` targets the same resolved local identity.
- Require the right-hand expression to have exactly the local's declared type;
  reassignment performs no inference, promotion, conversion, or truthiness
  coercion.
- Resolve the destination before checking the source, evaluate the source
  exactly once, store only after successful source evaluation, and end the
  full expression after the store so source temporaries retain their existing
  cleanup timing.
- Keep every `var` local definitely initialized by its declaration.
  Reassignment changes neither local liveness nor cleanup registration, and
  primitives acquire no ownership or destruction behavior.
- Preserve lexical binding identity, declaration-before-use, nested shadowing,
  duplicate-binding rejection, deterministic diagnostics, and deterministic
  AST, resolved-IR, HIR, MIR, and assembly dumps.
- Reuse `MirStore`, its exact scalar-type and mutable-place verification, and
  the existing x86-64 scalar store selection. Add no runtime entry point, ABI
  rule, layout rule, or target-specific semantic choice.
- Keep primitive value-parameter rebinding, alias locals, assignment
  expressions, chained or compound assignment, destructuring, increment and
  decrement operators, and new field, array, optional, object, or shared-owner
  behavior out of scope.
- Preserve initializer-body restrictions: primitive local reassignment does
  not become legal inside bodies that admit only direct receiver-field
  initialization.

## Progress

- [ ] PLR0 — Freeze the primitive local reassignment contract
- [ ] PLR1 — Implement and execute primitive local reassignment

## PR-sized implementation sequence

### PLR0 — Freeze the primitive local reassignment contract

**Purpose:** Settle the source-visible rule and its phase ownership before
introducing a new resolved and typed statement category.

- [ ] Specify primitive `var` reassignment in the bindings, statement,
      exact-type, and evaluation-order language contracts, including grouped
      destinations, lexical shadowing, evaluate-once behavior, and the absence
      of a produced value.
- [ ] Record the rule as a frozen design in the status matrix while retaining
      the current implementation boundary until execution lands.
- [ ] Confirm that the existing `place = expression;` grammar and AST
      assignment shape cover an identifier or grouped identifier without a
      syntax extension; clarify documentation or parser diagnostics where
      wording incorrectly implies that every primitive-local-shaped statement
      is rejected syntactically.
- [ ] Document phase ownership: resolution classifies a primitive `var`
      destination by `LocalId`; type checking selects an exact primitive
      assignment; HIR preserves the selected destination and typed source; MIR
      evaluates the source then emits a verified store; targets consume that
      store mechanically.
- [ ] State explicit exclusions for primitive value parameters, invalid or
      non-local roots, compound and expression-valued assignment, and all
      existing non-primitive assignment families so later implementation does
      not broaden the place model accidentally.

**Tests:** Documentation link/index checks through `make docs-check`, followed
by `make check` and `git diff --check`.

**Exit criteria:** Living language and compiler documentation contains one
implementation-ready contract with no unresolved representation, evaluation,
diagnostic, or scope decision; the status matrix identifies the design as
frozen but not yet implemented.

### PLR1 — Implement and execute primitive local reassignment

**Purpose:** Carry the frozen operation through semantic classification,
typed HIR, verified MIR, and native execution using the existing scalar store
pipeline.

- [ ] Classify identifier and grouped-identifier assignment destinations during
      resolution. Emit a dedicated resolved primitive-local assignment only
      for a primitive `LocalId`, preserve the exact declaration selected under
      shadowing, resolve the source independently for diagnostic recovery, and
      leave every existing object, shared, optional, array, field, and
      whole-pointee classification unchanged.
- [ ] Diagnose primitive parameter assignment and other excluded roots at
      their owning phase without falling through to misleading object-place
      diagnostics.
- [ ] Type-check all five primitive destination types with the ordinary exact
      type requirement. Add a dedicated HIR statement carrying the destination
      binding, typed source expression, type, and source span; expose it in
      deterministic HIR dumps without retaining source names or backend
      details.
- [ ] Lower the typed statement by evaluating its source once, emitting
      `MirStore` to the local's existing storage, and ending the full
      expression after the store. Reuse the current MIR place/type verifier,
      dump format, x86-64 frame layout, byte canonicalization, floating-point
      movement, and scalar store selection; strengthen verifier or target
      tests only where the new source path reveals an uncovered invariant.
- [ ] Add focused syntax/regression, resolution, type-check, HIR-dump,
      MIR-lowering/verifier, and x86-64 tests for every primitive type,
      grouped destinations, repeated writes, nested shadowing, calls and
      expression sources, source-before-store ordering, and cleanup after a
      source expression with temporaries.
- [ ] Add source-to-native golden coverage that observes reassigned values for
      `i64`, `u64`, `u8`, `f64`, and `bool`, including reassignment across
      conditional control flow and canonical `u8`/`bool` storage. Add exact
      rejection coverage for type mismatches and excluded primitive parameter
      rebinding where complete rendered diagnostics are part of the contract.
- [ ] Update the status matrix and living language/compiler/testing
      documentation from frozen direction to implemented behavior. Remove
      stale claims that primitive local reassignment is unsupported and avoid
      roadmap task codes in code, tests, or living documentation.

**Tests:** Focused syntax, resolver, type-check, HIR dump, MIR lowering and
verification, backend, and golden tests while iterating; then `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** Every supported primitive `var` can be reassigned with an
exactly typed source and the new value is observable on Linux x86-64; malformed
or excluded forms fail deterministically before HIR; MIR verification rejects
wrong-type or non-mutable stores; all ordinary and MSRV gates pass; and living
documentation describes reassignment as implemented.

## Ordering and dependencies

PLR0 comes first because the local-versus-parameter boundary, exact-type rule,
evaluation and cleanup order, and phase ownership determine the semantic IR
shape. PLR1 then implements that closed contract as one vertical compiler
slice. It depends only on existing lexical binding identities, typed scalar
expressions, local MIR storage, `MirStore` verification, and x86-64 scalar
stores; it has no dependency on another active roadmap and introduces no
runtime or ABI work.
