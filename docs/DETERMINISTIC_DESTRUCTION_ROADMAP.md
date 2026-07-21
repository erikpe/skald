# Deterministic Destruction Roadmap

Status: in progress; DD0–DD4 are complete and DD5 is next.

This roadmap adds observable deterministic destruction to Skald's existing
local-only inline-object model. It is deliberately limited to normal control
flow, owning local objects, and their recursively contained inline fields. It
does not add copying, object values, exceptions, inheritance, shared ownership,
or user-invoked early destruction.

The completed slice should make cleanup order directly observable:

```ska
extern fn ska_rt_println_i64(value: i64) -> unit;

class Leaf {
    tag: i64;

    init(tag: i64) {
        self.tag = tag;
    }

    destroy {
        ska_rt_println_i64(self.tag);
    }
}

class Pair {
    left: Leaf;
    right: Leaf;

    init(left: i64, right: i64) {
        self.left = Leaf(left);
        self.right = Leaf(right);
    }
}

fn main() -> i64 {
    var outer: Pair = Pair(1, 2);
    {
        var inner: Leaf = Leaf(3);
    }
    return 0;
}
```

The required output is `3`, `2`, `1`: the nested scope ends first, then
`Pair`'s inline fields are destroyed in reverse declaration order.

## 1. Scope and semantic boundary

The first destruction profile includes:

- at most one contextual `destroy { ... }` declaration per class;
- an implicit mutable `self`, no parameters, and an implicit `unit` result;
- an empty user destruction body when no declaration is present;
- automatic cleanup of successfully initialized owning object locals when
  their lexical storage scope ends through fallthrough or `return`;
- evaluation of a return value before cleanup begins;
- reverse successful-initialization order for locals in one scope;
- a class's user destruction body before its class-typed fields, followed by
  class-typed fields in reverse declaration order;
- recursive cleanup of acyclic inline subobjects without copying them;
- exactly-once cleanup across nested blocks and conditional control flow;
- explicit target-independent cleanup order in MIR and structured verification;
- deterministic x86-64 execution and diagnostics.

The profile continues to exclude:

- explicit `destroy` expressions or statements and early lifetime termination;
- object assignment, copying, object parameters/results, object temporaries,
  and aggregate ABI classification;
- exceptions, panic unwinding, failed-construction cleanup, and cleanup pads;
- loops, `break`, `continue`, and other control-flow forms not yet implemented;
- inheritance, base-subobject destruction, virtual dispatch, and dynamic type
  metadata;
- `shared`, allocation, reference counting, deallocation, and borrow anchors;
- arrays, optionals, statics, globals, and cross-module initialization order;
- destructor overloading, parameters, explicit results, and throwing
  destructors.

Only normal exits exist in the implemented language. The design must leave a
clear extension point for exceptional and partial-construction cleanup, but it
must not invent those semantics in this slice.

## 2. Architectural contract

1. Syntax recognizes `destroy` only as a contextual class-member introducer;
   the spelling remains available as an ordinary identifier elsewhere.
2. Resolution assigns a stable lifecycle identity. Later phases never select a
   destruction body by source name.
3. HIR records the destructor body, implicit receiver capability, lexical
   owning scopes, and all source-level legality decisions.
4. MIR makes cleanup operations and their order explicit. A backend must not
   reconstruct lexical lifetimes or language destruction order.
5. Cleanup planning tracks initialized owning places, not class values. Class
   objects remain place-only throughout MIR.
6. Recursive field cleanup uses semantic `ClassId` and `FieldId` identities.
   Target offsets remain a backend responsibility.
7. Every cleanup operation is verified for place ownership, liveness, class
   type, access, and exactly-once placement on each normal exit.
8. The x86-64 backend lowers verified cleanup mechanically through the existing
   receiver/place machinery and retains structured malformed-MIR defenses.
9. Alias parameters remain non-owning. Ending an alias parameter's scope never
   destroys its referent.
10. Primitive locals and fields require no cleanup operation.

Prefer one target-independent cleanup planner shared by block fallthrough and
return lowering. Avoid parallel special cases in the type checker, MIR builder,
and backend.

## 3. Cleanup order and state

For a completed object, destruction is:

1. execute the class's user `destroy` body, if declared;
2. destroy class-typed fields in reverse declaration order;
3. finish the object's lifetime without deallocation.

During the user body, `self` and every completed field remain live. Field
cleanup starts only after the body returns. Once field cleanup starts, source
code cannot observe the partially destroyed object.

Within a lexical scope, owning locals are registered when their direct
construction completes, then cleaned up in reverse registration order. An
unexecuted conditional arm registers nothing. Every normal edge leaving one or
more scopes emits the applicable cleanup sequence exactly once. For `return`,
the primitive result is evaluated and preserved before any cleanup body runs.

