# Skald Aliases and Ownership

Status: authoritative for executable class and `Obj` aliases. Interface views
follow the same source rules and are validated through HIR; their executable
boundary is owned by [polymorphism](POLYMORPHISM.md). Shared ownership, borrow anchors, local
aliases, and aliases into other future value families are not implemented or
frozen. Their maturity is authoritative in the [status matrix](STATUS.md).

The [grammar](GRAMMAR.md#compilation-unit-and-declarations) defines accepted
parameter syntax, [functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md)
defines call and argument order, and
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#object-places-and-projections)
defines object places, copying, and owning-object lifetime.

## Binding modes

An alias parameter is a non-owning name for an existing class object place or
one of its static views. Its binding mode is separate from the static target:

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

The designated type may be one concrete class, one interface, or `Obj`.
Primitive, `unit`, shared, optional, array, and function alias parameter types
are unsupported. `Obj` is a universal non-owning target with no members or
inline storage. Interfaces expose only their declared requirements. Alias
modifiers are not accepted on locals, fields, results, elements, statics, or
captures.

## Eligible argument places

An alias argument must designate an existing, already-live object place or a
forwarded interface/`Obj` view. Its root may be:

- an owning exact-class local;
- an owning exact-class value parameter;
- a live method or destructor `self`;
- an existing `ref` or `mut ref` parameter being forwarded.

Any number of exact-class field projections may follow a supported root.
Grouping around a root or projection preserves the same place. This includes
inline subobjects reached through owning values, receivers, and existing
aliases.

A concrete class source may convert to the same class, any ancestor class, or
`Obj`. These conversions retain the original complete object and do not slice.
An `Obj` view may be forwarded only to `Obj`; there is no implicit downcast.
Unrelated classes are invalid.

A fresh construction, object-returning call, primitive binding or field, and
any other produced value is not an alias source.
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
aliases execute through the x86-64 internal calling convention. They retain
the same non-owning, call-scoped, access-restricted source semantics.

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

All currently eligible places have stable inline storage for the call, and a
fresh temporary cannot be borrowed. The implemented subset therefore needs no
caller-owned borrow anchor, lifetime extension, ownership-provenance tag, or
exclusivity analysis. The current normal-flow language also has no exceptional
call exit requiring an anchor cleanup rule.

## Future ownership boundary

Shared ownership and heap allocation are an **exploratory direction**, not
usable syntax or a settled language contract. The intended broad model is a
non-null owning handle with shared lifetime and deterministic destruction of
the complete dynamic object when its last owner is released. Allocation
failure, copying and assignment effects, cycle behavior, dynamic metadata,
thread safety, and exact destruction integration remain to be frozen before
implementation.

Aliases from shared-owned or otherwise replaceable storage are unsupported.
Such a feature would need a source-visible guarantee that the containing
storage stays alive for the call even if another path replaces an owner. A
caller-owned borrow anchor is one design direction, but eligible sources,
anchor selection, lifetime extension, retain/release observability, cleanup on
all exits, and anchor coalescing are not current rules.

Local alias declarations are also an open design area. Their syntax, eligible
sources, lexical lifetime, initialization, control-flow joins, interaction
with relocation, and any anchoring requirement are not frozen. The current
parameter restrictions do not implicitly specify that larger feature.

Optional payloads and array elements are not alias sources because neither
value family is implemented. Any future design must define how a conditional
payload or element remains present and at a stable location while aliased;
the current parameter model does not settle those constraints. Frozen
polymorphic alias conversions and scoped narrowing are defined by
[polymorphism](POLYMORPHISM.md); the active roadmap owns their implementation.

## Implementation boundary

The language requires an alias call to preserve place identity, access,
evaluation order, and source lifetime. It does not require a raw-pointer type,
object address to be source-visible, a particular parameter representation,
frame home, register class, field offset, or calling convention.

The current target realization is an implementation concern recorded in the
[backend and target contract](../compiler/BACKEND.md). Future
allocation, reference counting, and ownership-runtime mechanisms are outside
the current [runtime ABI](../compiler/RUNTIME_ABI.md#responsibility-boundary)
until their language design is frozen.
