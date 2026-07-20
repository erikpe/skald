# First Inline Objects Roadmap

Status: OBJ0–OBJ8 complete; OBJ9 is next.

This roadmap introduces Skald's first non-primitive values through a deliberately
restricted inline-object slice. Its purpose is to establish durable compiler
foundations—not to implement a small version of every object feature. Stable
identities, phase-owned metadata, projected places, construction state, target
layout, and the receiver ABI should all remain useful as the object model grows.

The completed slice should compile:

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
}

fn main() -> i64 {
    var counter: Counter = Counter(40);
    counter.add(2);
    ska_rt_println_i64(counter.get());
    return 0;
}
```

with exact stdout `42\n`.

This is a better first object slice than object call-by-value. Object parameters
and results immediately require copy construction, parameter cleanup, aggregate
calling conventions, return storage, temporaries, and copy elision. Those
semantics should follow only after Skald can represent and execute one object in
stable storage correctly.

## 1. Scope and Design Constraints

### Included

- nominal top-level `class` declarations;
- zero or more fields of any implemented primitive value type;
- exactly one explicit, non-overloaded `init` per class;
- direct construction into a newly declared local of the exact class type;
- primitive constructor parameters, evaluated left to right;
- `self`, field reads, and field writes;
- direct non-virtual methods with primitive parameters and primitive or `unit`
  results;
- read-only `fn` and mutable `mut fn` receiver rules;
- direct calls on a local inline object or `self`;
- existing primitive expressions, calls, locals, conditionals, and output in
  method bodies where their current rules permit them;
- target-independent class/member identities and field-projected places;
- deterministic inline layout and hidden receiver-address lowering on Linux
  x86-64 System V;
- phase tests, verifier corruption tests, deterministic dumps, diagnostics, and
  exact native-output goldens.

### Explicitly excluded

- objects as parameters, results, ordinary call arguments, or general
  expression values;
- construction except as the complete initializer of a new exact-type local;
- constructor or method overloading, synthesized/default/delegating
  constructors;
- whole-object copying, assignment, moves, slicing, or copy elision;
- `assign`, `destroy`, cleanup emission, or observable object termination;
- object fields, inheritance, interfaces, virtual dispatch, casts, or type
  tests;
- `shared`, `new`, alias parameters, local aliases, borrow anchors, or dynamic
  type metadata;
- static members, access control, `final`, or method values;
- object linkage through the restricted external-function ABI;
- arrays, optionals, strings, generics, checked exceptions, and AArch64.

Every excluded form should receive a focused diagnostic rather than failing as
an unresolved primitive call or an accidental backend error.

### Restricted semantic contract

OBJ0 freezes the exact grammar. The class-member surface is:

```text
class-declaration = "class" identifier "{" class-member* "}"
class-member      = field-declaration
                  | initializer-declaration
                  | method-declaration
field-declaration = identifier ":" primitive-type ";"
initializer-declaration = "init" parameter-list block
method-declaration      = ["mut"] "fn" identifier parameter-list
                          "->" result-type block
