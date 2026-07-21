# Alias Parameters Roadmap

Status: complete; AL0–AL7 implemented.

This roadmap adds Skald's first non-owning bindings: `ref` and `mut ref`
parameters over the inline class places the compiler already supports. The
slice is intentionally call-scoped and non-storable. It provides cheap object
arguments without requiring object copying, destruction, aggregate ABI rules,
general lifetime inference, or a borrow checker.

The completed slice should compile programs such as:

```ska
extern fn ska_rt_println_i64(value: i64) -> unit;

class Counter {
    value: i64;

    init(value: i64) {
        self.value = value;
    }

    mut fn add(amount: i64) -> unit {
        self.value = self.value + amount;
    }

    fn get() -> i64 {
        return self.value;
    }

    mut fn add_from(mut ref other: Counter) -> unit {
        other.add(self.value);
    }
}

fn inspect(ref counter: Counter) -> i64 {
    return counter.get();
}

fn increment(mut ref counter: Counter, amount: i64) -> unit {
    counter.add(amount);
}

fn forward(mut ref counter: Counter) -> unit {
    increment(counter, 1);
}

fn main() -> i64 {
    var counter: Counter = Counter(40);
    increment(counter, 1);
    forward(counter);
    ska_rt_println_i64(inspect(counter));
    return 0;
}
```

with exact stdout `42\n`.

This slice is the next object-model step because an alias parameter passes one
stable address regardless of class size. By-value object parameters would
instead require copy construction, destruction, parameter cleanup, aggregate
calling conventions, return storage, temporaries, and elision.

## 1. Scope

### Included

- `ref name: Class` and `mut ref name: Class` explicit parameters on internal
  top-level functions, methods, and initializers;
- exact nominal class matching, without inheritance or conversions;
- alias arguments formed from an existing inline class local, `self`, or an
  existing compatible alias parameter;
- grouped forms of those places where grouping does not change the designated
  storage;
- read-only field access and read-only method calls through `ref`;
- field mutation and both receiver modes through `mut ref`;
- forwarding `mut ref` as either `mut ref` or `ref`, and forwarding `ref` only
  as `ref`;
- multiple parameters designating the same object, including multiple mutable
  aliases;
- alias arguments mixed with existing primitive by-value arguments, preserving
  source evaluation order;
- target-independent parameter binding modes and call-argument kinds in HIR
  and MIR;
- verified indirect places in MIR;
- an internal Linux x86-64 System V ABI that passes each alias as one
  integer-class pointer;
- deterministic phase dumps, focused diagnostics, verifier corruption tests,
  backend tests, and native golden coverage.

### Explicitly excluded

- value parameters, results, locals, fields, or temporaries of class type;
- locally declared aliases, alias fields, alias returns, alias values, function
  values containing alias signatures, captures, or storage of an alias;
- alias parameters whose designated type is primitive, `unit`, optional,
  array, `shared`, interface, or function type;
- `ref` or `mut ref` on external declarations or through the C ABI;
- object fields, array elements, static objects, optional payloads, or other
  place forms that the compiler does not yet support;
- constructed object temporaries as alias arguments;
- shared pointees, borrow anchors, retain/release operations, or ownership
  provenance at runtime;
- whole-object replacement through `mut ref`;
- exclusivity checking, overlap diagnostics, or Rust-style borrowing rules;
- inheritance, subtype conversion, interfaces, virtual dispatch, casts, or
  dynamic type metadata;
- `assign`, `destroy`, cleanup edges, checked exceptions, or unwinding;
- AArch64 and any stable cross-module object ABI.

Unsupported forms must receive source-level diagnostics. They must not become
ordinary object values accidentally or reach the backend as malformed MIR.

## 2. Semantic Contract

### Parameter grammar

The implemented parameter grammar becomes:

```text
parameter = [binding-mode] identifier ":" parameter-type

binding-mode = "ref"
             | "mut" "ref"
```

