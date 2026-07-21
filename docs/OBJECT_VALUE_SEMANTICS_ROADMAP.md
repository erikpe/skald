# Object Value Semantics Roadmap

Status: in progress; OVS0–OVS4 are complete and OVS5 is next.

This roadmap extends Skald's place-only inline-object core with explicit copy
construction, copy assignment, and carefully bounded object values. It builds
on deterministic destruction: every new owning destination and temporary must
have a precise initialization point and exactly-once cleanup before object
parameters, results, or elision become executable.

The sequence intentionally starts with source semantics and local operations.
Aggregate ABI and return-storage work comes only after target-independent IR
can represent ownership and cleanup without treating an object as a scalar.

## 1. Scope and invariants

The completed profile should include:

- the exact-class copy constructor `init(ref other: T)` and copy assignment
  member `assign(ref other: T)`;
- deterministic selection of user-defined or synthesized copy operations;
- direct initialization of an owning local from another exact-class place;
- assignment to an already-live exact-class local or projected field;
- self-assignment with the language-defined member behavior;
- object value arguments and parameters with caller-to-callee copy ownership;
- object results through explicit caller-provided return storage;
- bounded owning temporaries with explicit full-expression cleanup;
- the specification's permitted copy-elision cases;
- recursive composition through acyclic inline class fields;
- exact observable construction, assignment, destruction, and evaluation order;
- deterministic diagnostics, phase dumps, assembly, and native execution.

The profile must preserve these invariants:

1. Class objects remain addressable places in MIR. An object operation names a
   source place and/or destination place; it never creates a scalar class
   `MirValue`.
2. Resolution is the only name-selection phase. Copy operations use stable
   lifecycle identities or an explicit synthesized-operation identity.
3. HIR owns source legality, selected copy operation, exact class type, and
   whether a destination is uninitialized or already live.
4. MIR owns evaluation order, destination initialization, temporary lifetime,
   cleanup, and return-storage flow without byte offsets or registers.
5. A destination becomes live only after its copy constructor completes.
   Assignment does not end or restart the destination lifetime.
6. Destruction remains exactly once for every successfully initialized owning
   place, including parameters, results, and materialized temporaries.
7. Backends lower verified aggregate operations; they do not infer ownership,
   lifetime, copy selection, or elision.

Still excluded unless OVS0 explicitly narrows otherwise:

- moves or destructive transfer, implicit relocation, and trivial byte-copy
  shortcuts that bypass observable members;
- slicing, inheritance, base subobjects, virtual dispatch, interfaces, casts,
  and dynamic type metadata;
- `shared`, allocation, reference counting, deallocation, and borrow anchors;
- arrays, optionals, statics, globals, closures, and cross-module values;
- exceptions, failed-copy cleanup, unwinding, and throwing lifecycle members;
- external object ABI, variadics, alternate calling conventions, and AArch64;
- explicit early destruction or whole-object replacement through aliases.

## 2. PR-sized implementation sequence

Each task should remain independently reviewable. A task may include small
facade or table refactorings needed to keep one canonical representation.

### OVS0 — Freeze the executable copy/value profile

**Purpose:** Turn the draft's broad copy rules into one implementation-ready
normal-flow contract.

- [x] Freeze copy-constructor and copy-assignment syntax, signatures,
      coexistence with ordinary initializers, duplicate behavior, and stable
      identities.
- [x] Freeze which operations are required, synthesized, deleted, or diagnosed
      for primitive-only, nested, empty, and non-copyable classes.
- [x] Freeze local copy initialization, live-object assignment, self-assignment,
      evaluation order, and alias visibility.
- [x] Freeze object argument/parameter ownership, result storage, temporary
      boundaries, cleanup order, and permitted elision.
- [x] Define exactly which copy-member side effects elision may omit.
- [x] Reconcile the grammar, draft specification, destruction contract, and
      exclusions; add consistency tests where useful.

**Acceptance criteria:** every later slice can implement source behavior,
ownership, and diagnostics without making another language-design decision.

