# Skald Aliases and Ownership

Status: authoritative for executable primitive, class, `Obj`, inline
optional-container, and array aliases. Interface views
follow the same source rules and lower through verified MIR; their backend
execution boundary is owned by [polymorphism](POLYMORPHISM.md). Shared-backed
call borrows and their hidden owner anchors are implemented as specified by
[Shared Ownership and Heap Allocation](SHARED_OWNERSHIP.md). Local aliases and
aliases into other future value families remain unfrozen. Array-specific
descriptor and detached-backing behavior belongs to [Arrays](ARRAYS.md).
Feature maturity is authoritative in the [status matrix](STATUS.md).

The [grammar](GRAMMAR.md#compilation-unit-and-declarations) defines accepted
parameter syntax, [functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md)
defines call and argument order, and
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#object-places-and-projections)
defines object places, copying, and owning-object lifetime.

## Binding modes

An alias parameter is a non-owning name for an eligible existing place. Its
binding mode is separate from the static target:

```ska
fn inspect(ref value: Item) -> i64 {
    return value.read();
}

fn update(mut ref value: Item, amount: i64) -> unit {
    value.add(amount);
}
```

`ref value: Item` provides read-only access and `mut ref value: Item` provides
mutable access. Neither spelling constructs a reference value or a new type.
Within the body, `value` still designates an `Item` place.

Alias parameters are implemented on internal top-level functions, ordinary
initializers, and methods. Copy constructors and copy-assignment members use
the same read-only source-binding semantics as part of their more specialized
[lifecycle declarations](CLASSES_AND_LIFECYCLE.md#lifecycle-declarations).
External declarations may parse alias syntax for recovery, but such signatures
are semantically invalid.

The implemented designated type may be a primitive, concrete class,
interface, `Obj`, inline array, or supported primitive/exact-class inline
optional container. `unit`, shared-owner, and function alias parameter types
are unsupported. The container borrows `ref value: T?` and
`mut ref value: T?` designate an always-present optional wrapper, not an
optional reference. A shared-backed source is explicitly dereferenced, as in
`inspect(*owner)`, and borrows the allocated class/interface/`Obj` pointee
rather than treating `shared T` as the alias's designated type. `Obj` is a
universal non-owning target with no members or inline storage. Interfaces
expose only their declared requirements. Alias modifiers are not accepted on
locals, fields, results, elements, statics, or captures.

## Eligible argument places

An object alias argument must designate an existing, already-live object place
or a forwarded interface/`Obj` view. Its root may be:

- an owning exact-class local;
- an owning exact-class value parameter;
- a live method or destructor `self`;
- an existing `ref` or `mut ref` parameter being forwarded.
- a dereferenced stable shared local or value parameter;
- a dereferenced shared field or nested shared place, through a hidden copied
  owner; or
- a dereferenced produced shared owner, through hidden storage that adopts the
  result.

Any number of exact-class field projections may follow a supported root.
Grouping around a root or projection preserves the same place. This includes
inline subobjects reached through owning values, receivers, and existing
aliases.

For an inline optional parameter, the argument may likewise be an exact
optional local, value parameter, forwarded optional alias, or optional field
reached through a supported mutable/read-only object path. `none`, an ordinary
payload, a produced optional result, and an unwrapped payload are values rather
than existing optional-container places and cannot bind the alias.

A concrete class source may convert to the same class, any ancestor class, or
`Obj`. These conversions retain the original complete object and do not slice.
An `Obj` view may be forwarded only to `Obj`; there is no implicit downcast.
Unrelated classes are invalid.

A primitive alias argument may designate an existing primitive local, value
parameter, forwarded primitive alias parameter, or primitive static field.
Static selection evaluates no receiver. Primitive fields and produced scalar
values are not yet primitive alias sources.

A fresh inline construction, inline object-returning call, and any other
produced inline value is not an object alias source. A
dereferenced produced shared allocation or shared-returning call is eligible
because its owner is adopted into call-scoped anchor storage. A raw shared
handle is an owning value rather than an alias place and is rejected here.
Initializer `self` is also ineligible while the enclosing object is incomplete;
an already initialized direct field may be passed independently when its
initializer-body rules permit.

Alias binding does not copy the object or begin a new object lifetime. The
callee operates on the same place selected by the caller.

## Access propagation

Each root supplies one access capability for its complete projection path:

| Root | Access |
|---|---|
| Owning local or owning value parameter | Mutable |
| `self` | The current member body's receiver access |
| `ref` parameter | Read-only |
| `mut ref` parameter | Mutable |

Projecting an inline class field or direct base preserves that access. A view
conversion may restrict mutable access when forwarding to `ref`; read-only
access cannot satisfy a `mut ref` parameter.

Through read-only access, code may read primitive fields, call read-only
methods, use the object as a copy source, and forward the place to another
`ref` parameter. It cannot write a field, call a mutable method, or forward the
place as `mut ref`.

Through mutable access, code may additionally write primitive fields, call
mutable methods, and forward the place as either alias mode. A mutable alias
still cannot be rebound or used as a whole-object replacement destination.
That prohibition applies to the alias root and to every class subobject
projected from it. Supported field mutation and method calls do not rebind the
alias or end the object's lifetime.

For an optional-container alias, read-only access permits presence tests,
copying the optional into an owning boundary, and checked payload consumers.
Mutable access additionally permits whole-container assignment from `none`, a
compatible payload, or an exact optional source. Replacing a class optional
still performs the dynamic presence-guard check before changing its payload.

## Forwarding, copying, and calls

Forwarding passes the same complete object into a nested call. A `ref`
parameter may be forwarded only as `ref`; a `mut ref` parameter may be
forwarded as either mode. Class-to-ancestor, class-to-conforming-interface,
and class/interface-to-`Obj` conversions change only the static view target.
An interface may forward to the same interface but does not implicitly
cross-cast to another interface. Grouping does not change these rules.

An alias name is not an ordinary scalar value. It cannot itself be copied,
stored, assigned, or returned. The object it designates may still be copied in
a supported object-copy context: for example, it may initialize an owning
local or value parameter or supply an exact-class return copy. The new object
then has its own lifetime; the alias remains non-owning.

Calls retain ordinary evaluation order. A method receiver is selected first,
then explicit arguments are evaluated from left to right. Alias place
selection participates at its source position alongside value and object-copy
arguments. It does not create a separately observable value or permit argument
reordering.

Static polymorphic views are explicit and checked through MIR as
source/target/access conversions. Exact-class, ancestor-class, and `Obj`
aliases execute through the x86-64 internal calling convention. Interface
views use the same verified non-owning representation and execute through
backend-owned class witness entries.

## Non-exclusivity

Aliases are deliberately non-exclusive. Multiple read-only or mutable alias
arguments may designate overlapping places, including the same complete
object:

```ska
fn touch(mut ref left: Item, mut ref right: Item) -> unit {
    left.add(1);
    right.add(1);
}

touch(item, item);
```

No identity or overlap check is inserted. Operations execute in ordinary
source order. Read-only access therefore restricts only operations performed
through that binding; it does not promise that another alias cannot mutate the
same object during the call.

## Lifetime and non-escape

An alias parameter is valid only for the dynamic execution of its call. The
caller retains ownership of every directly supplied inline place for that
complete call. Forwarding relies on the enclosing call's same guarantee.

The alias owns no cleanup registration and is never destroyed. Ending the
callee's parameter scope does not affect the referenced object's lifetime.
Conversely, source syntax provides no way to retain the alias after the call:
aliases cannot be fields, results, local values, elements, statics, captures,
or heap contents, and the binding cannot be assigned or rebound.

Stable inline places, forwarded aliases, and stable shared locals or value
parameters need no new owner. A replaceable shared field or nested place is
copied into hidden owner storage at its receiver or argument evaluation
position. A produced shared owner is adopted there. The owner remains live
through the call, including later argument effects, and is released with other
full-expression temporaries in reverse completion order.

The complete allocation owner anchors every inline base and field subobject in
its payload. This is containment, not object-graph search: following another
shared field establishes another anchor for that owning edge.

An explicit `(T) source` passed to an alias parameter first creates its bounded
checked-view carrier. When the source is shared-backed, the same stable,
copied-field, or adopted-produced owner classification covers that carrier
through the call. The checked view ends before any hidden owner is released.

## Shared ownership boundary

Shared ownership, heap allocation, and call-scoped shared borrowing are
implemented. The focused
[shared-ownership authority](SHARED_OWNERSHIP.md) defines non-null owning
handles, copy/adopt/release value semantics, dynamic last-owner destruction,
strong-cycle leaks, and shared-backed borrowing.

That design extends eligible alias arguments to objects reached through shared
ownership. An existing shared local or value parameter remains live through an
ordinary call. A produced shared temporary has its lifetime extended, while a
replaceable shared field or nested place is copied into a hidden owning anchor
at its receiver or argument evaluation position. The complete allocation owner
also anchors inline subobjects within its payload. Anchors remain live through
the call and are then released in ordinary reverse temporary order.

An alias still cannot be cast into ownership of its source object. The frozen
`new T(copy alias)` form may instead use the alias as a target-directed checked
copy source, invoke exact `T` copy construction, and return ownership of a
distinct allocation before the enclosing alias lifetime ends.

Local alias declarations are also an open design area. Their syntax, eligible
sources, lexical lifetime, initialization, control-flow joins, interaction
with relocation, and any anchoring requirement are not frozen. The current
parameter restrictions do not implicitly specify that larger feature.

Optional owning values, including `shared? T`, and aliases to supported inline
optional containers execute. Their
[contract](OPTIONAL_VALUES.md#aliases)
bounds a checked `value!` payload view to one complete immediate consumer
under a dynamic presence guard. Read-only aliases may inspect and unwrap;
mutable aliases may additionally set, clear, or replace an unguarded
container. This does not introduce `ref?`, aliases to `shared? T`, stored
payload aliases, or optional reference values. The implemented
[array contract](ARRAYS.md#aliases-mutation-and-backing-anchors) admits whole
array places and exact-class or nested-array elements as call-scoped alias
sources and uses hidden backing anchors across replacement. Implemented
polymorphic alias conversions and checked casts are defined by
[polymorphism](POLYMORPHISM.md) and
[object casts](OBJECT_CASTS.md). Checked places exist only for one consuming
full expression; they do not introduce local aliases.

## Implementation boundary

The language requires an alias call to preserve place identity, access,
evaluation order, and source lifetime. It does not require a raw-pointer type,
object address to be source-visible, a particular parameter representation,
frame home, register class, field offset, or calling convention.

The current target realization is an implementation concern recorded in the
[backend and target contract](../compiler/BACKEND.md). Allocation, reference
counting, anchoring, and ownership-runtime mechanisms
are specified in the
[shared-ownership implementation contract](../compiler/SHARED_OWNERSHIP.md).
Anchors compile to ordinary retain/adopt/release operations and require no
additional C runtime entry point.