For this slice, a parameter without a binding mode keeps the existing
primitive by-value rules. A parameter with a binding mode must name an exact
class type. `ref mut`, repeated modifiers, missing names/types, and modifiers
in any non-parameter position are invalid.

`ref` and `mut` are reserved keywords in the implemented grammar. `ref` and
`mut ref` are binding modes, not type constructors. The underlying parameter
type remains `Class`; phase IR must represent the mode separately from the
type rather than inventing pointer or reference source types.

### Places and calls

An alias argument must be an addressable, already-live class place. The
implemented sources are:

1. a directly constructed inline local;
2. `self` in a method whose receiver access is compatible;
3. an alias parameter forwarded to another call;
4. a parenthesized form of one of the above.

The argument never creates, copies, moves, or destroys a class value. It
selects an existing place and passes that place's address. A callee cannot
observe whether the address originated from a local, `self`, or a forwarded
alias.

Calls retain their existing left-to-right evaluation rule. Selecting one of
the supported alias places has no user-visible computation, but the ordered
argument representation must leave room for later place expressions whose
anchor evaluation is observable. An implementation must not split value and
alias arguments into independently reordered lists.

The destination of a direct construction exists but is not a live object
until its initializer returns. It cannot be supplied as an alias from within
that initializer. Existing initializer-body restrictions remain in force.
An initializer may itself receive an alias parameter and read from it while
initializing primitive fields.

### Access modes

Access is a two-level capability:

```text
mutable  ->  read-only
```

A mutable place may satisfy either alias mode. A read-only place may satisfy
only `ref`.

- A `ref` binding may read fields, call read-only methods, and be forwarded to
  another `ref` parameter.
- A `ref` binding may not assign a field, call a `mut fn`, or satisfy a
  `mut ref` parameter.
- A `mut ref` binding may read or assign fields, call either receiver mode,
  and be forwarded as either alias mode.
- Passing a mutable place to `ref` restricts access only through the callee's
  binding; it does not freeze the object globally.
- `self` has its enclosing method's receiver access. An initializer's `self`
  remains governed by construction-state rules rather than being a generally
  passable mutable place.

Aliases are non-exclusive. The following is legal even when both arguments
designate the same local:

```ska
fn touch_twice(mut ref left: Counter, mut ref right: Counter) -> unit {
    left.add(1);
    right.add(1);
}

touch_twice(counter, counter);
```

No overlap analysis is performed. This is memory-safe because both aliases
are call-scoped, the local remains live, and neither alias can be stored or
returned.

### Lifetime boundary

Every supported source place already has storage whose lifetime encloses the
call:

- an inline local remains in its declaring activation;
- a method receiver is kept alive by its caller;
- a forwarded alias inherits the enclosing call's guarantee.

Consequently this slice requires no retain/release operation, hidden guard,
allocation, runtime provenance tag, graph search, or lifetime analysis. MIR
still records alias calls explicitly so the later `shared` slice can attach a
syntax-directed caller-owned anchor without changing callee semantics.

Aliases cannot escape because the language has no alias values: an alias name
is accepted only in place contexts such as member access, method receiving, or
another alias argument. It remains invalid as an ordinary object expression,
return value, value argument, local initializer, field value, or assignment
source.

## 3. IR and ABI Direction

### Phase representation

Each phase owns an explicit parameter binding-mode enum with the conceptual
values `Value`, `ReadOnlyAlias`, and `MutableAlias`. The mode travels with
stable parameter identity and remains separate from the nominal `Type`.

HIR call arguments distinguish:

- an evaluated by-value expression;
- an alias place with explicit class and access capability.

MIR signatures similarly contain parameter descriptors rather than parallel
type and mode arrays. MIR call and initialization arguments form one ordered
sequence whose entries are either scalar values or places. This avoids index
drift and makes signature verification local.

