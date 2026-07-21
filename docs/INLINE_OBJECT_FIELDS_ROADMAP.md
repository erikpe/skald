# Class-Typed Inline Object Fields Roadmap

Status: in progress; IOF0–IOF2 are complete and IOF3–IOF6 are planned.

This roadmap adds class-typed fields to Skald's existing inline-object model.
The slice is deliberately about containment, construction into stable storage,
and recursive place projection. It does not add object copying or destruction.

The completed slice should compile programs such as:

```ska
extern fn ska_rt_println_i64(value: i64) -> unit;

class Cell {
    value: i64;

    init(value: i64) {
        self.value = value;
    }

    mut fn increment() -> unit {
        self.value = self.value + 1;
    }

    fn get() -> i64 {
        return self.value;
    }
}

class Pair {
    left: Cell;
    right: Cell;

    init(left: i64, right: i64) {
        self.left = Cell(left);
        self.right = Cell(right);
    }

    mut fn increment_left() -> unit {
        self.left.increment();
    }

    fn total() -> i64 {
        return self.left.get() + self.right.get();
    }
}

fn inspect(ref cell: Cell) -> i64 {
    return cell.get();
}

fn main() -> i64 {
    var pair: Pair = Pair(19, 22);
    pair.increment_left();
    ska_rt_println_i64(inspect(pair.right));
    return pair.total();
}
```

with exact stdout `22\n` and exit status `42`.

This is the next object-model slice because the current compiler already has
nominal class identities, class-typed storage, semantic field projections,
direct initialization into a place, checked target layout, receiver addresses,
and call-scoped aliases. Class-typed fields exercise those foundations without
requiring the copy, cleanup, aggregate ABI, return-storage, or temporary rules
needed by general object values.

## 1. Scope

### Included

- fields whose type is an exact concrete class declared in the same source
  file, including forward references;
- acyclic by-value containment of any finite depth;
- direct construction of each class-typed field in its own storage from the
  enclosing class's initializer;
- exact constructor-type matching and the existing single explicit `init` per
  class;
- primitive field initialization alongside class-field construction in the
  existing straight-line initializer body;
- primitive reads and writes through nested inline-field paths;
- direct method calls on nested inline fields;
- nested inline fields as `ref` and `mut ref` arguments, including paths rooted
  at locals, method receivers, and forwarded alias parameters;
- access propagation from the root binding through every inline projection;
- source-level rejection of direct and indirect recursive by-value
  containment;
- target-independent projection paths in resolved IR, typed HIR, and MIR;
- dependency-ordered Linux x86-64 layouts and checked address calculation;
- deterministic phase dumps, focused diagnostics, verifier corruption tests,
  backend tests, native goldens, and cross-process determinism coverage.

### Explicitly excluded

- whole-object field assignment or replacement after construction;
- object-valued locals initialized from a field, object-valued expressions,
  temporaries, arguments, results, or external signatures;
- implicit or synthesized copy construction, copy assignment, moves, slicing,
  or copy elision beyond the already permitted direct-local construction form;
- `assign`, `destroy`, initialized-place cleanup, scope-exit cleanup, partial-
  construction cleanup, or exceptional control-flow edges;
- construction into an arbitrary nested path, existing local, alias parameter,
  array element, optional payload, static object, or shared allocation;
- locally declared aliases, alias fields, alias returns, or storable addresses;
- inheritance, base subobjects, interfaces, virtual dispatch, casts, dynamic
  type metadata, access control, `final`, or static members;
- `shared`, `new`, allocation, reference counting, and borrow anchors;
- arrays, optionals, checked exceptions, multiple source files, and AArch64.

Unsupported forms must fail in syntax or semantic analysis with a focused
diagnostic. They must not become accidental object rvalues or malformed MIR.

## 2. Semantic Contract

### Field declarations and containment

The restricted field grammar becomes:

```text
field-declaration = identifier ":" field-type ";"
field-type        = primitive-type | class-name
```

`unit` remains invalid as a field type. A named field type must resolve to a
class, not a function or an unknown name. Classes remain nominal, and a later
declaration in the same file may be referenced because top-level collection
precedes type resolution.

Every class-typed field contains a complete inline subobject. It is not a
pointer, nullable handle, alias, or separately allocated value. Repeating a
class type in separate fields creates separate subobjects.

Class containment must form a directed acyclic graph. A direct self-field and
an indirect cycle such as `A -> B -> C -> A` are invalid regardless of target.
The source diagnostic should identify the participating fields in a stable,
readable cycle path. The backend retains defensive cycle detection for
hand-built or corrupt MIR, but a valid source program must be rejected before
MIR lowering or target selection.

