# Polymorphism Roadmap

Status: planned; the compiler-maintainability prerequisite is complete and PM0
is next. The completed cleanup plan is preserved in the
[archived compiler maintainability roadmap](archive/MAINTAINABILITY_ROADMAP.md).

This roadmap extends Skald's completed exact-class object-value model with
single inheritance, base subobjects, opt-in virtual dispatch, interfaces, and
checked narrowing. It preserves the same place, initialization, copy,
destruction, return-storage, temporary, and cleanup contracts rather than
introducing a parallel polymorphic object representation.

The sequence deliberately composes base lifecycle behavior before enabling
dynamic dispatch. Interface and cast work follows verified class inheritance
so each later PR has one established dynamic-type and object-view model.

## 1. Scope and invariants

The completed profile should include:

- one direct base class through contextual `extends`, with acyclic hierarchies;
- explicit base-subobject initialization and stable base projections;
- inherited fields and methods with deterministic lookup and diagnostics;
- lifecycle capability composition across the base subobject and direct fields;
- derived-before-base destruction under the existing exactly-once cleanup model;
- opt-in `virtual` methods and explicit compatible `override` declarations;
- interface declarations, explicit `implements`, and exact conformance checks;
- read-only and mutable polymorphic alias arguments without slicing;
- statically known upcasts and explicitly checked narrowing;
- deterministic dynamic metadata, dispatch tables, dumps, assembly, and native
  behavior.

The profile must preserve these invariants:

1. Resolution remains the only source-name selection phase. Inherited members,
   overrides, interface requirements, and conversions receive stable semantic
   identities before HIR.
2. HIR owns static type, selected conversion, receiver access, dispatch kind,
   and selected lifecycle operations. It contains no vtable offsets, registers,
   or target layout.
3. MIR represents base and interface views as verified semantic places or
   explicit polymorphic operands. A class object never becomes a scalar
   `MirValue` and no aggregate bytes imply ownership.
4. Every derived object has one complete-object lifetime. Base construction,
   copy, assignment, return storage, temporary cleanup, and destruction extend
   the existing initialized-place state machine.
5. Backends consume verified dispatch and conversion operations. They do not
   repeat override selection, infer dynamic type, choose elision, or reconstruct
   cleanup order.
6. Receiver mutability remains part of exact override and interface
   compatibility. A conversion never increases access.
7. Static inline slicing, alias upcasts, interface views, and checked narrowing
   are distinct operations; no implicit conversion silently changes ownership.
8. The implementation remains deterministic across hierarchy traversal,
   metadata layout, diagnostics, phase dumps, assembly, and native execution.

Still excluded unless PM0 explicitly narrows otherwise:

- multiple class inheritance or interface inheritance;
- virtual or interface fields, default interface bodies, and overload sets;
- `shared`, allocation, reference counting, borrow anchors, or dynamic shared
  destruction;
- external polymorphic/object ABI and cross-module metadata coalescing;
- arrays, optionals, closures, generics, statics/globals, and reflection;
- exceptions, failed-construction unwinding, and partial-copy cleanup;
- unsafe pointer casts, user-visible vtable access, and user-defined conversion
  operators.

## 2. PR-sized implementation sequence

### PM0 — Freeze the executable polymorphism profile

**Purpose:** Resolve the remaining language and representation decisions before
adding syntax.

- [ ] Freeze `extends`, `virtual`, `override`, `interface`, `implements`, base
      initialization, upcast, slicing, and checked-narrowing source forms.
- [ ] Freeze inherited lookup, shadowing, override compatibility, interface
      conformance, receiver access, and deterministic diagnostic precedence.
- [ ] Freeze complete-object/base addresses and the target-independent dynamic
      type and interface-view model for inline aliases.
- [ ] Freeze base construction/copy/assignment/destruction order and interaction
      with return storage, temporaries, and permitted elision.
- [ ] Reconcile the grammar, draft specification, object-value contract, and
      exclusions; add design-consistency tests where useful.

**Acceptance criteria:** every later slice can implement syntax, ownership,
dispatch, and diagnostics without another representation-level language choice.

### PM1 — Parse and resolve hierarchy declarations

