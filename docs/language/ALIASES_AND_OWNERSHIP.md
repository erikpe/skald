# Skald Aliases and Ownership

Status: authoritative for executable primitive, class, `Obj`, inline
optional-container, and array aliases. Interface views
follow the same source rules and lower through verified MIR; their backend
execution boundary is owned by [polymorphism](POLYMORPHISM.md). Shared-backed
call borrows and their hidden owner anchors are implemented as specified by
[Shared Ownership and Heap Allocation](SHARED_OWNERSHIP.md). Local aliases and
aliases into other future value families remain unfrozen. The produced
exact-class read-only alias extension is implemented through type checking,
HIR, verified MIR, and native x86-64 execution. Array-specific descriptor and
detached-backing behavior belongs to [Arrays](ARRAYS.md). Feature maturity is
authoritative in the [status matrix](STATUS.md). Produced primitive read-only
alias materialization is frozen below as a prerequisite for operator
protocols, but is not implemented.

The [grammar](GRAMMAR.md#compilation-unit-and-declarations) defines accepted
parameter syntax, [functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md)
defines call and argument order, and
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#object-places-and-projections)
defines object places, copying, and owning-object lifetime.

## Binding modes

An implemented alias parameter is a non-owning name for an eligible place.
Its binding mode is separate from the static target. Existing-place sources
select storage directly; an accepted produced-object source first materializes
an owning place in hidden caller storage and then applies the same non-owning
binding model:

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
interface, `Obj`, inline array, shared owner, optional shared owner, or a
supported inline optional container. `unit` and function alias parameter types
are unsupported. A shared-owner alias designates the exact owner storage slot,
not its pointee; replacing it through `mut ref` therefore performs ordinary
release/adoption. The container borrows `ref value: T?` and
`mut ref value: T?` designate an always-present optional wrapper, not an
optional reference. A shared-backed source is explicitly dereferenced, as in
`inspect(*owner)`, and borrows the allocated class/interface/`Obj` pointee
rather than treating `shared T` as the alias's designated type. `Obj` is a
universal non-owning target with no members or inline storage. Interfaces
expose only their declared requirements. Alias modifiers are not accepted on
locals, fields, results, elements, statics, or captures.

## Implemented eligible argument sources

An existing object alias source designates an already-live object place or a
forwarded interface/`Obj` view. Its root may be:

- an owning exact-class local;
- an owning exact-class value parameter;
- a live method or destructor `self`;
- an existing `ref` or `mut ref` parameter being forwarded;
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
optional local, value parameter, forwarded optional alias, optional field
reached through a supported mutable/read-only object path, or a compatible
inline-optional static field. Static selection evaluates no receiver. `none`,
an ordinary payload, a produced optional result, and an unwrapped payload are
values rather than existing optional-container places and cannot bind the
alias.

A concrete class source may convert to the same class, any ancestor class, or
`Obj`. These conversions retain the original complete object and do not slice.
An `Obj` view may be forwarded only to `Obj`; there is no implicit downcast.
Unrelated classes are invalid.

A primitive alias argument may designate an existing primitive local, value
parameter, forwarded primitive alias parameter, or primitive static field.
Static selection evaluates no receiver. Primitive fields and produced scalar
values are not yet primitive alias sources.

A fresh inline construction, exact-class object-returning call, canonical
class literal, or supported cast composition is now an object alias source for
read-only `ref` during type checking and HIR construction. The
[produced-object rule](#implemented-produced-read-only-alias-arguments) changes only
this exact-class restriction; `mut ref` remains place-based. A
dereferenced produced shared allocation or shared-returning call is eligible
because its owner is adopted into call-scoped anchor storage. A raw shared
handle is an owning value rather than an alias place and is rejected here.
Initializer `self` is also ineligible while the enclosing object is incomplete;
an already initialized direct field may be passed independently when its
initializer-body rules permit.

Alias binding does not copy the object or begin a new object lifetime. The
callee operates on the same place selected by the caller.

The implemented
[produced-object field-read contract](FUNCTIONS_AND_CONTROL_FLOW.md#produced-object-field-reads)
allows an exact-class field projected from one accepted produced root to
serve as a read-only `ref` source. The hidden complete root, rather than the
projected field, owns cleanup and remains live through the call; the projected
subobject preserves its static field class and the root's complete-object
origin. This creates no local or stored alias and does not make the same path
eligible for `mut ref`. Primitive fields, optional-container fields, and array
fields retain their existing alias-place restrictions, while inline array
fields can form bounded read-only whole-array aliases and shared fields still
require explicit dereference plus any required owner anchor.

## Implemented produced read-only alias arguments

An ordinary expression that produces one complete inline object of a known
exact class may bind directly to a compatible read-only `ref` parameter. Type
checking, HIR, MIR lowering and verification, and x86-64 execution implement
its source classification, compatibility, access, produced-view
representation, hidden temporary lifetime, and ordinary internal alias ABI.
Accepted producers are:

- a fresh exact-class construction;
- an exact-class result from an internal direct, static, instance-method, or
  interface call;
- a canonical class-valued literal such as a `Str` literal;
- any grouping of an accepted producer; and
- a supported `(T) source` checked cast whose selected object is backed by an
  accepted produced exact-class value.

The rule applies uniformly to internal functions, static and instance
methods, interface calls, and ordinary initializer overloads. It creates no
standard-library exception. Syntax and resolution continue to treat the
source as an ordinary argument expression; no explicit reference expression
or new grammar form is introduced.

The producer's exact dynamic class determines compatibility. The temporary
may be viewed as that class, an ancestor, any interface implemented by that
class, or `Obj`. These are non-owning views of the same complete object: they
do not slice, copy, reconstruct metadata, or change dynamic identity.
Unrelated targets, implicit downcasts, and unsupported interface conversions
remain invalid. An explicit checked cast performs its ordinary static or
runtime selection first. Any bounded checked-view carrier remains subordinate
to the owning producer temporary and ends before that temporary is destroyed.

Only read-only `ref` receives this relaxation. A `mut ref` argument continues
to require an existing mutable source place, even though the compiler's hidden
temporary storage is physically writable during initialization. This keeps
implicit temporary binding observational. Any future facility for mutating an
unnamed object would require its own source syntax and contract.

The implemented relaxation does not apply to produced primitives, optional
containers, arrays, raw shared handles, or implicit shared dereference. Existing
inline-optional and array alias rules remain place-based. Shared-backed
borrowing continues to require explicit `*` or `->` selection and follows its
existing stable-owner or hidden-anchor rules. The extension also creates no
local alias, stored reference, alias result, capture, external alias
signature, or independently storable reference type.

This compact example covers a canonical class literal, fresh construction,
and object-returning call without staging locals:

```ska
class Item {
    value: i64;
    init(value: i64) { self.value = value; }
}

fn make() -> Item { return Item(2); }
fn inspect(ref value: Obj) -> unit {}
fn mutate(mut ref value: Item) -> unit {}

fn example() -> unit {
    inspect("literal");
    inspect(Item(1));
    inspect(make());

    // error[TYP020]: mutable alias argument requires an existing object place
    mutate(Item(3));
}
```

The first three calls create one caller-owned temporary apiece. The final call
is rejected because implicit produced binding is read-only; assigning the
object to a mutable local first is the supported spelling when mutation is
intended.

### Production, ownership, and lifetime

The caller evaluates an accepted producer exactly once at its ordinary
left-to-right argument position. A method receiver is selected first. The
complete object is initialized directly in one hidden caller-owned exact-class
temporary before evaluation proceeds to later arguments. The temporary
becomes live and acquires cleanup responsibility only after production
successfully completes; a failed producer or checked conversion does not
enter the call.

The same temporary remains live through later argument effects, the complete
dynamic call, and any nested forwarding performed by the callee. The alias is
valid only during that call, while its owner follows the enclosing
full-expression lifetime and is destroyed in reverse completion order with
other owning temporaries after the call result has been secured. A selected
short-circuit path owns only the producers it actually evaluates.

Binding performs no copy construction and transfers no ownership to the
callee. The parameter owns no cleanup and cannot destroy, retain, rebind,
store, or return its alias. If the callee copies the designated object into an
owning destination or result, that distinct copy must complete before the
temporary is destroyed and then follows its own lifetime.

### Diagnostics

Diagnostics distinguish source-category failure from type incompatibility:

- an exact-class producer that cannot supply the requested class, interface,
  or `Obj` view is a type mismatch; its diagnostic identifies the producer
  and retains parameter-type declaration context;
- an otherwise compatible producer passed to `mut ref` is an invalid mutable
  alias source whose diagnostic requires an existing mutable place, rather
  than describing the producer as a read-only place; and
- excluded primitive, optional, array, and shared-owner families retain their
  family-specific alias or explicit-dereference diagnostics.

Call checking continues through the complete argument list so independent
errors retain ordinary reporting and source order. The typed representation
uses one produced read-only view. Verified MIR constructs its owner once,
keeps it live for the complete call, and destroys it once at the enclosing
full-expression boundary.

## Frozen produced primitive read-only alias arguments

The frozen
[operator-protocol contract](OPERATOR_OVERLOADING.md#evaluation-aliases-and-cleanup)
requires any successfully checked primitive value expression to bind to a
compatible read-only primitive `ref` parameter. A literal, call result,
primitive field read, cast, or compound primitive expression is evaluated once
at its ordinary argument position and materialized in hidden caller-owned
scalar storage. That storage remains live through later argument effects and
the complete call and ends at the enclosing full-expression boundary after
the result is secured.

An existing compatible primitive place continues to borrow directly. `mut
ref` remains restricted to an existing mutable place. Materialization creates
no source reference value, alias local, escaping or stored alias, external
alias signature, implicit conversion, or observable permission to mutate
unnamed storage.

This extension is frozen but not implemented. The implemented primitive-source
list above remains authoritative until the status matrix promotes the feature.

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

For a produced object alias, the compiler-created temporary
supplies read-only alias access regardless of the temporary's internal
initialization capability. It therefore forwards only to another `ref`.

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

[Structural bracket calls](INDEXING_AND_SLICING.md) use this same access
propagation. Index and slice getters are read-only method calls; setters are
mutable method calls. Their key, bounds, replacement, result, produced
receiver temporary, shared-owner anchor, and cleanup follow ordinary call and
alias rules after resolution. Bracket spelling grants no additional mutable
access and creates no storable alias or borrowed-result category.

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

For a produced object alias, hidden caller storage owns the materialized place
through the enclosing full expression. The alias
still cannot outlive its dynamic call; the slightly longer owner lifetime
only supplies deterministic cleanup and does not create an escaping view.

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

Optional owning values, including `(shared T)?`, and aliases to supported inline
optional containers execute. Exact `(shared T)?` owner slots also bind directly
to `ref` and `mut ref` parameters and retain their ordinary conditional owner
lifecycle. Their
[contract](OPTIONAL_VALUES.md#aliases)
bounds a checked `value!` payload view to one complete immediate consumer
under a dynamic presence guard. Read-only aliases may inspect and unwrap;
mutable aliases may additionally set, clear, or replace an unguarded
container. This does not introduce `ref?`, produced-owner aliases, stored
payload aliases, or optional reference values. The implemented
[array contract](ARRAYS.md#aliases-mutation-and-backing-anchors) admits whole
array places and exact-class or nested-array elements as call-scoped alias
sources and uses hidden backing anchors across replacement. Implemented
polymorphic alias conversions and checked casts are defined by
[polymorphism](POLYMORPHISM.md) and
[object casts](OBJECT_CASTS.md). Checked places exist only for one consuming
full expression; they do not introduce local aliases.

The implemented
[compositional optional profile](OPTIONAL_VALUES.md#compositional-optional-types)
admits aliases whose designated container is a supported inline optional,
including a nested optional or optional array. Optional shared-owner slots are
also eligible as their exact canonical owning type; they are not treated as
inline wrappers or implicitly dereferenced. An alias to an optional container borrows the
always-present wrapper. Passing `value!` from
an optional array to `ref T[]` or `mut ref T[]` instead creates a checked
call-scoped payload view: a presence guard pins the wrapper and a backing
anchor covers the immediate call. It does not add `ref?`, stored references,
or escaping payload views.

## Implementation boundary

The language requires an alias call to preserve place identity, access,
evaluation order, and source lifetime. It does not require a raw-pointer type,
object address to be source-visible, a particular parameter representation,
frame home, register class, field offset, or calling convention.

Produced object aliases change only how the caller establishes the aliased
place. They add no external object or alias ABI, change no internal alias
calling convention, and require no runtime service. The backend receives the
same verified non-owning alias representation used for an existing place.

The current target realization is an implementation concern recorded in the
[backend and target contract](../compiler/BACKEND.md). Allocation, reference
counting, anchoring, and ownership-runtime mechanisms
are specified in the
[shared-ownership implementation contract](../compiler/SHARED_OWNERSHIP.md).
Anchors compile to ordinary retain/adopt/release operations and require no
additional C runtime entry point.