### Direct field construction

The existing assignment-shaped initializer statement has two meanings based
on the declared field type:

```ska
self.count = 1;          // initialize primitive field storage
self.child = Child(1);   // construct class field storage in place
```

For a class-typed field, the right side must be an ungrouped construction of
that exact field class. It does not produce a value and is not an assignment to
a live object. The only new construction destination in this slice is a direct
field of the current initializer's `self`; the nested field's own initializer
is responsible for constructing its fields.

Every direct field—primitive or class-typed—is initialized exactly once before
the enclosing initializer completes. Field initialization continues to follow
source order and need not match declaration order. A class field becomes a
live subobject only after its initializer returns normally. The enclosing
`self` becomes a live complete object only after all direct fields are
initialized and its initializer returns.

Before a class field becomes live, it cannot be read, used as a receiver, or
passed as an alias. After it becomes live, later initializer expressions may
read its primitive descendants, call methods on it, or pass it to a compatible
alias parameter. The incomplete enclosing `self` remains unavailable as a
method receiver or alias argument. Constructor arguments retain their existing
left-to-right evaluation order, and an object field is marked initialized only
after its nested initializer returns.

There are no recoverable construction failures in this slice. Instruction
order and completed-subobject boundaries must nevertheless remain explicit so
the destruction and checked-exception roadmaps can add cleanup without
reconstructing source evaluation order.

### Nested places and access

The supported object-place roots remain:

1. a live inline local;
2. method `self` when the complete receiver is live;
3. a `ref` or `mut ref` parameter;
4. a grouped form of one of those roots.

An object place may then contain zero or more class-field projections. A final
primitive field selection loads or stores a scalar; a class-typed endpoint may
serve as a method receiver or alias argument. Selecting a class field in an
ordinary value context remains invalid.

Access propagates unchanged through inline containment:

- a mutable local, mutable method receiver, or `mut ref` root provides mutable
  access to its inline subobjects;
- a read-only method receiver or `ref` root provides only read-only access to
  every inline subobject;
- read-only access permits primitive reads, read-only method calls, and `ref`
  arguments;
- mutable access additionally permits primitive writes, mutable method calls,
  and `mut ref` arguments;
- no access path permits whole-object replacement in this slice.

Aliases to inline fields remain call-scoped and non-owning. The containing
local, receiver, or forwarded alias already keeps the storage alive for the
call, so no borrow anchor, retain/release operation, provenance tag, or general
lifetime inference is required.

## 3. Compiler Direction

### Source phases and typed HIR

Resolution remains the only name-selection phase. It resolves class field
types to `ClassId`s and each member step to a `FieldId` or `MethodId`. Lower
phases must never repeat selection from source spelling.

`ResolvedObjectPlace` and `HirObjectPlace` should grow from a single binding
into a root binding plus an ordered field-projection path and terminal class.
The exact Rust shape may differ, but one canonical path must serve field reads,
writes, receiver calls, alias arguments, and construction destinations. Avoid
parallel vectors or separate nested-place implementations whose indices can
drift.

HIR must distinguish scalar field initialization from construction into a
class-field destination. A class construction remains a destination-directed
operation, never a `HirExpression` that later becomes an object value. Place
access and initializer liveness are explicit type-checker decisions.

### MIR and backend

MIR already has the intended representation: a `MirPlace` contains a storage
base plus ordered `Field(FieldId)` projections, and `MirInitialize` constructs
into a destination place. This slice should extend lowering and verification,
not introduce a second aggregate IR or byte offsets above the backend.

The x86-64 data-layout service already computes nested class layouts in
dependency order and defensively rejects recursive layouts. It remains the
sole authority for sizes, alignments, offsets, and checked target arithmetic.
Deep projection addressing must work identically from direct storage, receiver
storage, and indirect alias bases. No object-bearing calling-convention change
is part of this slice: nested receivers and alias arguments still pass one
address.

### Maintainability rules

- Keep `mod.rs` files as concise facades with explicit re-exports.
- Put the containment-graph algorithm in a cohesive type-checking module
  rather than enlarging program orchestration with private traversal state.
- Centralize projection walking and access calculation instead of adding
  special cases independently to reads, writes, calls, aliases, and dumps.
- Extend existing MIR/backend place and layout services rather than adding
  feature-specific offset calculation.
- Prefer small, local refactors in touched modules when they remove duplicated
  signature, place, or diagnostic logic; avoid unrelated repository-wide
  rearrangement.