**Purpose:** Establish stable hierarchy identities without enabling polymorphic
use.

- [ ] Parse contextual class inheritance and virtual/override method modifiers.
- [ ] Parse interface signatures and explicit class conformance declarations.
- [ ] Resolve base classes, interface identities, requirements, and member
      modifiers in deterministic source order.
- [ ] Reject unknown, duplicate, malformed, and wrong-kind declarations with
      focused recovery.
- [ ] Extend exact AST and resolved dumps and grammar snapshots.

**Acceptance criteria:** valid hierarchy structure crosses resolution by stable
identity while inherited access and dispatch remain disabled.

### PM2 — Validate hierarchies, lookup, and compatibility

**Purpose:** Build the canonical semantic class graph used by every later phase.

- [ ] Reject direct and indirect inheritance cycles before HIR or layout.
- [ ] Compute deterministic inherited field/method visibility and collision
      rules without lower-phase name lookup.
- [ ] Validate virtual roots, explicit overrides, exact signatures, receiver
      mutability, result compatibility, and final/non-virtual exclusions.
- [ ] Validate interface requirement uniqueness and exact class conformance.
- [ ] Cover forward declarations, deep chains, diamonds through interfaces,
      and deterministic first-error paths.

**Acceptance criteria:** HIR receives one canonical hierarchy, override map, and
interface-conformance map for every valid class.

### PM3 — Compose base initialization and lifecycle capabilities

**Purpose:** Extend ownership to base subobjects before introducing dispatch.

- [ ] Type-check the frozen explicit base-initialization form before derived
      fields and track base liveness separately during initialization.
- [ ] Include the base subobject first in synthesized copy construction and
      assignment capability computation.
- [ ] Extend user lifecycle body rules and unavailable-capability diagnostics
      through deterministic base paths.
- [ ] Extend destruction plans to derived body, derived fields in reverse, then
      the complete base destruction sequence.
- [ ] Cover empty bases, nested fields, user/synthesized combinations, returns,
      temporaries, and exactly-once cleanup.

**Acceptance criteria:** derived objects use the existing verified ownership
model with one ordered base lifecycle contribution.

### PM4 — Add semantic base places and static access

**Purpose:** Make inherited access and exact base views explicit above the
backend.

- [ ] Add HIR base projections with selected declarations and preserved access.
- [ ] Type-check inherited field access, direct non-virtual calls, alias upcasts,
      and the frozen inline slicing form.
- [ ] Add MIR base projections and verifier rules for owner chains, terminal
      types, overlap, liveness, and mutation.
- [ ] Keep slicing as selected base copy construction, never raw prefix bytes.
- [ ] Extend HIR/MIR dumps and malformed-IR tests.

**Acceptance criteria:** all static inheritance behavior is target-independent,
identity-based, and executable without dynamic dispatch.

### PM5 — Lower base layout and lifecycle behavior on x86-64

**Purpose:** Execute verified single inheritance without leaking target layout
into MIR.

- [ ] Lay out the direct base subobject according to the frozen ABI with checked
      derived-field offsets and complete-object alignment.
- [ ] Lower base projections for local, receiver, parameter, return, argument,
      temporary, and alias storage bases.
- [ ] Lower base construction, copying, assignment, slicing, and destruction
      through selected semantic operations.
- [ ] Retain structured errors for corrupt hierarchy metadata and displacement
      overflow; add no implicit aggregate-copy path.
- [ ] Add native traces for deep chains, padding, empty bases, cleanup, and
      mixed scalar/object call pressure.

**Acceptance criteria:** static inheritance and complete lifecycle composition
execute deterministically on x86-64.

### PM6 — Represent and verify virtual dispatch

**Purpose:** Introduce dynamic method selection without backend-owned semantics.

- [ ] Assign stable virtual slots from canonical override families.
- [ ] Represent direct versus virtual calls explicitly in HIR and MIR.
- [ ] Carry the frozen dynamic class/view metadata through permitted alias
      bindings without changing ownership.
- [ ] Verify slot ownership, signature/access agreement, receiver liveness, and
      complete-object adjustment requirements.
