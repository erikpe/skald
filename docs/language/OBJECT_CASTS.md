# Object Casts

Status: **plain checked-place casts implemented; shared-owner casts planned**.
This document is authoritative for C-style cast syntax and the complete
conversion matrix between inline class places, non-owning aliases, and shared
owners. The current compiler implements plain checked-place casts for method
receivers, alias arguments, field access, exact-class copy construction and
assignment, value parameters, results, and owning slicing. Produced inline
sources use owning full-expression temporaries. Shared-owner casts remain
planned. Scoped `narrow` also remains implemented during the staged migration
owned by the [object-casts roadmap](../roadmaps/OBJECT_CASTS_ROADMAP.md).

Primitive conversions, optional casts, unsafe reinterpretation, user-defined
conversions, and external object ABI are outside this profile.

## Source forms and precedence

The source forms are:

```text
object-cast-expression = "(" object-cast-target ")" unary-expression
object-cast-target     = view-target | "shared" view-target
view-target            = identifier | "Obj"
```

Examples:

```ska
((Leaf) value).read()
(Leaf) value
(shared Leaf) shared_value
```

An object cast has unary precedence. Postfix member selection and calls on the
result therefore use grouping as in `((Leaf) value).read()`. The source is
evaluated exactly once before any dynamic check or result ownership operation.

`shared` is contextual in a cast target. A parenthesized identifier followed by
an expression is retained as a cast candidate until the identifier is resolved
in the type namespace. If it names a class or interface, or is `Obj`, the form
is a cast. Otherwise it is diagnosed as an invalid cast target rather than
reinterpreted using value lookup. This deliberately gives cast syntax priority
over a grouped callable spelling such as `(f)(argument)`; direct calls remain
`f(argument)`. It also leaves `(value) - other` as grouped subtraction because
the token after the closing parenthesis is a binary operator, not an adjacent
cast operand.

The [implemented grammar](GRAMMAR.md) is authoritative for current acceptance.
`(shared T) source` is retained syntactically but currently produces a focused
unsupported-feature diagnostic before shared semantic types are introduced.

## Two cast results

A plain object cast and a shared cast have different ownership results.

### Checked place cast

`(T) source` selects a checked, non-owning object place with static target `T`.
It does not itself allocate, retain, copy, slice, or create a source-visible
alias binding.

The selected place preserves:

- the source complete-object identity;
- the source dynamic-class metadata;
- the unique target class subobject, when `T` is a class;
- the target interface view, when `T` is an interface; and
- no more access than the source provides.

The place is a bounded expression result, not a first-class reference value.
Its consuming context may use it as a receiver, an alias argument, a field
source, or the source for an owning inline operation. It cannot be stored as an
alias, returned as an alias, captured, or used to replace a whole object through
an alias-rooted path.

### Shared-owner cast

`(shared T) source` is valid only when `source` already has a shared type. It
dynamically selects the requested class/interface/`Obj` view while preserving
the same allocation and complete dynamic object.

A named source is copied and therefore creates one additional strong owner. A
produced source transfers its existing owner. The result never allocates,
copies object payload, slices, or changes dynamic metadata.

Neither an inline object nor a `ref`/`mut ref` alias may be cast to `shared T`.
An alias has no authority to manufacture ownership, and converting an inline
object would require a new allocation rather than an identity-preserving cast.

## Complete ownership matrix

In this table, `T` may be a class, interface, or `Obj`. An inline destination
exists only when `T` is a class.

| Source category | `(T) source` | Inline destination from `(T) source` | `(shared T) source` |
|---|---|---|---|
| Existing inline class place | Checked borrowed view; source access preserved | Copy-construct or assign exact `T`; may slice | Invalid |
| Produced inline class object | Checked view backed by its full-expression temporary | Copy-construct or assign exact `T`; may slice | Invalid |
| `ref S` alias | Checked read-only borrowed view | Copy-construct or assign exact `T`; may slice | Invalid |
| `mut ref S` alias | Checked mutable borrowed view | Copy-construct or assign exact `T`; may slice | Invalid |
| Existing `shared S` owner | Checked borrowed view, anchored when its place is replaceable | Copy-construct or assign exact `T`; may slice | Copy one owner of the same allocation |
| Produced `shared S` owner | Checked view backed by the produced owner | Copy-construct or assign exact `T`; may slice | Transfer the produced owner of the same allocation |

The conceptual directions are:

```text
inline  --borrow--> checked place
inline  --copy----> inline
inline  ----X-----> shared

alias   --borrow--> checked place
alias   --copy----> inline
alias   ----X-----> shared

shared  --borrow--> checked place
shared  --copy----> inline
shared  --owner---> shared
```