An alias parameter's incoming machine word is an address, not inline object
storage. MIR must distinguish an indirect alias-parameter base from an owning
local-storage base. Field projections compose from either base without
embedding target byte offsets in MIR.

The verifier is responsible for at least these invariants:

- signature and body parameter counts, identities, modes, and types agree;
- value parameters have value-compatible storage and alias parameters have
  indirect alias storage;
- value arguments match value parameters and alias places match alias
  parameters;
- every alias place is live, has the exact class type, belongs to the calling
  body, and has sufficient access;
- a read-only indirect base is never the destination of a store and never the
  receiver of a mutable call;
- class places never enter scalar value instructions or scalar return paths;
- external declarations contain no alias modes;
- malformed projections and foreign storage identities are rejected before a
  backend sees them.

### Linux x86-64 System V ABI

For internal calls, both alias modes use the same representation: one pointer
to the complete inline object storage. The pointer is integer-class and has
machine-pointer size and alignment. `ref` versus `mut ref` is a compile-time
access distinction and does not change code generation.

Alias parameters participate in the existing source-ordered mixed-class call
layout:

- the implicit receiver, when present, remains first and integer-class;
- each alias consumes the next integer register or source-ordered stack slot;
- primitive integer and SSE arguments keep their independent register
  counters;
- stack arguments retain the existing System V order and alignment rules.

The callee stores each incoming alias pointer in a pointer-sized frame home.
Loading or storing a field through the alias first obtains that pointer, then
adds target-owned field offsets. The caller materializes the address of the
verified source place into the assigned argument location. No object bytes are
copied.

Receiver and alias parameters are both indirect object bases. Backend cleanup
may generalize their shared address-resolution mechanics, while preserving
their distinct source identities and semantic access rules.

This is an internal stage-0 ABI. It does not enable object-bearing external
declarations and promises no cross-module ABI stability.

## 4. Architectural Rules

1. Binding mode is explicit in syntax, resolved IR, HIR, MIR, dumps, and
   callable signatures; it must not be inferred again in lower phases.
2. Binding mode remains orthogonal to type even though the first slice accepts
   aliases only for class types.
3. Resolution selects stable bindings and callable identities but does not
   perform type or access checking.
4. Type checking is the authority for place eligibility, exact nominal type,
   and access-capability reduction.
5. Calls use one ordered argument representation. Do not maintain parallel
   value/place vectors or reconstruct source order in MIR lowering.
6. MIR represents semantic indirection and field identity, never target
   pointers, registers, sizes, or byte offsets.
7. MIR verification is extended before backend lowering relies on alias
   invariants.
8. The backend classifies parameter descriptors at its ABI boundary and
   structurally rejects unsupported modes or types.
9. Shared-ownership anchor selection is not anticipated with placeholder
   runtime behavior. The call representation should make a later explicit
   anchor-lowering step possible.
10. Dumps remain deterministic and render mode/place distinctions clearly
    enough to diagnose phase-boundary mistakes.
11. Repeated parameter-signature traversal should use a common phase-local
    descriptor/helper where that removes duplicated zip/count logic.
12. Refactor receiver/alias address handling when it creates a smaller
    coherent abstraction; avoid broad unrelated rewrites.

## 5. Progress Summary

- [x] AL0 — Freeze the restricted alias-parameter contract
- [x] AL1 — Add binding-mode syntax and parser diagnostics
- [x] AL2 — Resolve alias signatures and existing object places
- [x] AL3 — Type-check alias access and build typed call arguments
- [x] AL4 — Add verified alias parameters and place arguments to MIR
- [x] AL5 — Lower the alias pointer ABI on Linux x86-64
- [x] AL6 — Connect typed aliases through HIR-to-MIR lowering
- [x] AL7 — Enable, harden, and document the complete native slice

A task is complete only when all of its checkboxes, acceptance criteria, and
relevant quality gates pass.

## 6. PR-Sized Tasks

### AL0 — Freeze the restricted alias-parameter contract