- Keep substantial unit-test collections in the existing phase `tests/`
  modules and complete source-to-diagnostic or source-to-native behavior in the
  top-level golden suite.
- Preserve deterministic declaration and projection order; never make dumps or
  diagnostics depend on hash-map iteration.

## 4. Progress Summary

- [x] IOF0 — Freeze the class-typed-field contract
- [x] IOF1 — Resolve field types and reject containment cycles
- [x] IOF2 — Generalize semantic object places to projection paths
- [ ] IOF3 — Construct class fields and track initializer liveness
- [ ] IOF4 — Type-check nested access, receivers, and alias arguments
- [ ] IOF5 — Lower and execute projected subobjects through MIR and x86-64
- [ ] IOF6 — Harden, document, and publish the complete slice

A task is complete only when its checkboxes, tests, acceptance criteria, and
relevant quality gates pass.

## 5. PR-Sized Implementation Tasks

### IOF0 — Freeze the class-typed-field contract

**Purpose:** Remove ambiguity about containment, initialization, access, and
evaluation before those decisions are duplicated across phases.

- [x] Add a restricted class-typed-field profile to the draft specification.
- [x] Update the parser-facing grammar for primitive or named class field
      types without enabling object types in other value positions.
- [x] Define exact source forms for primitive field initialization and direct
      class-field construction.
- [x] Freeze field liveness, exactly-once initialization, and the permitted use
      of already-constructed subobjects inside an enclosing initializer.
- [x] Freeze nested read/write, receiver, alias, grouping, access-propagation,
      and evaluation-order rules.
- [x] Define deterministic source diagnostics for unknown field types, direct
      recursion, indirect containment cycles, wrong constructors, premature
      use, duplicate/missing initialization, and object-value misuse.
- [x] Reconcile the new profile with the later destruction, copy, inheritance,
      shared-ownership, and exception contracts without implementing them.

**Tests:** Cross-document review against `grammar/README.md`, the current HIR
and MIR place model, x86-64 layout behavior, and the archived object/alias
roadmaps. No compiler behavior changes.

**Acceptance criteria:** Later PRs need not invent field syntax, containment
legality, subobject liveness, access propagation, construction order, or
excluded-feature behavior.

### IOF1 — Resolve field types and reject containment cycles

**Purpose:** Make class metadata semantically complete and reject impossible
inline layouts before executable bodies or targets consume it.

- [x] Allow a field declaration to carry a named type while retaining the
      focused `unit` and unsupported-type diagnostics.
- [x] Resolve a named field type through the collected top-level class table,
      assigning a stable `ClassId` and diagnosing unknown names or functions
      used as types.
- [x] Replace primitive-only field lowering with canonical `Type::Class`
      metadata in HIR while preserving source spans and declaration order.
- [x] Add a target-independent containment validator over resolved class and
      field identities.
- [x] Reject direct and indirect cycles with a stable path that names the
      participating classes and fields; avoid duplicate diagnostics caused by
      traversal start order.
- [x] Accept acyclic diamonds, repeated field types, forward dependencies, and
      nested empty classes.
- [x] Keep the backend's recursive-layout check as a structured defense for
      malformed hand-built MIR.
- [x] Put the graph traversal and its tests in a cohesive module, exposing only
      a narrow validation entry point through the type-checker facade.

**Tests:** Parser AST tests; resolved and HIR metadata/dump tests; unknown and
non-class names; direct self-containment; multi-class cycles; multiple acyclic
dependency shapes; deterministic diagnostic ordering; existing primitive
field regressions.

**Acceptance criteria:** Every valid HIR class table has exact field types and
an acyclic inline-containment graph, independent of target layout.

### IOF2 — Generalize semantic object places to projection paths

**Purpose:** Represent an arbitrarily deep inline subobject once so every
source feature consumes the same stable, identity-based path.

- [x] Generalize resolved object places from one binding to a root binding plus
      ordered `FieldId` projections and a terminal `ClassId`.
- [x] Resolve nested member receivers recursively through class-typed fields,
      selecting every field or method exactly once during resolution.
- [x] Preserve grouping spans without losing the canonical projection path.
- [x] Generalize HIR object places to the same identity path and keep access as
      one capability on the complete path rather than per-step flags.
- [x] Retain an explicit terminal scalar-field selection where it keeps load
      and store typing clear; do not create an object rvalue for class fields.
- [x] Centralize place-path construction, terminal-type lookup, and diagnostic
      rendering so reads, writes, calls, aliases, and later construction do not
      rebuild paths independently.