```

`init` remains a contextual special-member introducer rather than a globally
reserved name.

1. Classes are nominal. Equal structure does not make class types compatible.
2. A class has exactly one explicit `init`. It takes only primitive by-value
   parameters and has an implicit mutable receiver and `unit` result.
3. An initializer is straight-line for construction-state purposes. It assigns
   every field exactly once, never reads a field before it is initialized, and
   cannot return early. OBJ0 defines the precise right-hand-side subset.
4. Constructor arguments are evaluated left to right. Destination storage
   exists but is not a live object while `init` runs; normal completion makes it
   live.
5. `T(...)` is legal only as the complete initializer in
   `var value: T = T(...);`. It does not produce a general object temporary.
6. Current object locals are mutable because they use `var`. Read-only methods
   may read fields and call read-only methods. `mut fn` methods may also assign
   fields and call mutable methods.
7. Method selection is static and direct. Member names are unique within their
   category as defined by OBJ0; this slice performs no overload resolution.
8. The hidden receiver address is not a source parameter and is not observable
   as a language value.
9. Empty classes have nonzero addressable target storage so later aliasing need
   not change the object-address model.
10. There is no observable cleanup in this slice, but IR must preserve
    construction state and source order for later destruction and exceptions.

### Architectural rules

1. Resolution assigns stable `ClassId`, `FieldId`, initializer, and method
   identities. No lower phase selects a member again from source text.
2. Every executable body has one documented identity. Top-level functions,
   initializers, and methods must not acquire unrelated semantic and codegen
   identities joined by name-based maps.
3. Each phase owns the class declaration/definition table appropriate to that
   phase; source AST structures do not leak into HIR, MIR, or the backend.
4. HIR uses a nominal class type and records selected fields, initializers,
   methods, and receiver access explicitly.
5. MIR distinguishes addressable places from loaded scalar values. A place has
   a storage base and typed projections such as `Field(FieldId)`.
6. Construction is represented as initialization into a destination place, not
   as an aggregate rvalue followed by a store. This leaves room for partial
   initialization, cleanup, and elision later.
7. MIR contains semantic field identities, never byte offsets. A target layout
   service computes sizes, alignments, and offsets from canonical metadata.
8. Object storage can have a class type; this slice does not permit a class
   object as an ordinary MIR value, call argument, or result. The verifier owns
   that invariant.
9. Initializers and methods receive the address of existing object storage.
   The backend owns its ABI location and interaction with explicit arguments.
10. Layout, ABI classification, symbols, frames, and instructions remain
    backend concerns. HIR and MIR do not mention registers or offsets.
11. Public syntax is enabled only after the full supported x86-64 path exists.
    Earlier backend milestones use hand-built MIR; frontend milestones remain
    phase-tested behind a clear capability boundary.
12. Tables, dumps, layouts, and symbols use stable identity/source order, never
    hash-map iteration order.

### Initial layout and receiver ABI direction

OBJ0 makes the following target contract normative before code generation:

- fields are laid out in declaration order at correctly aligned offsets;
- class alignment is the maximum field alignment, or one when empty;
- trailing padding rounds size to class alignment; an empty class has size and
  alignment one;
- `i64`, `u64`, and `f64` use size/alignment 8/8; `u8` and `bool` use 1/1 on
  the initial target;
- checked layout arithmetic diagnoses target limits rather than overflowing;
- an initializer or method receives the object address as a hidden first
  integer-class argument;
- the receiver consumes one integer register but no SSE register, preserving
  the existing independent System V argument-class allocation;
- one collision-proof symbol service derives internal initializer and method
  names from stable identities, not unchecked source-name concatenation.

This is an internal stage-0 ABI. Objects remain excluded from the C ABI, and no
cross-module object ABI stability is promised yet.

### Evaluation and future lifetime boundary

The complete evaluation/cleanup contract remains a prerequisite for
destruction, shared ownership, and checked exceptions. This slice freezes only
the ordering it exercises:

- evaluate a method receiver before its arguments;
- evaluate explicit arguments from left to right;
- calculate a field place after its receiver place;
- evaluate an assignment value before storing it;
- enter `init` only after evaluating every constructor argument;
- mark the destination live only after `init` returns normally.

MIR must make this order explicit so future cleanup edges extend it rather than
reconstructing source evaluation.

## 2. Progress Summary

- [x] OBJ0 — Freeze the restricted inline-object contract
- [x] OBJ1 — Establish object identities and executable-body ownership
- [x] OBJ2 — Add target-independent object places and construction-aware MIR
- [x] OBJ3 — Implement x86-64 inline layout and projected-place addressing
- [x] OBJ4 — Implement the hidden receiver ABI with hand-built MIR
- [x] OBJ5 — Add class, member, and construction syntax
- [x] OBJ6 — Resolve nominal classes, fields, initializers, and methods
- [x] OBJ7 — Type-check inline objects, construction, and receiver access
- [x] OBJ8 — Lower the frontend object model into verified MIR
- [ ] OBJ9 — Enable and harden the complete native slice

A task is complete only when its checkboxes, acceptance criteria, and relevant
quality gates pass.

## 3. PR-Sized Implementation Tasks

### OBJ0 — Freeze the restricted inline-object contract

**Purpose:** Remove semantic and ABI ambiguity before decisions are duplicated
across compiler phases.

- [x] Add an implementation-profile subsection to the draft specification.
- [x] Freeze grammar, contextual names, construction/member precedence, and the
      restricted assignment form.
- [x] Define legal object-local and field-access positions.
- [x] Define straight-line field initialization, allowed right-hand sides, and
      missing/duplicate/premature-use diagnostics.
- [x] Define receiver access, member uniqueness, dispatch, and evaluation order.
- [x] Freeze primitive field layout, empty-class behavior, layout failures,
      hidden receiver classification, and internal symbols.
- [x] Record all exclusions, especially copy, destruction, general temporaries,
      polymorphism, `shared`, and aliases.
- [x] Reconcile `grammar/README.md`, the draft spec, and architecture notes.

**Tests:** Cross-document review against current parser/IR boundaries, System V
call layout, and draft lifetime rules. No behavior changes.

**Acceptance criteria:** Later milestones need not invent syntax semantics,
construction state, layout, receiver ABI, or unsupported-feature behavior.

### OBJ1 — Establish object identities and executable-body ownership

**Purpose:** Give nominal declarations and executable bodies stable,
name-independent identities before object metadata crosses phases.

- [x] Add class, field, initializer, and method identities with explicit
      owner/index relationships and deterministic display.
- [x] Choose and document one body/callable identity strategy for top-level
      functions, initializers, and methods.
- [x] Generalize parameter/local and MIR storage/value/block ownership only as
      required; do not create identities that later need name-based joining.
- [x] Retain the validated dense/sparse function tables and defer new
      class/member table abstractions until phase records exist, avoiding an
      unused general arena or identity-trait framework.
- [x] Keep neutral identities separate from phase-specific declaration data.
- [x] Preserve current function behavior and deterministic dumps.

**Tests:** Owner/index validation, ordering/display, wrong-owner lookup, sparse
body tables, and full regression coverage.

**Acceptance criteria:** All future class/member/body references can cross
boundaries by typed stable identity without source-name lookup or ad hoc maps.

### OBJ2 — Add target-independent object places and construction-aware MIR

**Purpose:** Model addressable aggregate storage before frontend syntax depends
on it and independently of x86 layout.

- [x] Add nominal class types and canonical class/member metadata to MIR.
- [x] Generalize loads/stores to a place with a storage base and field
      projections; represent scalar locals as zero-projection places.
- [x] Represent initialization into a destination place explicitly.
- [x] Represent direct calls with an optional receiver place separately from
      explicit scalar arguments.
- [x] Record initializer/method receiver access and canonical signatures.
- [x] Verify owner/type-correct projection chains, class storage, construction
      targets, receivers, arguments, and scalar-only values/results.
- [x] Extend deterministic MIR dumps without introducing target data.

**Tests:** Hand-built valid MIR; nested-form projection tests; verifier mutations
for foreign fields, wrong owners/types, object rvalues, bad construction,
receivers, and arguments; exact dumps; primitive regressions.

**Acceptance criteria:** Verified MIR describes construction, scalar field
access, and receiver calls without byte offsets or object scalar temporaries.

### OBJ3 — Implement x86-64 inline layout and projected-place addressing

**Purpose:** Centralize physical layout and address calculation while preserving
the clarity of existing scalar homes.

- [x] Add a checked target data-layout service for primitive and class
      size/alignment plus field offsets.
- [x] Compute immutable layouts once in dependency order behind a narrow API.
- [x] Diagnose incomplete metadata, recursive layouts, and arithmetic overflow
      even though source-level fields are primitive-only for now.
- [x] Give each object local one aligned contiguous frame allocation.
- [x] Lower zero-projection and field-projected scalar accesses through one
      address path.
- [x] Preserve width-correct `bool`/`u8` access and canonicalization.
- [x] Keep layout arithmetic outside general instruction selection.

**Tests:** Empty, mixed-width, padded, and reordered layouts; overflow/failure
cases; multiple frame objects; projected assembly shape; assembler acceptance;
primitive stability.

**Acceptance criteria:** One tested backend authority allocates and accesses
inline storage; semantic phases contain no byte offsets.

### OBJ4 — Implement the hidden receiver ABI with hand-built MIR

**Purpose:** Prove initializer/method execution at the backend boundary before
public syntax commits to it.

- [x] Extend internal call layout with an optional hidden integer-class
      receiver while retaining independent integer/SSE counters.
- [x] Materialize a receiver-place address at call sites without a pointer MIR
      value and bind it as addressable storage in the callee.
- [x] Lower direct initialization, read-only/mutable methods, and primitive or
      `unit` results.
- [x] Centralize collision-proof symbols based on stable identities.
- [x] Reject object-bearing external declarations through target legality.
- [x] Preserve stack alignment and calling conventions for mixed receiver,
      integer, SSE, and stack signatures.

**Tests:** Hand-built native MIR that constructs/mutates/prints; exhausted
integer and SSE banks; stack arguments; nested direct calls; symbol collisions;
assembly shape, assembler acceptance, and target failures.

**Acceptance criteria:** Verified MIR executes construction and direct methods
correctly on x86-64 without changing ordinary call behavior.

### OBJ5 — Add class, member, and construction syntax

**Purpose:** Represent the restricted source faithfully without semantic lookup
in the parser.

- [x] Add required tokens/contextual handling without globally reserving
      lifecycle names unnecessarily.
- [x] Generalize type syntax to retain named types with complete spans.
- [x] Add AST nodes for classes, fields, initializers, receiver methods, `self`,
      member access, and field assignment.
- [x] Parse construction/calls through a coherent postfix/member grammar while
      leaving semantic selection to resolution.
- [x] Preserve receiver/member/name/operator spans for diagnostics.
- [x] Recover within class bodies without discarding later declarations.
- [x] Diagnose malformed and explicitly excluded member forms cleanly.
- [x] Update AST dumps and grammar notes.

**Tests:** Tokens/contextual names, postfix precedence, malformed members and
assignments, class/top-level recovery, exact dumps, recursion limits, and parser
regressions.

**Acceptance criteria:** AST captures source shape and useful spans without
resolving types or members; errors do not corrupt following declarations.

### OBJ6 — Resolve nominal classes, fields, initializers, and methods

**Purpose:** Make resolution the sole authority for declarations and member
selection, including forward top-level class use.

- [x] Collect top-level names before bodies and diagnose cross-kind duplicates.
- [x] Assign all stable identities in deterministic source order.
- [x] Build phase-owned class declaration/definition tables while preserving
      callable declaration/body separation.
- [x] Resolve named local types, construction targets, `self`, fields, and
      methods to explicit identities.
- [x] Keep member namespaces owner-scoped and enforce the non-overloaded profile.
- [x] Resolve initializer/method parameter and local scopes; reject `self`
      outside instance bodies.
- [x] Preserve selected identities in dumps; lower phases never look up names.
- [x] Diagnose excluded positions at the earliest informative boundary.

**Tests:** Forward use, duplicates/collisions, unknown types/members,
wrong-owner fields, shadowing, `self` scope, deterministic IDs/dumps, recovery,
and resolution regressions.

**Acceptance criteria:** Successful resolved IR has no unresolved object/member
reference and every selection carries its stable identity.

### OBJ7 — Type-check inline objects, construction, and receiver access

**Purpose:** Enforce object semantics once and produce explicit typed HIR.

- [x] Add nominal class types and phase-owned class/member signatures to HIR.
- [x] Enforce primitive fields, one initializer, primitive parameters, and
      primitive-or-`unit` method results.
- [x] Validate constructor type, arity, arguments, and direct-local context.
- [x] Implement straight-line definite field initialization: exactly once, no
      early read/exit, and all fields live at normal completion.
- [x] Type field accesses as selected place operations and retain receiver
      mutability.
- [x] Enforce `fn`, `mut fn`, and implicit mutable `init` receiver rules,
      including calls on `self`.
- [x] Reject every excluded object-valued context with focused diagnostics.
- [x] Reuse structured return-flow analysis for methods; give initialization a
      separate completion rule.
- [x] Add deterministic HIR dumps.

**Tests:** Nominal mismatches, field/type errors, initializer errors,
pre-initialization reads, invalid control flow, read-only mutation, mutable
calls, `self` calls, exclusions, method returns, exact dumps, regressions.

**Acceptance criteria:** Successful HIR fully specifies construction, places,
member selection, and access; invalid forms fail before MIR.

### OBJ8 — Lower the frontend object model into verified MIR

**Purpose:** Connect HIR to the tested object MIR without adding implicit
lifecycle behavior.

- [x] Lower metadata and executable declarations in stable identity order.
- [x] Allocate one class-typed storage place per object local.
- [x] Evaluate constructor arguments left to right and initialize directly into
      that place.
- [x] Lower field access and calls through projected/receiver places and stable
      member identities.
- [x] Evaluate receivers before explicit arguments and preserve source order in
      MIR instructions/control flow.
- [x] Keep object locals out of `MirValue`; create no aggregate copies,
      returns, or temporaries.
- [x] Verify all produced MIR and report invariant failures structurally.
- [x] Extend lowering/dump fixtures without production test helpers.

**Tests:** Construction, both receiver modes, `self` calls, mixed fields,
multiple locals, nested blocks/conditionals, evaluation order, exact dumps, and
verifier integration.

**Acceptance criteria:** Every successful restricted program lowers to verified
MIR already accepted by the OBJ4 backend path.

### OBJ9 — Enable and harden the complete native slice

**Purpose:** Make the phase-tested path a dependable public feature and close
integration gaps before broader object semantics begin.

- [ ] Enable the source feature in the normal `skac` pipeline.
- [ ] Add exact native goldens covering construction, all primitive field types,
      field access, both method modes, results, conditionals, multiple objects,
      and observable call order.
- [ ] Add compile-failure goldens for exclusions and each major
      initialization/receiver diagnostic family.
- [ ] Add repeated-process AST/resolved/HIR/MIR/assembly and diagnostic
      determinism checks.
- [ ] Cover padding and mixed integer/SSE receiver ABI boundaries.
- [ ] Audit new modules for size, dependency direction, duplication, comments,
      and accidental test APIs.
- [ ] Update README status, architecture, grammar, specification, samples,
      next-slice boundaries, and roadmap checkboxes.
- [ ] Record follow-up roadmaps instead of adding copy/destruction/shared or
      polymorphism incidentally.

**Tests:** `make check`, full native goldens, dumps, compile failures,
cross-process determinism, assembler/linker acceptance, and document review.

**Acceptance criteria:** The example prints exactly `42`; all included forms
work end-to-end; exclusions fail intentionally; the full suite passes; and
documentation accurately describes implemented behavior.

## 4. Required Quality Gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `make runtime-test`
- [ ] `make golden-test`
- [ ] `make check`
- [ ] Deterministic AST, resolved, HIR, MIR, assembly, and diagnostics
- [ ] No source-name lookup below resolution
- [ ] No target offsets/registers/ABI locations in HIR or MIR
- [ ] No class object represented as a scalar MIR value in this slice
- [ ] Focused diagnostics for every explicitly excluded form
- [ ] Documentation and milestone checkboxes match behavior

## 5. Intended Follow-on Sequence

This roadmap establishes stable inline storage, not the complete object model.
A sensible progression afterward is:

1. `ref` and `mut ref` parameters for inline objects, reusing places and direct
   receiver addresses without copying;
2. inline object fields and recursive layouts, followed by a finalized
   evaluation/cleanup-order contract;
3. `destroy` and scope/control-flow cleanup over initialized places;
4. copy construction/assignment, then object value parameters/results with
   explicit return storage and permitted elision;
5. inheritance, base projections, virtual dispatch, interfaces, and casts;
6. `shared`, dynamic complete-object metadata, reference counting, and borrow
   anchors;
7. checked exceptions with partial-construction and cleanup edges.

Each should have its own roadmap. Adding class types to function signatures
before copy, destruction, and ABI semantics exist would defeat the reason this
first slice is intentionally local-only.
