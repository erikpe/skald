# Shared Ownership and Heap Allocation

Status: **implemented on x86-64**. This
document is authoritative for the source-visible semantics of `shared T`,
heap allocation, shared copying and assignment, deterministic last-owner
destruction, borrowing from shared storage, and strong cycles. The
[status matrix](STATUS.md) remains authoritative for current compiler support.
The [implemented grammar](GRAMMAR.md) accepts these forms. Resolution retains
stable targets and allocation modes, and typed HIR records shared targets,
ordinary allocation, and copy-versus-adopt owner provenance. Exact-class
shared locals now support ordinary allocation, named-owner copying,
secure-before-release assignment, full-expression ownership boundaries, and
normal cleanup through verified MIR and native execution. Same-target shared
parameters and results now transfer owners through internal functions,
initializers, methods, and interface requirements, including produced
arguments and results. Shared fields participate in initialization, field
replacement, copying, assignment, and destruction in typed HIR and verified
MIR, and execute as one-word owning edges on x86-64.
Compatible shared up-views, stable shared-local and value-parameter member
access, virtual/interface dispatch, and `is` type tests now execute without
changing allocation identity or dynamic metadata. Shared-backed alias
arguments, method/interface receivers, and plain checked place casts now use
explicit verified hidden owners for fields, nested places, and produced
owners; stable owners borrow directly. Checked places may feed receivers,
alias arguments, field access and mutation, and owning inline copy,
assignment, argument, result, and slicing contexts. Owner-preserving
`(shared T) source` casts execute with retain for a named source and transfer
for a produced source, after any required metadata check. Explicit
`new T(copy source)` allocation accepts inline, alias, produced, and
shared-backed checked sources.
Compiler and runtime realization is defined separately in the
[shared-ownership implementation contract](../compiler/SHARED_OWNERSHIP.md).
Object conversion syntax and the complete inline/alias/shared direction matrix
are owned by [Object Casts](OBJECT_CASTS.md).
Ordinary overload and explicit-copy semantics are owned by
[Classes and Lifecycle](CLASSES_AND_LIFECYCLE.md) and are an implementation
prerequisite rather than redefined by shared allocation.

Pointee access has explicit source forms. Prefix `*source` selects a bounded
non-owning place from a `shared T` owner, while `source->member` selects one
member through one shared edge and evaluates `source` once. The place is
accepted by every implemented object-place consumer: field access and
mutation, class or interface calls, `ref` and `mut ref` arguments, checked
casts and type tests, and target-directed inline local, field, value-parameter,
result, assignment, slicing, `T(copy source)`, and `new T(copy source)` copies.
Those uses retain the corresponding stable-owner or hidden-anchor lifetime
behavior. Dereferencing does not allocate,
copy an inline object, or transfer a strong owner. Non-shared operands are
rejected.

Owner contexts remain deliberately distinct: shared initialization,
assignment, arguments, results, up-views, and `(shared T) source` consume the
handle itself. A dereferenced place is rejected in those contexts rather than
being silently converted back into ownership. Whole-pointee assignment
`*owner = source` is also unsupported; use `owner = replacement` to replace
the handle or `owner->field = value` to mutate a supported field. Potential
whole-pointee copy assignment is a deferred direction because its behavior for a derived
allocation viewed through `shared Base` needs a separate lifecycle design.

## Safety contract