- [x] Update resolved and HIR dumps to render complete paths deterministically.
- [x] Refactor oversized touched modules into descriptive private submodules
      only where projection logic has become an independently coherent concern.

**Tests:** Nested member resolution from locals, `self`, and both alias modes;
wrong member at intermediate and terminal classes; field-versus-method misuse;
grouping; identity paths with repeated field names; exact deterministic dumps.

**Acceptance criteria:** Resolved IR and HIR can identify any supported inline
subobject without source names, target offsets, pointers, or duplicated path
representations.

### IOF3 — Construct class fields and track initializer liveness

**Purpose:** Initialize nested objects directly in their final storage while
preserving a precise boundary between incomplete and live subobjects.

- [ ] Interpret `self.field = Class(...)` as destination-directed construction
      when the declared field type is a class.
- [ ] Require an ungrouped constructor of the exact field class and reject a
      scalar expression, wrong class, ordinary object place, or other object
      context with a focused diagnostic.
- [ ] Permit class construction only into a direct field of the current
      initializer's `self`; reject nested, foreign, local, alias, and live
      destinations.
- [ ] Add an explicit HIR statement for construction into a field place,
      reusing the existing typed construction payload and ordered arguments.
- [ ] Track each direct field as uninitialized or initialized and transition a
      class field only after its constructor arguments check successfully.
- [ ] Apply the existing exactly-once and missing-field checks uniformly to
      primitive and class fields.
- [ ] Reject reads, receiver use, or aliasing through an uninitialized direct
      field while allowing prior initialized fields in later expressions.
- [ ] Preserve source statement order and left-to-right constructor argument
      order without adding cleanup state or exception edges.

**Tests:** Mixed primitive/class fields; declaration-order and non-declaration-
order initialization; nested constructors; alias constructor arguments;
wrong/grouped constructors; premature, duplicate, and missing initialization;
attempted nested or non-`self` destinations; exact HIR dumps.

**Acceptance criteria:** Typed HIR distinguishes scalar stores from direct
subobject construction and identifies exactly when each field becomes live,
without representing construction as an object expression.

### IOF4 — Type-check nested access, receivers, and alias arguments

**Purpose:** Make contained objects useful through one consistent access and
liveness model rather than feature-specific exceptions.

- [ ] Read and write primitive leaf fields through projection paths of any
      finite supported depth.
- [ ] Call read-only and mutable methods on live class-field endpoints.
- [ ] Accept live class-field endpoints as exact-class `ref` or `mut ref`
      arguments, including forwarding from alias-rooted paths.
- [ ] Propagate the root binding's read-only or mutable capability unchanged
      through every inline projection.
- [ ] Enforce the same access matrix for nested stores, mutable receiver calls,
      and mutable alias arguments.
- [ ] During an initializer, permit method calls and alias use only through
      already-live external places or completed class fields; keep incomplete
      `self` unavailable as a complete receiver or alias.
- [ ] Reject a class-field endpoint in scalar, return, ordinary value-argument,
      local-initializer, or whole-object assignment contexts.
- [ ] Keep exact nominal matching and the existing non-exclusive alias policy;
      do not add subtype conversion, overlap checking, or borrow anchors.

**Tests:** Complete read-only/mutable matrix across local, `self`, `ref`, and
`mut ref` roots; nested receivers; nested alias forwarding and deliberate
overlap; completed versus incomplete fields in `init`; exact-type mismatch;
object-value escape attempts; deterministic HIR diagnostics and dumps.

**Acceptance criteria:** Typed HIR alone determines every nested place's class,
projection path, liveness, and access sufficiency. MIR lowering performs no
source-level member selection or access inference.

### IOF5 — Lower and execute projected subobjects through MIR and x86-64

**Purpose:** Connect the typed feature to the prepared place/layout machinery
and harden that machinery at the new source-reachable depth.

- [ ] Lower HIR object paths to ordered MIR field projections from direct,
      receiver, and indirect alias storage bases.
- [ ] Lower class-field construction to `MirInitialize` with the exact
      projected destination and source-ordered arguments.
- [ ] Keep class endpoints as places for receivers and alias arguments; never
      allocate a class-typed `MirValue`.
- [ ] Audit and, where useful, refactor MIR place verification so one projection
      walker returns terminal type and access for loads, stores, receivers,
      arguments, and initialization.
- [ ] Verify every projection owner/type step, terminal scalar load/store,
      exact initializer destination, and access requirement.
- [ ] Extend deterministic MIR dumps and pass-pipeline preservation tests for
      deep projected construction, calls, aliases, loads, and stores.