**Purpose:** Remove semantic and ABI ambiguity before binding modes are copied
through every compiler phase.

- [x] Add a restricted alias-parameter implementation profile to the draft
      specification.
- [x] Freeze the grammar, supported declaration positions, exact class-type
      restriction, and valid argument-place forms.
- [x] Define the access-capability rules for locals, `self`, `ref`, and
      `mut ref`.
- [x] Define forwarding, deliberate alias overlap, evaluation order, and the
      non-escaping lifetime argument.
- [x] Define the MIR signature/call shape and indirect-place invariants.
- [x] Freeze the initial internal pointer ABI and external-declaration
      exclusion.
- [x] Reconcile the draft specification, implemented grammar, repository
      architecture, and future-boundaries documentation.

**Tests:** Cross-document review against current object-place, receiver,
parameter, call, verifier, frame, and ABI boundaries. No executable behavior
changes.

**Acceptance criteria:** Later tasks do not need to invent grammar, place
eligibility, mutability, lifetime, IR, or ABI rules.

### AL1 — Add binding-mode syntax and parser diagnostics

**Purpose:** Represent the source distinction precisely without teaching the
parser ownership or access semantics.

- [x] Add `ref` tokenization and the exact `ref` / `mut ref` parameter grammar.
- [x] Add an AST parameter binding-mode enum separate from `TypeSyntax`.
- [x] Preserve the complete modifier/name/type span information needed for
      diagnostics and dumps.
- [x] Parse binding modes uniformly for functions, methods, initializers, and
      external declarations; leave legality to the appropriate semantic
      boundary.
- [x] Diagnose malformed order, repetition, missing components, and use of
      modifiers where a declaration or expression is expected.
- [x] Extend parameter recovery so one malformed alias parameter does not hide
      later parameters, members, or declarations.
- [x] Update exact AST dumps and split parser tests by alias concern if the
      existing declaration/object test modules become crowded.

**Tests:** Lexer keyword/prefix tests; parser tests for every callable form,
mixed parameter lists, grouping, malformed modifiers, recovery, spans, and
exact deterministic AST dumps.

**Acceptance criteria:** Valid alias syntax has an unambiguous source-shaped
AST; malformed syntax produces stable parser diagnostics; no semantic phase
infers a mode from tokens or spans.

### AL2 — Resolve alias signatures and existing object places

**Purpose:** Carry binding modes and stable nominal identities through name
resolution without allowing object expressions by accident.

- [x] Add a resolved parameter binding mode separate from `ResolvedType`.
- [x] Resolve named class types for alias parameters on internal functions,
      methods, and initializers.
- [x] Keep ordinary class value parameters and class results rejected by the
      restricted object profile.
- [x] Resolve alias parameter names as normal stable `ParameterId` bindings.
- [x] Permit those bindings as object-place bases for field selection, method
      selection, and context-dependent call arguments.
- [x] Preserve grouped place recognition and existing shadowing/name-space
      behavior.
- [x] Retain argument source shape until type checking decides whether each
      expression is a value or a required place.
- [x] Extend resolved dumps with modes and identity-based alias-place uses.

**Tests:** Resolution tests for forward calls/classes, all callable owners,
alias parameter shadowing, same member names across classes, grouped places,
unknown class/member names, ordinary object-value rejection boundaries, and
exact deterministic resolved dumps.

**Acceptance criteria:** Every alias declaration and use below resolution is
identity-based; resolution neither loses argument source order nor performs
access/type checking.

### AL3 — Type-check alias access and build typed call arguments

**Purpose:** Make type and access semantics explicit in HIR before any address
or ABI lowering occurs.

- [x] Add a HIR parameter mode and a single typed parameter descriptor used by
      all callable signature queries.
- [x] Add ordered HIR call arguments that distinguish scalar values from
      class places.
- [x] Centralize place-capability calculation for local objects, `self`, and
      alias parameters.