- [ ] Extend dumps and corruption tests for virtual metadata and calls.

**Acceptance criteria:** MIR identifies exactly one verified virtual family and
receiver view for every dynamic call.

### PM7 — Lower virtual dispatch on x86-64

**Purpose:** Make opt-in virtual calls executable under a checked internal ABI.

- [ ] Emit deterministic per-class dispatch metadata from stable identities.
- [ ] Map polymorphic alias views to the frozen internal register/stack layout.
- [ ] Lower virtual calls while preserving receiver-before-argument evaluation,
      scalar/object arguments, result storage, and temporary cleanup.
- [ ] Reject malformed metadata or unsupported external signatures before
      instruction selection.
- [ ] Add native tests for base/derived calls, mutable access, deep overrides,
      recursion, stack pressure, and sliced inline bases.

**Acceptance criteria:** calls through base aliases select the dynamic override,
while direct and sliced calls retain their specified static behavior.

### PM8 — Add interfaces and interface dispatch

**Purpose:** Generalize verified polymorphic views without adding interface
objects.

- [ ] Type-check class-to-interface and frozen interface-to-root alias
      conversions from canonical conformance metadata.
- [ ] Represent interface view and requirement identities explicitly in HIR/MIR.
- [ ] Verify access, signature, view adjustment, lifetime, and non-ownership.
- [ ] Emit deterministic interface tables and lower interface calls on x86-64.
- [ ] Cover multiple implemented interfaces, reordered requirements, mutable
      methods, forwarding, overlap, and wrong-conformance diagnostics.

**Acceptance criteria:** interface aliases dispatch to the conforming complete
object without permitting standalone inline interface storage.

### PM9 — Add checked narrowing and casts

**Purpose:** Complete the frozen conversion profile over the same metadata.

- [ ] Type-check explicit base/interface narrowing and its scoped success form.
- [ ] Preserve source access and lifetime; reject escaping or access-increasing
      narrowed aliases.
- [ ] Represent static success, runtime check, failure behavior, and view
      adjustment explicitly in MIR.
- [ ] Lower checks against deterministic dynamic metadata without object-graph
      search or ownership transfer.
- [ ] Cover success/failure, deep bases, interfaces, grouping, nested calls, and
      invalid cast diagnostics.

**Acceptance criteria:** every cast is explicit, checked when required, and
produces only a bounded non-owning view.

### PM10 — Harden, document, and publish polymorphism

**Purpose:** Make the restricted polymorphism profile dependable and prepare
shared ownership.

- [ ] Complete native and compile-failure goldens across hierarchy declarations,
      lifecycle composition, layout, static access, virtual/interface dispatch,
      conversions, casts, aliases, object values, and cleanup.
- [ ] Assert exact output/status/stderr and cross-process determinism for every
      phase product, metadata artifact, assembly, and diagnostic.
- [ ] Audit source-reachable assertions, dispatch-table assumptions, complete-
      object adjustments, initialized-place transitions, and backend legality.
- [ ] Update grammar, specification, architecture, README, debugging, samples,
      golden documentation, and future boundaries.
- [ ] Run the complete quality gate, archive this roadmap, update the archive
      index, and publish shared ownership as the next object-model roadmap.

**Acceptance criteria:** restricted polymorphism is explicit, deterministic,
exactly owned, structurally verified, and fully documented.

## 3. Quality and completion gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `make runtime-test`
- [ ] `make golden-test`
- [ ] `make check`
- [ ] No source-name lookup below resolution
- [ ] No class object represented as a scalar MIR value
- [ ] Explicit base initialization, lifecycle, view, dispatch, and cast state
- [ ] No target layout, vtable offset, or ABI location in HIR/MIR
- [ ] No accidental shared, allocation, exception, or external-object ABI
- [ ] Exact deterministic artifacts, metadata, diagnostics, and observations
- [ ] Touched Rust modules retain concise facades and cohesive ownership
- [ ] Living documentation and roadmap checkboxes match behavior

The slice is complete only when all PM0–PM10 acceptance criteria and quality
gates pass. Shared ownership must reuse the complete-object metadata, dynamic
dispatch, lifecycle, and alias-view contracts established here.