This is initialized-place state, not a general ownership analysis. The current
language has no source operation that moves, copies, replaces, or explicitly
ends an owning object after initialization.

## 4. PR-sized implementation sequence

Each task should be independently reviewable and keep the compiler building.

### DD0 — Freeze the executable destruction profile

**Purpose:** Settle observable behavior before adding syntax or IR.

- [x] Reconcile the draft specification's broad destruction design with the
      local-only, no-exception compiler profile above.
- [x] Freeze contextual syntax, duplicate-declaration behavior, receiver
      capability, allowed statements, and diagnostic vocabulary.
- [x] Freeze local, nested-scope, conditional, and return cleanup order.
- [x] Freeze body-before-fields and reverse-field order for inline containment.
- [x] State precisely when construction registers an owning place for cleanup.
- [x] Record all exclusions, especially copy/value, failed construction,
      exceptions, inheritance, shared ownership, and explicit early destroy.
- [x] Add parser-facing grammar examples and cross-document consistency tests
      or assertions where useful.

**Tests:** Specification/grammar review; parser fixtures for the frozen syntax
shape and contextual-keyword behavior may be added without accepting the
feature semantically.

**Acceptance criteria:** Every later task can implement syntax, state, order,
and failure behavior without making a new language-design decision.

### DD1 — Parse and resolve destruction members

**Purpose:** Introduce stable source and identity representations without
making destruction executable yet.

- [x] Parse `destroy { ... }` as a dedicated class member with a complete span.
- [x] Preserve `destroy` as an ordinary identifier outside the contextual form.
- [x] Recover cleanly from parameters, result annotations, missing braces, and
      other malformed destruction declarations.
- [x] Reject duplicate destruction members deterministically.
- [x] Assign an owner-qualified lifecycle identity during resolution.
- [x] Resolve destructor bodies with an implicit mutable `self` and the normal
      class-member namespace.
- [x] Extend exact AST/resolved dumps and keep declaration iteration stable.

**Tests:** Syntax recovery, contextual spelling, duplicates, member lookup,
forward declarations, identity ownership, and deterministic exact dumps.

**Acceptance criteria:** Valid destruction declarations cross resolution by
stable identity; malformed forms produce focused diagnostics and never leak
source-name lookup into later phases.

### DD2 — Type-check destructor bodies and represent them in HIR

**Purpose:** Complete all source-level destructor legality and access decisions.

- [x] Add the dedicated HIR lifecycle member and implicit mutable receiver.
- [x] Type-check destructor bodies with the existing field, method, call, alias,
      conditional, block, and return rules.
- [x] Require an implicit `unit` result and reject value returns.
- [x] Keep the complete object live throughout the user destruction body.
- [x] Reject direct construction into already-live fields and all object-value
      or explicit-destroy forms retained outside the profile.
- [x] Reuse the existing receiver/access vocabulary rather than creating a
      destructor-only mutability path.
- [x] Extend deterministic HIR dumps and focused type-checker diagnostics.

**Tests:** Mutable/read-only access, nested fields, receiver calls, aliases,
conditionals, returns, excluded construction/value forms, and exact HIR dumps.

**Acceptance criteria:** HIR fully describes each valid destruction body by
stable IDs, types, places, and access; invalid source cannot reach MIR.

### DD3 — Add verified target-independent cleanup operations

**Purpose:** Give MIR an explicit, maintainable representation of object
lifetime ends before inserting cleanup on source control-flow edges.

- [x] Define one MIR cleanup operation over a typed semantic object place.
- [x] Represent user destruction bodies and recursive field cleanup without
      embedding target offsets or backend layout decisions.
- [x] Make body-before-fields and reverse-field order explicit in MIR or a
      target-independent generated cleanup body.
- [x] Extend the shared place walker to verify cleanup bases and projections.
- [x] Reject wrong-class, non-owning, read-only, dead, duplicated, foreign, or
      scalar cleanup targets structurally.
- [x] Preserve the no-class-`MirValue` invariant.
- [x] Extend exact MIR dumps and pass-pipeline verification.

**Tests:** Hand-built valid cleanup MIR, verifier mutations for every invariant,
deep projected fields, empty classes, and deterministic exact dumps.

**Acceptance criteria:** MIR can express and verify one complete destruction
sequence while remaining target-independent and place-only.

### DD4 — Plan cleanup for scopes and normal exits

**Purpose:** Insert each cleanup exactly once on every implemented normal
control-flow path.

- [x] Track successfully initialized owning locals per lexical scope during
      lowering.
- [x] Emit reverse-order cleanup on ordinary block and function fallthrough.
- [x] Emit cleanup for every exited scope on `return`.
- [x] Evaluate and preserve primitive return values before cleanup execution.
- [x] Handle nested and conditional scopes without cleaning unexecuted locals
      or duplicating cleanup at joins.