- [x] Check exact nominal class equality and reject implicit conversion or
      ordinary object-value fallback.
- [x] Allow mutable-to-read-only capability reduction and reject every
      read-only-to-mutable path.
- [x] Enforce field-write and mutable-method restrictions through alias bases
      using the same access vocabulary as receivers.
- [x] Support alias arguments for direct functions, methods, and initializers,
      including forwarding and mixed value/alias lists.
- [x] Reject alias modes on extern declarations and all non-class types in this
      profile with focused diagnostics.
- [x] Keep aliases invalid in returns, scalar expressions, ordinary value
      arguments, construction destinations, and every escaping position.
- [x] Extend HIR dumps with parameter modes, ordered argument kinds, and place
      access.

**Tests:** Type-checker tests covering the complete access matrix, exact-type
mismatches, alias overlap, forwarding, `self`, initializer alias reads, mixed
arguments, wrong arity, external aliases, excluded types/positions, and exact
deterministic HIR dumps.

**Acceptance criteria:** Typed HIR alone determines whether a parameter is by
value or alias, whether each argument is a value or place, and whether access
is sufficient. Lower phases do not repeat source-level type checking.

### AL4 — Add verified alias parameters and place arguments to MIR

**Purpose:** Establish a target-independent, corruption-resistant address
model before changing a backend.

- [x] Replace bare MIR parameter-type arrays with ordered parameter
      descriptors containing mode and underlying type.
- [x] Introduce ordered MIR call/initialization arguments with value and place
      variants.
- [x] Represent alias parameter homes as indirect place bases, distinct from
      owning local object storage.
- [x] Carry read-only versus mutable access in the MIR location where the
      verifier can enforce writes and mutable receiver calls.
- [x] Extend MIR builders and fixtures with small helpers for parameter
      descriptors and ordered call arguments.
- [x] Verify declaration/definition agreement, parameter storage, call kinds,
      exact types, place ownership, projection validity, and access sufficiency.
- [x] Reject alias parameters on external MIR declarations.
- [x] Extend MIR dumps with stable mode, indirect-base, and argument-kind
      rendering.
- [x] Refactor duplicated signature zip/count checks into verifier helpers when
      that makes the new invariants easier to audit.

**Tests:** Hand-built valid MIR for direct calls, methods, initializers,
forwarding, overlap, and mixed arguments; corruption tests for every invariant;
exact dumps; pass-pipeline preservation tests. No source syntax is enabled in
the backend yet.

**Acceptance criteria:** Invalid alias MIR is rejected structurally and valid
alias MIR remains target-independent, ordered, deterministic, and free of byte
offsets or registers.

### AL5 — Lower the alias pointer ABI on Linux x86-64

**Purpose:** Prove the machine representation independently with verified,
hand-built MIR before connecting the frontend.

- [x] Classify every alias descriptor as one integer-class machine pointer.
- [x] Spill incoming alias registers/stack slots into pointer-sized frame
      homes without allocating inline class payload storage.
- [x] Resolve indirect alias places and field projections through their stored
      pointer.
- [x] Materialize outgoing alias place addresses into integer registers or
      source-ordered stack slots.
- [x] Preserve independent integer/SSE counters for mixed receiver, alias, and
      primitive signatures.
- [x] Share receiver/alias indirect-address machinery where it improves
      clarity without conflating their semantic identities.
- [x] Extend target legality to reject unsupported descriptors and malformed
      object-bearing external signatures before instruction selection.
- [x] Keep layout computation in the target data-layout service and use
      checked displacement/frame arithmetic.

**Tests:** ABI classification boundaries; register and stack exhaustion;
receiver plus aliases plus SSE values; read/write field access through an alias;
alias forwarding; same-address arguments; assembler acceptance; native
execution from hand-built MIR; structured target failures.

**Acceptance criteria:** Hand-built verified MIR can read and mutate aliased
inline objects across internal calls without copying object bytes, and all ABI
edge cases are deterministic.