### OVS1 — Resolve copy lifecycle declarations

**Purpose:** Add stable declaration and operation identities without enabling
object values.

- [x] Parse `assign(ref other: T) { ... }` as a dedicated lifecycle member and
      retain `assign` as a contextual spelling.
- [x] Permit and distinguish the frozen ordinary/copy initializer set.
- [x] Resolve exact owner-class signatures and reject malformed or duplicate
      declarations deterministically.
- [x] Represent selected user-defined and synthesized copy operations without
      lower-phase source-name lookup.
- [x] Extend exact AST/resolved dumps and recovery tests.

**Acceptance criteria:** valid copy declarations cross resolution by stable
identity, while object copy expressions remain rejected.

### OVS2 — Type-check copy bodies and compute class capabilities

**Purpose:** Complete lifecycle-body legality and determine whether each class
supports copy construction and assignment.

- [x] Represent copy construction and assignment in HIR with an implicit
      mutable destination `self` and read-only source alias.
- [x] Apply initializer liveness to copy constructors and live-object rules to
      assignment bodies.
- [x] Validate recursive class-field capabilities in deterministic dependency
      order, including empty and forward-declared classes.
- [x] Define synthesized operations as ordered semantic field operations, not
      raw memory copies.
- [x] Reject cycles, missing capabilities, invalid field access, object-value
      escape, and unsupported lifecycle calls with focused diagnostics.

**Acceptance criteria:** HIR exposes one canonical copy capability and selected
operation for every class; invalid bodies cannot reach MIR.

### OVS3 — Add local copy construction and assignment to HIR

**Purpose:** Introduce the first source object-value contexts over existing
stable places.

- [x] Accept direct exact-class local initialization from a local, receiver,
      field, or read-only alias place.
- [x] Accept exact-class assignment to live owning locals and projected fields
      within the frozen access boundary.
- [x] Select copy construction versus copy assignment explicitly in HIR.
- [x] Preserve left-to-right source evaluation and define overlap/self-assignment
      behavior without inventing moves.
- [x] Keep unsupported parameters, results, general temporaries, and external
      signatures diagnosed until their slices land.

**Acceptance criteria:** typed HIR represents local copy operations as selected
source/destination place operations with no class rvalues.

### OVS4 — Model verified copy operations and temporary ownership in MIR

**Purpose:** Extend destination-oriented MIR before target lowering.

- [x] Add explicit copy-construction and copy-assignment operations over exact
      semantic places and stable lifecycle operations.
- [x] Extend initialized-place state so a destination becomes live only after
      successful normal completion.
- [x] Represent synthesized field composition in target-independent order.
- [x] Define bounded owning temporary storage and full-expression cleanup.
- [x] Verify ownership, liveness, access, type, overlap, operation capability,
      and exactly-once cleanup across control flow.
- [x] Preserve the no-class-`MirValue` invariant and extend exact MIR dumps.

**Acceptance criteria:** valid local copy/assignment MIR is fully structural;
corrupt or hand-built MIR fails before backend instruction selection.

### OVS5 — Lower local copy construction and assignment on x86-64

**Purpose:** Make local copy behavior observable before changing callable ABI.

- [ ] Lower user lifecycle calls through the existing hidden-receiver and alias
      address machinery.
- [ ] Lower synthesized recursive field operations in MIR-defined order.
- [ ] Extend frame planning for explicit destinations and bounded temporaries.
- [ ] Preserve scalar intermediates and live aliases across lifecycle calls.
- [ ] Retain checked layouts and structured malformed-MIR errors; add no
      implicit `memcpy` path.
- [ ] Add native traces for copy, assignment, self-assignment, nesting,
      temporaries, cleanup, padding, and empty classes.

**Acceptance criteria:** local copy construction and assignment execute with
exact deterministic lifecycle order and no backend-owned semantic selection.

### OVS6 — Add owning object parameters and arguments