Every live Skald value denotes a value. A `shared T` value is therefore always
a valid owning handle to one live heap allocation; it is never null, empty,
dangling, or moved from. The executable
[optional-values contract](OPTIONAL_VALUES.md#shared-ownership) uses
`shared? T` to represent absence around that ordinary owner. It never makes a
`shared T` null. A successful `owner!` first retains a nonzero canonical
handle into an ordinary owner; only that secured owner may enter existing
dereference, cast, anchor, metadata, and release operations.

Safe Skald code cannot:

- observe or manufacture a dangling shared handle;
- use a shared allocation after its lifetime ends;
- obtain a non-owning alias whose source can die before the alias ends; or
- inspect or modify a reference count.

Strong ownership keeps an allocation live. Non-owning `ref` and `mut ref`
bindings provide access without extending lifetime themselves. The compiler
inserts hidden owning anchors where a borrow cannot otherwise be proved to
remain live for its complete scope. These guarantees do not depend on tracing
garbage collection or a conservative runtime root scan.

## Type and construction forms

The source forms are:

```ska
shared Widget
shared Base
shared Drawable
shared Obj

new Widget(arguments)
new Widget(copy source)
```

`shared` and `new` are contextual words in these exact positions. A shared
target may be a concrete class, an ancestor class, an interface, or `Obj`.
`new` must name a concrete, constructible class.

Allocation creates one complete object of the named concrete class and returns
one produced strong owner. The expected type may immediately view that owner
as the same class, an ancestor, a conformed interface, or `Obj`; this preserves
the allocation and its complete dynamic class rather than slicing it.

There are two allocation modes. `new ConcreteClass(arguments)` selects one
ordinary initializer overload under the
[class construction rules](CLASSES_AND_LIFECYCLE.md#ordinary-initializer-overloads).
`new T(copy source)` is explicit copy allocation: the contextual `copy` marker
selects `T`'s copy-constructor capability exactly once in the new allocation.
The marker takes exactly one source and does not form an ordinary initializer
argument. Conversely, `new T(source)` participates only in ordinary
initializer overload resolution and never falls back to copy construction.
Ordinary allocation and initializer selection cross typed HIR. Compatible
same/ancestor/interface/`Obj` owner transfers, local initialization and
assignment, internal callable parameters and results, shared fields, and
polymorphic use through stable shared locals and value parameters cross
verified target-independent MIR and execute on the current x86-64 backend.
Explicit copy allocation uses the same verified checked-place and hidden-anchor
pipeline and executes through the selected copy-constructor operation.

The copy-allocation target must be concrete and copy-constructible. The source
may be an existing or produced inline object, a `ref` or `mut ref` alias, or
an explicitly dereferenced shared pointee such as `*owner`, subject to the
checked-place rules in [Object Casts](OBJECT_CASTS.md). It executes in this
order:

1. evaluate the copy source exactly once and establish any required temporary
   or hidden owning anchor;
2. select the exact `T` place through the target-directed checked-copy
   relation, terminating on a required failed dynamic check;
3. allocate storage for one exact `T`;
4. copy-construct the payload from that checked place; and
5. publish the completed allocation as one produced `shared T` owner.

A failed cast therefore does not allocate the copy destination; source
evaluation may already have performed its own operations. The source view and
anchor remain live through the copy and until its result owner is secured. The
explicit copy-constructor operation is not eligible for copy elision.

The named allocation class determines the complete dynamic class. For example,
`new Animal(copy dog)` deliberately copies and slices to an exact `Animal`;
`new Dog(copy animal)` first checks the view and then creates an exact `Dog`.
To retain a statically known `Dog` while satisfying `shared Animal`, use
`new Dog(copy dog)` and the ordinary shared upcast.

The compiler supplies the target-directed static selection or dynamic check
when required. An explicit inner cast is optional and expresses an additional
refinement, as in `new Animal(copy (Dog) source)`. It does not change the fact
that the allocation has exact dynamic class `Animal`.

Only source forms headed by `new` create a shared allocation. Reads,
assignments, calls, results, casts, upcasts, and hidden anchors may create,
transfer, or end owners of an allocation that already exists, but none creates
another allocated object. An inline value or alias cannot be converted to a
shared owner by casting; it can only be copied into a distinct allocation by
the explicit copy-allocation form.

Shared types are distinct from inline exact-class values and from non-owning
aliases. There is no implicit conversion between an inline owning value and a
shared owner. Shared values are permitted as locals, value parameters,
results, class fields, and explicitly initialized static fields. They are not
permitted in external signatures. The [static-field contract](STATIC_FIELDS.md)
permits initializer-free `shared? T` storage, initially absent, and explicitly
initialized `shared T` or `shared? T` storage through ordinary adoption or
copy. It does not make zero a valid ordinary `shared T` handle. Static owners
use ordinary replacement, cast, view, and anchoring rules while executing;
their final owner remains retained until reverse normal-return shutdown is
implemented.

## Strong-owner value semantics

Each live shared storage location or owning temporary contributes exactly one
strong owner. The abstract owner operations are:

- **copy** — create another owner of the same allocation;
- **adopt** — transfer a newly produced owner into its destination without
  creating an additional owner; and
- **release** — end one owner, destroying and deallocating the allocation if it
  was the last.

These operations define behavior; they do not make moved-from values or
explicit ownership primitives available in source.

### Reads, parameters, and results

Reading a named shared local, parameter, or field as a value copies it. The
source remains a live owner and the destination becomes another owner.

A produced shared result, including either `new` allocation form and a call
returning `shared T`, already owns its result. A destination adopts that owner
rather than copying it and immediately releasing a redundant temporary.

For a shared value parameter:

- a named argument is copied at its argument position;
- a produced argument transfers its existing owner;
- the callee adopts the incoming owner into the parameter; and
- the parameter releases that owner during ordinary parameter cleanup.

Returning a named shared place copies it into the result. Returning a produced
shared result transfers its existing owner. The completed result is owned by
the caller.

### Assignment

Shared assignment is valid for a live, mutable shared local, value parameter,
or field when the right side has a compatible shared target. It executes in
this order:

1. evaluate the right side once;
2. secure its destination owner, by copying a named place or adopting a
   produced owner;
3. release the destination's old owner; and
4. store the secured owner in the destination.

Securing the incoming owner before releasing the old owner makes direct and
indirect self-assignment safe. The destination is always initialized and never
passes through a source-visible empty state.

### Temporaries and cleanup

A produced owner that is not adopted by longer-lived storage remains an owning
temporary through its full expression. Shared temporaries are released in
reverse completion order at the existing full-expression boundary.

Shared locals and value parameters participate in the existing normal cleanup
order alongside inline class owners. Their releases occur at the point where
that storage's lifetime ends. Retains and releases are not independently
source-observable, but the resulting last-owner destructor timing is.

## Access and mutation

Ownership and access are separate. A `shared T` owner permits ordinary mutable
access to its allocated `T` object; sharing does not make the pointee immutable
and does not provide exclusivity.

Read-only access to an enclosing inline object is shallow with respect to a
shared field. It prevents replacing that field's shared handle, but it does
not make the separately allocated pointee read-only. Code may therefore call a
mutable method through a shared field while it has only read-only access to the
enclosing object.

Non-owning aliases remain deliberately non-exclusive. Multiple overlapping
`mut ref` borrows of a shared pointee are legal, just as overlapping mutable
alias arguments are legal for inline objects. Effects follow source evaluation
order; Skald does not promise data-race safety or thread-safe sharing in this
initial profile.

`*owner` and `owner->member` expose the pointee of stable shared locals and
value parameters as class or interface receivers, alias arguments, and roots
of inherited base and field projections. A replaceable shared field is copied
into a hidden strong owner before its pointee is exposed; a produced owner is
adopted into hidden storage. These anchors cover inline payload subobjects,
remain live through the complete call, and are released in reverse completion
order after the result is secured.

The expression `*owner is T` is available for shared class, interface, and
`Obj` pointees. It reads the allocation header's dynamic metadata and neither
retains nor releases the owner. Statically guaranteed and impossible outcomes
use the same closed-world classifier as inline and alias sources. The raw
owner form `owner is T` is rejected because it omits pointee selection.

## Shared fields and lifecycle

A shared field is an owning field, not inline containment. Its allocation is a
separate complete object and a shared edge does not participate in the
finite-inline-containment cycle check.

Ordinary initialization must initialize every shared field exactly once, using
a compatible shared expression. User-defined copy construction and assignment
use ordinary shared initialization and assignment operations in their bodies.
Synthesized lifecycle behavior is:

- copy construction processes fields in declaration order and copies each
  shared field;
- copy assignment processes fields in declaration order, securing each
  incoming shared owner before releasing the corresponding old owner; and
- destruction processes fields in reverse declaration order, releasing each
  shared field where an inline class field would recursively be destroyed.

Inheritance keeps the existing lifecycle composition: the base step precedes
direct fields for construction and assignment, while derived destruction runs
the user body, direct fields in reverse declaration order, and then the base
sequence.

## Last-owner destruction

Releasing the last strong owner runs the complete most-derived object's
ordinary destruction sequence exactly once and then deallocates its heap
allocation exactly once. The dynamic class, not the shared handle's static
target, selects this destruction.

The object and its allocation remain live while its user destructor and field
cleanup execute. Shared fields released during that cleanup may in turn
destroy other allocations. After complete-object destruction finishes, no
source operation can reach the allocation.

There is no explicit early-release operation. A program influences destruction
time only through ordinary value lifetimes and assignment.

## Polymorphic views

Shared upcasts from a class to an ancestor, a conformed interface, or `Obj`
preserve one allocation, one owner relationship, and the complete dynamic
class. They never slice. Direct, virtual, and interface method calls operate on
the allocated complete object using the static target for selection and the
dynamic class where dispatch requires it.

`is` accepts an explicitly dereferenced shared class/interface/`Obj` pointee
and uses the existing static-or-runtime classification. It evaluates the
owner source once and does not change ownership.

The cast profile provides two distinct shared-backed operations:

```ska
((Dog) *shared_animal).speak();
var copied: Dog = (Dog) *shared_animal;
var owner: shared Dog = (shared Dog) shared_animal;
```

`(Dog) *shared_animal` is a checked non-owning place view. An immediate call
borrows it; an inline `Dog` destination copy-constructs an independent exact
`Dog` and may slice a more-derived dynamic object. `(shared Dog) shared_animal`
creates or transfers an owner of the same allocation; it does not allocate,
copy payload, or slice. The complete matrix is defined by
[Object Casts](OBJECT_CASTS.md#complete-ownership-matrix).

## Borrow anchors

An anchor is a compiler-created strong owner whose sole purpose is to keep the
borrowed complete allocation live. It has no source name or source type syntax.
It is established at the source evaluation position that selects the borrowed
object and released after the borrow's required lifetime.

### Call-scoped aliases

The following rules apply when a `ref` or `mut ref` argument, method receiver,
or checked place cast reaches an object through shared ownership:

| Source | Lifetime proof |
|---|---|
| Existing shared local or shared value parameter | Its existing owner remains live through the ordinary call; no extra copy is needed because the callee cannot rebind the caller's place. |
| Produced shared value | The produced owning temporary is extended through the call and acts as the anchor. |
| Shared field or another replaceable shared place | A hidden copied owner is created when that receiver or argument is evaluated and retained through the call. |
| Inline subobject reached through a shared allocation | The owner of the complete allocation anchors every inline subobject within it. |
| Existing `ref` or `mut ref` parameter being forwarded | The enclosing borrow's lifetime guarantee is inherited; no new shared owner is inferred. |

A receiver is selected and anchored before explicit arguments. Arguments are
then evaluated and anchored left to right. All required anchors remain live
through the call and are released with full-expression temporaries in reverse
completion order after the result has been secured.

The compiler must not omit or merge anchors when doing so could change
last-owner destruction timing. Anchor elimination is permitted only as a
semantics-preserving optimization after the required ownership operations are
represented and verified.

### Checked place casts

A checked place cast from a shared source uses the same anchor classification.
An existing shared local or value parameter remains live through the consuming
full expression. A replaceable shared field or nested place is copied into a
hidden anchor at cast-source evaluation. A produced shared source keeps its
produced owner through the expression.

The cast view ends before its anchor is released. If the view supplies a method
receiver, alias argument, inline copy source, or copy-allocation source, the
call or copy completes and its result is secured first. Copy allocation
performs its check before allocating its destination. Any cast failure
terminates and makes no remaining cleanup guarantee.

## Deferred dynamic cloning

Copy allocation always creates the concrete class named by `new`. It does not
inspect the source's metadata to choose an allocation class and cannot preserve
an arbitrary source dynamic class through an ancestor, interface, or `Obj`
view.

A dynamic cloning facility is deferred beyond the initial shared profile. It
may eventually use a `clone()` method convention or dedicated syntax, but its
dispatch, result type, allocation authority, copy behavior, and failure
contract require a separate design. No cast or current `new` form implies that
facility.

## Strong cycles

Reference counting does not collect a cycle containing only strong shared
edges. Such a cycle is permitted to leak: its allocations remain live, and
their destructors do not run merely because program entry returns or the
process exits.

This is a resource-liveness limitation, not a memory-safety exception. Every
handle in the cycle still denotes live storage. A future `weak T` feature may
provide non-owning cycle-breaking handles, but weak ownership is not part of
this design.

## Unrecoverable failures

Allocation failure and strong-count overflow terminate the process
unsuccessfully. They do not return a value, expose a catchable error, or
guarantee cleanup of remaining live values. Strong-count underflow, invalid
handles, double finalization, and use after release are compiler or runtime
defects, not source failure cases.

The frozen [common panic policy](ERRORS.md#frozen-panic-design) reports valid
host allocation failure and source-reachable ownership-count overflow through
one reporter. Count underflow and the invalid states above remain hard
compiler-defect traps; they must not be converted into user-facing reports.

Future checked exceptions may extend allocation and cleanup behavior only by
explicitly revising this contract. They are not implied by the shared
ownership design.

## Exclusions

This profile does not include:

- generalized `shared T?` or `shared? T?` boxes;
- aliases whose designated container type is `shared? T`;
- weak ownership;
- explicit early release or user-visible reference counts;
- raw pointers or unsafe handle construction;
- casting an inline object or alias into shared ownership;
- dynamic cloning that preserves an arbitrary source dynamic class;
- custom allocators;
- shared values in external signatures or other public object ABI;
- atomic reference counts, concurrency, or thread-safety guarantees;
- non-optional static or global shared storage; the implemented static-field
  profile permits only optional shared owners as class-owned statics;
- recoverable allocation failure; or
- exceptional cleanup or failed-construction unwinding.

These exclusions bound the current implementation. The implemented
[array contract](ARRAYS.md) extends these rules with exact non-polymorphic
`shared T[]` and `shared? T[]` ownership; it does not change the
class/interface/`Obj` boundary above.
The implemented
[explicit array element-list form](ARRAYS.md#explicit-element-list-construction)
reuses ordinary target compatibility at each shared-owner element position. A
named owner is copied/retained, while a produced owner is transferred/adopted;
the optional-owner form applies the same rule only when present. Listing one
named owner twice therefore creates two owners of one allocation, while two
separate `new` expressions produce distinct allocations. These rules do not
make arrays covariant or add an implicit shared target conversion.
Other omissions do not reserve syntax or freeze their eventual designs.