### AL6 — Connect typed aliases through HIR-to-MIR lowering

**Purpose:** Complete the source-to-machine path while keeping lowering
mechanical and independently testable.

- [x] Lower HIR parameter descriptors into MIR descriptors and parameter
      storage in stable source order.
- [x] Map alias-bound `BindingId` uses to indirect MIR place bases.
- [x] Lower value and alias arguments through one left-to-right traversal.
- [x] Lower local, `self`, grouped, and forwarded alias sources to exact MIR
      places without constructing object values.
- [x] Lower alias arguments consistently for functions, methods, and
      initializers.
- [x] Preserve existing scalar expression evaluation and receiver-before-
      arguments ordering.
- [x] Run MIR verification immediately after lowering and at the existing pass
      and backend trust boundaries.
- [x] Update lowering tests and exact MIR snapshots without duplicating
      type-checker access logic.

**Tests:** Source-driven MIR tests for read-only and mutable aliases,
forwarding, aliasing the same local twice, `self`, constructor alias arguments,
mixed integer/SSE arguments, nested calls, conditionals, and deterministic
exact dumps.

**Acceptance criteria:** Every valid typed alias program lowers to the same
verified MIR shape expected by AL5; lowering contains no source-name lookup,
access inference, ABI classification, or anchor logic.

### AL7 — Enable, harden, and document the complete native slice

**Purpose:** Make alias parameters a dependable public feature and close every
failure path exposed by end-to-end use.

- [x] Add native golden programs that observe read-only access, mutation,
      forwarding, deliberate overlap, `self`, initializer aliases, nested
      calls, conditionals, and mixed register/stack signatures.
- [x] Add compile-failure goldens for malformed syntax, excluded declaration
      positions/types, object-value misuse, exact-type mismatch, read-only
      mutation, mutable forwarding from `ref`, extern aliases, and wrong arity.
- [x] Assert exact stdout, exit status, empty runtime stderr, deterministic
      assembly, and deterministic diagnostics across compiler processes.
- [x] Add a representative cross-process object/alias phase-determinism case.
- [x] Audit panics and backend assumptions reachable from malformed supported
      source; convert failures to structured diagnostics where appropriate.
- [x] Update `grammar/README.md`, the draft specification, top-level README,
      debugging notes, golden-test documentation, and future boundaries to
      describe the implemented feature rather than the roadmap.
- [x] Remove obsolete “alias parameters not implemented” statements from
      living documentation while retaining exclusions for local aliases,
      primitive aliases, shared sources, and anchors.
- [x] Run `make check` and archive this roadmap only after every checkbox and
      acceptance criterion is satisfied.

**Tests:** Full `make check`, including compiler unit/integration tests, all
goldens, assembler/linker execution, and runtime tests.

**Acceptance criteria:** The public compiler accepts exactly the documented
alias profile, rejects excluded forms with stable diagnostics, produces
correct native behavior, remains deterministic, and leaves no living document
describing a completed implementation sequence as future work.

## 7. Completion Gate

The slice is complete when:

- [x] all AL0–AL7 tasks and their acceptance criteria are complete;
- [x] `ref` and `mut ref` remain binding modes rather than reference types;
- [x] valid arguments are restricted to existing, stable inline class places;
- [x] read-only and mutable access are enforced consistently through calls,
      fields, methods, HIR, MIR, and verification;
- [x] aliases remain call-scoped, non-owning, non-storable, and non-returnable;
- [x] no borrow checker, runtime provenance tag, ownership search, retain, or
      release is introduced;
- [x] the x86-64 ABI passes one pointer per alias and never copies object bytes;
- [x] malformed source and malformed MIR fail structurally rather than
      panicking or being miscompiled;
- [x] dumps, diagnostics, assembly, stdout, and exit behavior are deterministic;
- [x] living documentation describes the implemented state and this completed
      roadmap has moved to `docs/archive/`.