**Purpose:** Establish caller-to-callee ownership without coupling language IR
to System V aggregate classification.

- [ ] Accept exact-class value parameters and object-place arguments only when
      the selected copy constructor is available.
- [ ] Make caller-side evaluation and callee destination ownership explicit.
- [ ] Specify cleanup responsibility on every supported normal exit.
- [ ] Add a target-independent calling convention contract for owned object
      parameters, then map it to x86-64 storage/address passing.
- [ ] Cover mixed scalar/object/alias signatures, overlap, recursion, stack
      pressure, and deterministic diagnostics.
- [ ] Keep object-bearing `extern fn` signatures excluded.

**Acceptance criteria:** a value parameter owns an independent copy with one
cleanup, while aliases retain their existing non-owning semantics.

### OVS7 — Add object results and return storage

**Purpose:** Return owning objects without scalarizing or copying anonymous
aggregate bytes.

- [ ] Accept exact-class function and method results under the frozen copy
      capability rules.
- [ ] Represent caller-provided uninitialized return storage in HIR/MIR and the
      internal ABI.
- [ ] Initialize return storage before callee locals clean up and transfer
      ownership exactly once to the caller.
- [ ] Diagnose wrong-class returns, missing returns, alias escape, and invalid
      external results.
- [ ] Cover conditionals, nested calls, recursion, mixed scalar arguments,
      return-value cleanup order, padding, and empty classes.

**Acceptance criteria:** every normal object return constructs one caller-owned
result in explicit storage with verified lifetime and cleanup.

### OVS8 — Implement bounded temporaries and permitted elision

**Purpose:** Complete the frozen expression profile while keeping optimization
separate from correctness.

- [ ] Materialize every required object temporary in explicit owning storage
      with a full-expression cleanup boundary.
- [ ] Implement only the frozen direct-initialization and return elision cases.
- [ ] Represent elision as destination selection or a target-independent pass,
      never as a backend guess.
- [ ] Preserve evaluation order and document the permitted omission of
      side-effectful copy/destruction calls.
- [ ] Compare elided and non-elided ownership graphs and verify each destination
      is initialized and cleaned exactly once.

**Acceptance criteria:** materialization and permitted elision both follow the
same verified ownership model and produce the specified observable behavior.

### OVS9 — Harden, document, and publish object values

**Purpose:** Make the full restricted profile dependable and prepare the
polymorphism roadmap.

- [ ] Add complete native and compile-failure goldens across declarations,
      capabilities, local operations, parameters, results, temporaries,
      elision, aliases, nesting, control flow, layout, and cleanup.
- [ ] Assert exact output/status/stderr and cross-process determinism for every
      phase product, assembly, and diagnostics.
- [ ] Audit source-reachable assertions, aggregate ABI assumptions, and all
      initialized-place transitions.
- [ ] Update grammar, specification, architecture, README, debugging, samples,
      golden documentation, and future boundaries.
- [ ] Run the complete quality gate, archive this roadmap, update the archive
      index, and publish polymorphism as the next object-model roadmap.

**Acceptance criteria:** restricted object values are explicit, deterministic,
exactly owned and cleaned, structurally verified, and fully documented.

## 3. Quality and completion gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `make runtime-test`
- [ ] `make golden-test`
- [ ] `make check`
- [ ] Stable lifecycle identities and no source-name lookup below resolution
- [ ] No class object represented as a scalar MIR value
- [ ] Explicit initialization, ownership transfer, temporary, and cleanup state
- [ ] No target layout or ABI location in HIR/MIR
- [ ] No accidental move, inheritance, shared, exception, or external-object ABI
- [ ] Exact deterministic artifacts, diagnostics, and native observations
- [ ] Touched Rust modules retain concise facades and cohesive ownership
- [ ] Living documentation and roadmap checkboxes match behavior

The slice is complete only when all OVS0–OVS9 acceptance criteria and quality
gates pass. Later polymorphism must build on these copy, destination, and
cleanup contracts rather than introducing a second object-value model.