- [ ] Exercise dependency-ordered class layout for forward references, deep
      containment, padding, repeated types, and empty subobjects.
- [ ] Resolve deep target addresses with checked offset arithmetic through the
      existing backend place path for local, receiver, and alias bases.
- [ ] Preserve the existing hidden-receiver and alias pointer ABI; add no
      aggregate argument/result classification.
- [ ] Apply small facade-oriented refactors in MIR or backend modules if new
      logic would otherwise duplicate projection or layout responsibilities.

**Tests:** Source-driven MIR lowering; hand-built verifier mutations; exact MIR
dumps; layout and frame tests; deep width-correct primitive accesses; receiver
and alias register/stack cases; assembler acceptance; hand-built and
source-driven native execution; structured backend cycle/incomplete-metadata
defenses.

**Acceptance criteria:** Verified MIR and the x86-64 backend construct and use
acyclic inline subobjects of arbitrary supported depth with semantic IDs above
the target boundary and checked offsets below it.

### IOF6 — Harden, document, and publish the complete slice

**Purpose:** Make class-typed fields a dependable public feature and leave the
repository ready for the destruction roadmap.

- [ ] Add native goldens covering mixed/nested fields, access through each root
      kind, method calls, aliasing, evaluation order, padding, and empty
      subobjects.
- [ ] Add compile-failure goldens for recursive containment, unknown/non-class
      field types, wrong or grouped constructors, premature/duplicate/missing
      initialization, read-only mutation, wrong alias access/type, object-value
      misuse, and whole-object replacement.
- [ ] Assert exact stdout, exit status, empty runtime stderr, deterministic
      assembly, and deterministic diagnostics across compiler processes.
- [ ] Audit source-reachable assertions and backend assumptions; convert
      malformed supported input into structured diagnostics where needed.
- [ ] Update `grammar/README.md`, the draft specification, repository
      architecture, top-level README, debugging notes, golden-test docs, and
      future boundaries to describe implemented behavior.
- [ ] Remove obsolete primitive-field-only and object-field exclusion text
      while retaining the copy, destruction, polymorphism, shared, and object-
      value exclusions.
- [ ] Run the complete quality gate and resolve all warnings or nondeterminism.
- [ ] Mark every roadmap checkbox complete, move this document to
      `docs/archive/`, update the archive index, and make destruction the next
      active object-model roadmap only after the slice is fully implemented.

**Tests:** Full `make check`, including compiler unit/integration tests,
goldens, assembler/linker execution, runtime tests, and documentation review.

**Acceptance criteria:** The public compiler accepts exactly the documented
class-field profile, rejects every excluded form intentionally, produces
correct deterministic native behavior, and leaves no living documentation
describing implemented class fields as future work.

## 6. Required Quality Gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `make runtime-test`
- [ ] `make golden-test`
- [ ] `make check`
- [ ] Deterministic AST, resolved, HIR, MIR, assembly, and diagnostics
- [ ] Recursive containment rejected before MIR and target selection
- [ ] Backend recursion and malformed-metadata defenses retained
- [ ] No source-name lookup below resolution
- [ ] No target size, alignment, offset, register, or ABI location in HIR/MIR
- [ ] No class object represented as a scalar MIR value
- [ ] No copy, destruction, cleanup, allocation, or borrow-anchor semantics
      introduced accidentally
- [ ] Touched Rust modules retain concise facades and cohesive implementation
      ownership
- [ ] Documentation and milestone checkboxes match behavior

## 7. Completion Gate

The slice is complete when:

- [ ] all IOF0–IOF6 tasks and their acceptance criteria are complete;
- [ ] class fields form a validated acyclic inline-containment graph;
- [ ] every class field is constructed exactly once in its final storage;
- [ ] incomplete subobjects cannot be observed and completed subobjects can be
      used through supported nested places;
- [ ] read-only and mutable access propagate consistently through containment;
- [ ] nested fields work as receivers and call-scoped alias arguments without
      copying or additional ownership machinery;
- [ ] MIR remains target-independent and class objects remain place-only;
- [ ] the x86-64 backend owns dependency-ordered layout and checked offsets;
- [ ] malformed source and malformed MIR fail structurally rather than
      panicking or being miscompiled;
- [ ] dumps, diagnostics, assembly, stdout, and exit behavior are deterministic;
- [ ] full quality gates pass and living documentation matches the compiler.

The next object-model roadmap is deterministic destruction: `destroy`,
initialized-place state in executable control flow, reverse-order scope
cleanup, and cleanup-aware exits. Copy/value semantics, polymorphism, shared
ownership, and checked exceptions remain later dedicated slices.