The `X` entries mean that no **cast** performs that direction. The explicit
copy-allocation form `new T((T) source)` may consume an inline, alias, or
shared-backed checked place and create a distinct allocation, as described
below. It is construction rather than a conversion to the original object's
ownership.

The shared-to-inline direction is not one compound storage conversion. The cast
first selects a borrowed target-class place; the inline destination then uses
the existing exact-class copy construction or assignment operation:

```ska
var inline_leaf: Leaf = (Leaf) shared_object;
```

`inline_leaf` has independent identity and lifetime. If the allocation's
dynamic class is derived from `Leaf`, the operation copies the selected `Leaf`
subobject into an exact `Leaf` and therefore slices. Shared fields within that
copy follow ordinary shared copying and retain their allocations.

## Allocation and copy construction

Casts never allocate. The authoritative
[shared construction contract](SHARED_OWNERSHIP.md#type-and-construction-forms)
instead provides two forms headed by `new` that create an allocation:

```ska
new ConcreteClass(arguments)
new ConcreteClass((ConcreteClass) source)
```

The first performs ordinary initialization. In the second, the explicit
matching `(ConcreteClass) source` supplies the checked place from which the
new exact-class allocation is copy-constructed. The matching cast is required
even for a same-type source:

```ska
var dog: shared Dog = new Dog((Dog) source_dog);
```

The cast source evaluates once and is anchored before its check. A failed
dynamic check terminates before the enclosing copy allocation allocates its
destination. On success, the checked place and anchor remain live through
selected copy construction and until the produced owner is secured. The
allocation and copy are effects of `new`, not of the cast.

The class named by `new`, not the source's complete dynamic class, determines
the new allocation's dynamic class. Copying through an ancestor cast therefore
slices deliberately:

```ska
fn copy_as_animal(ref dog: Dog) -> shared Animal {
    return new Animal((Animal) dog);
}

fn checked_copy_as_dog(ref animal: Animal) -> shared Dog {
    return new Dog((Dog) animal);
}

fn copy_inline_as_animal(dog: Dog) -> shared Animal {
    return new Animal((Animal) dog);
}
```

The first and third allocations have exact dynamic class `Animal`. The second
checks that the source supplies a `Dog` view, then creates an exact `Dog`.
To preserve a statically known `Dog` while returning `shared Animal`, allocate
and copy the `Dog`, then use the ordinary shared upcast:

```ska
return new Dog((Dog) dog);
```

This does not provide dynamic cloning. An operation that discovers and
preserves an arbitrary source dynamic class is deferred; a future `clone()`
method convention or dedicated syntax requires a separate design owned by
[shared ownership](SHARED_OWNERSHIP.md#deferred-dynamic-cloning).

Every other shared-producing operation works with an allocation that already
exists:

- reading a named shared place copies an owner;
- accepting or returning a produced shared value transfers its owner;
- shared assignment secures an incoming owner before releasing the old owner;
- `(shared T) source` checks and preserves the existing allocation;
- an implicit shared upcast preserves the existing allocation; and
- a hidden anchor temporarily copies or adopts an owner.

None of these operations allocates or copies the complete object payload.

Consequently, these are invalid:

```ska
var from_inline: shared Leaf = (shared Leaf) inline_leaf;
var from_alias: shared Leaf = (shared Leaf) borrowed_leaf;
```

The corresponding explicit copies are construction:

```ska
var from_inline: shared Leaf = new Leaf((Leaf) inline_leaf);
var from_alias: shared Leaf = new Leaf((Leaf) borrowed_leaf);
```

## Same-type, up-, down-, and cross-casts

Cast legality and runtime classification use the same complete dynamic-class
relation as `is`:

| Relationship | Cast behavior |
|---|---|
| Same target | Static success; no runtime check |
| Class to ancestor, guaranteed interface, or `Obj` | Static success; no runtime check |
| Possible class downcast | Dynamic metadata check |
| Possible interface-to-class or interface-to-interface cross-cast | Dynamic metadata check |
| `Obj` to a possible class or interface | Dynamic metadata check |
| No possible dynamic class supplies the target view | Compile-time error |

The current compilation unit is a closed declared-class set for determining
whether a cast can possibly succeed. A runtime check compares the preserved
dynamic class against the classes providing the target view; it does not
inspect payload bytes or recover type information from source names.

An existing inline exact-class value has that exact dynamic class. Casting an
inline `Base` value to `Derived` is therefore statically impossible. If the
`Base` was previously produced by slicing a `Derived`, the discarded derived
identity cannot be recovered. A class/interface/`Obj` alias or shared owner may
instead preserve a more-derived complete dynamic object and support a checked
downcast.

Explicit same-type and upcasts are allowed even when an implicit view
conversion would suffice. They do not force a runtime check.

## Interfaces and `Obj`

A place cast may target a class, interface, or `Obj`. Interface and `Obj`
targets remain non-owning views and cannot initialize standalone inline
interface or `Obj` storage.

A shared-owner cast may target `shared Class`, `shared Interface`, or
`shared Obj`. All preserve the same header pointer, allocation, and metadata.
Class-to-interface and class/interface-to-`Obj` upcasts normally need no
explicit cast, but the explicit spelling is valid. Interface cross-casts and
interface/`Obj` downcasts are checked when dynamically possible.

There is no cast between unrelated complete class branches under single
inheritance. An interface or `Obj` source may still cast to a class when at
least one possible dynamic class provides both source and target views.

## Access

Place casts preserve access rather than spelling a new access mode:

- a `ref` source produces a read-only target view;
- a `mut ref` source produces a mutable target view;
- an inline place preserves the access of its root; and
- a shared owner provides the ordinary mutable access of a shared pointee.

A cast never upgrades read-only access. The result may call only methods
permitted by that access and may satisfy only a compatible alias parameter.
Read-only access remains shallow around shared fields as defined by
[shared ownership](SHARED_OWNERSHIP.md#access-and-mutation).

Multiple cast views may overlap. They retain the language's deliberate
non-exclusivity and ordinary source evaluation order.

## Consuming a place cast

The following contexts consume `(T) source` without making aliases
first-class:

| Context | Effect |
|---|---|
| Method receiver | Call on the selected target place |
| `ref`/`mut ref` argument | Pass the selected place for that call |
| Field read or supported field mutation | Operate through the selected access |
| Exact-class local or field initialization | Copy-construct exact class `T` |
| Exact-class value argument or result | Copy into the parameter or result destination |
| Whole-object assignment to an owning exact `T` destination | Run exact `T` copy assignment |
| `new T((T) source)` | In the future shared profile, allocate exact `T` and copy-construct it from the checked place |

An interface or `Obj` place cast is valid only in view-consuming contexts
because neither has standalone inline storage. A class place cast used in an
owning context follows the existing copy capability, evaluation, slicing,
temporary, and cleanup rules. No untyped aggregate copy is introduced.

The cast place is not a valid whole-object assignment destination, even when
its access is mutable, because it remains alias-rooted. Supported field
mutation and mutable method calls do not rebind the view.

## Lifetime and anchors

The checked place remains live through its complete consuming full expression:

- an existing inline owner remains live under ordinary lexical cleanup;
- a produced inline object is materialized as an owning full-expression
  temporary;
- an existing alias inherits its enclosing call lifetime;
- an existing shared local or value parameter supplies its already-live owner;
- a replaceable shared field or nested replaceable place is copied into a
  hidden anchor at source evaluation; and
- a produced shared owner remains live and acts as the anchor.

The view ends before its anchor or source temporary is cleaned. For a call,
the receiver is evaluated and anchored before explicit arguments; arguments
remain left-to-right; and the result is secured before anchors and other
temporaries are released in reverse completion order.

The initial cast profile has no local alias declarations. A place cast cannot
be stored for reuse across statements. Repeating a cast repeats source
evaluation and any required dynamic check. This is a deliberate simplification
when removing `narrow`, not permission for a compiler to extend the temporary
view beyond its full expression.

## Failure

The source is evaluated and made lifetime-safe before a required dynamic check.
Static impossibility is a compile-time error. Dynamic failure terminates the
process unsuccessfully without producing a value, returning to Skald, or
guaranteeing remaining cleanup.

There is no null result and no unchecked object cast. A future optional cast
must be designed together with explicit optional values; recoverable cast
exceptions likewise belong to the future exception design.

## Removal of `narrow`

The final cast profile contains no `narrow` statement or narrowed-alias binding.
Immediate receiver, argument, field, copying, assignment, and return uses
consume a place cast directly. Multi-statement reuse requires repeating the
cast or creating an independent inline/shared owner where the program intends
one.

Local aliases remain a separate future design. Removing `narrow` does not
implicitly introduce `ref` locals, reference values, alias assignment, or
escaping borrows.

During the cast-roadmap transition, the compiler accepts both scoped `narrow`
and the implemented plain checked-place profile. It diagnoses `shared T` as
unsupported until shared ownership is implemented. The remaining cast-roadmap
work removes `narrow` and its statement-specific compiler representation; it
does not broaden the plain cast matrix.