- [x] Leave primitive locals and non-owning alias parameters out of cleanup.
- [x] Centralize edge cleanup planning so later loops and exceptions can extend
      the same state model.
- [x] Verify cleanup placement and initialized-place transitions.

**Tests:** Multiple locals, nested blocks, each conditional arm, early return,
return expressions with effects, empty scopes, aliases, and exact MIR order.

**Acceptance criteria:** Every implemented normal exit destroys precisely the
owning objects whose construction completed, in the frozen order, once.

### DD5 — Lower deterministic destruction on x86-64

**Purpose:** Make verified cleanup observable without moving language lifetime
rules into the backend.

- [ ] Lower cleanup through the existing local, receiver, and projected-place
      address machinery.
- [ ] Call user destruction bodies with the existing hidden-receiver ABI.
- [ ] Execute recursive field cleanup in the order already represented by MIR.
- [ ] Preserve live primitive return values across cleanup calls.
- [ ] Support empty, padded, mixed, nested, and forward-declared class layouts.
- [ ] Extend frame/call planning without aggregate copying or deallocation.
- [ ] Retain structured errors for malformed cleanup MIR and incomplete or
      recursive class metadata.
- [ ] Keep generated symbols and assembly deterministic.

**Tests:** Assembly snapshots, register preservation, deep place addressing,
layout cases, assembler acceptance, malformed MIR, and native execution.

**Acceptance criteria:** Verified destruction MIR lowers mechanically to correct
deterministic Linux x86-64 code with no backend-owned lifetime inference.

### DD6 — Harden, document, and publish destruction

**Purpose:** Make deterministic destruction a dependable public feature and
prepare the object-value/copy roadmap.

- [ ] Add native goldens for body/field/local order, nesting, conditionals,
      fallthrough, early return, return-value evaluation, aliases, empty
      objects, padding, and classes without user bodies.
- [ ] Add compile-failure goldens for malformed/duplicate declarations, invalid
      returns, excluded calls or construction, and access/type violations.
- [ ] Assert exact stdout, exit status, empty runtime stderr, deterministic
      assembly, and deterministic diagnostics across compiler processes.
- [ ] Audit source-reachable assertions and backend cleanup assumptions.
- [ ] Update grammar, specification, architecture, README, debugging, golden
      documentation, samples where useful, and future boundaries.
- [ ] Retain explicit exclusions for copying, values, exceptions, inheritance,
      shared ownership, arrays, and early destruction.
- [ ] Run the complete quality gate and resolve warnings or nondeterminism.
- [ ] Mark this roadmap complete, archive it, update the archive index, and make
      object copy/value semantics the next active object-model roadmap.

**Tests:** Full `make check`, including compiler tests, cross-process
determinism, goldens, native assembler/linker execution, and runtime tests.

**Acceptance criteria:** Destruction is observable, deterministic, exactly
once on every supported normal exit, structurally verified, fully documented,
and introduces none of the deferred ownership mechanisms.

## 5. Required quality gates

- [x] `cargo fmt --all -- --check`
- [x] `cargo check --workspace --all-targets`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace`
- [x] `make runtime-test`
- [x] `make golden-test`
- [x] `make check`
- [x] Deterministic AST, resolved, HIR, MIR, assembly, and diagnostics
- [x] Cleanup order is explicit above the backend boundary
- [x] No source-name lookup below resolution
- [x] No target layout or ABI location in HIR/MIR
- [x] No class object represented as a scalar MIR value
- [x] No copy, exception, inheritance, shared, allocation, or anchor semantics
      introduced accidentally
- [x] Touched Rust modules retain concise facades and cohesive ownership
- [x] Living documentation and milestone checkboxes match behavior

## 6. Completion gate

The slice is complete when:

- [ ] all DD0–DD6 tasks and acceptance criteria are complete;
- [x] destructor declarations have stable identities and typed mutable bodies;
- [ ] complete objects run their body then nested fields in frozen reverse order;
- [ ] owning locals are destroyed once in reverse initialization order;
- [x] nested scopes, conditionals, fallthrough, and return plan cleanup correctly;
- [ ] return expressions are evaluated before cleanup and their values survive;
- [x] aliases remain non-owning and primitives require no cleanup;
- [x] MIR explicitly represents and verifies cleanup over semantic places;
- [ ] the backend lowers cleanup without inferring lexical lifetime rules;
- [ ] malformed source and MIR fail structurally rather than panicking;
- [ ] observable output and compiler artifacts are deterministic;
- [ ] full quality gates pass and living documentation matches the compiler.

The following object-model roadmap should add copy construction, assignment,
object parameters/results, return storage, temporaries, and permitted elision
only after this destruction contract is complete.
