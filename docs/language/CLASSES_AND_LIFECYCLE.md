# Skald Classes and Lifecycle

Status: authoritative for implemented exact-class declarations, inline
containment, receiver access, ordinary initialization, and object-place
semantics. Copying, assignment, destruction, and object materialization remain
authoritative in the legacy
[exact-class object-value section](../SKALD_DRAFT_SPEC.md#546-frozen-exact-class-object-value-profile)
until those rules are migrated into this document.

The [status matrix](STATUS.md) defines feature maturity, the
[grammar](GRAMMAR.md#class-declarations) defines accepted source shape,
[types and values](TYPES_AND_VALUES.md#values-and-places) defines the
value/place distinction, and
[functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md) defines general
call and evaluation-order rules.

## Exact nominal classes

A class declaration introduces one nominal type. Two classes are different
types even when their declarations have identical members. The current model
has no inheritance or implicit class conversion, so every class use requires
the exact declared class.

A class value is one complete inline object containing all of its direct
fields. A class-typed field is a complete inline subobject of its containing
object, not a pointer, nullable handle, alias, or separately allocated object.
Two fields of the same class contain two distinct subobjects.

Classes may be declared in any top-level order. A field, parameter, or result
may name a class declared later in the same source file.

## Members and namespaces

The implemented class member categories are:

| Category | Contract |
|---|---|
| Field | Named inline storage with a primitive or exact-class type. |
| Ordinary initializer | The one required operation that establishes a new complete object. |
| Method | A statically selected instance operation with a read-only or mutable receiver. |
| Copy constructor, copy assignment, destructor | Optional or synthesized lifecycle operations whose detailed semantics are defined separately. |

Top-level classes, functions, and external functions share one non-overloaded
namespace. Within each class, fields and ordinary methods share one
non-overloaded member namespace. A field and method in the same class cannot
have the same name, and methods cannot be overloaded. The same member name may
be declared independently by unrelated classes.

Lifecycle declarations occupy dedicated class-owned slots rather than the
ordinary member namespace. `init`, `assign`, and `destroy` are contextual in
those declaration shapes and remain valid ordinary field, method, parameter,
local, and top-level function names elsewhere.

Every class has exactly one explicit ordinary initializer, including an empty
class. An initializer with exactly one read-only alias parameter of the
enclosing class is classified as the separate copy-constructor slot; it does
not satisfy the ordinary-initializer requirement. Any other valid `init`
signature is ordinary, and a second ordinary initializer is rejected rather
than forming an overload set.

All current fields and methods are accessible wherever the receiver is
available. Static members, access modifiers, and inherited member lookup are
not implemented.

## Fields and finite containment

A field has one stored type: `i64`, `u64`, `u8`, `f64`, `bool`, or an exact
class. `unit` has no payload and cannot be a field type. A named field type must
resolve to a class rather than a function or another declaration kind.

Inline containment must be finite. The directed relation from each class to
the classes of its direct fields must be acyclic. Direct self-containment and
indirect cycles are invalid. Forward references, repeated fields of the same
class, acyclic diamonds, and empty contained classes are valid.

Field declaration order is source-visible where initialization and lifecycle
rules refer to direct fields, but it does not require a particular physical
layout. Storage size, alignment, offsets, padding, and the representation of an
empty object are target concerns outside the language contract.

## Receivers and access

`self` denotes the current complete-object place within a class-owned instance
body. It is a keyword and cannot be shadowed. It is unavailable in top-level
function bodies.

An ordinary `fn` method has read-only receiver access. It may read through
`self` and call other read-only methods. A `mut fn` method has mutable receiver
access and may additionally write primitive fields, replace objects in
supported assignment contexts, and call mutable methods.

Receiver requirements are checked at the call site. A mutable method requires
a mutable receiver place; a read-only method accepts either access level. Calls
are selected statically from the exact receiver class. There is no virtual or
interface dispatch.

The root binding determines access for an entire inline path:

- an owning local, owning value parameter, or `mut ref` parameter is mutable;
- `self` has the access declared by its current member body;
- a `ref` parameter is read-only.

Every class-field projection preserves that access. Projection does not create
a new const-qualified type or perform a runtime conversion. Detailed alias
forwarding and overlap rules remain in the legacy
[alias-binding section](../SKALD_DRAFT_SPEC.md#45-alias-binding-modes) until the
focused ownership document replaces it.

## Object places and projections

An object place identifies existing class storage. It consists of a root and
zero or more class-field projections. Implemented live roots are owning locals,
owning value parameters, `self`, and `ref` or `mut ref` parameters. Grouping
around a root or projection preserves the same place.

Each intermediate projection must select a class-typed field. A class-typed
endpoint remains an object place and may be used in a supported copy,
assignment, receiver, or alias context. It is not an ordinary scalar value.
Selecting a final primitive field reads or writes that field according to the
surrounding expression or statement and the root's access.

For example, in `root.branch.leaf.value`, `root` is the root place,
`branch` and `leaf` select complete inline subobjects, and `value` selects the
final primitive field. In `root.branch.leaf.read()`, the `leaf` endpoint is the
method receiver. In `inspect(root.branch.leaf)`, it may be an alias argument
when the parameter expects that exact class.

Projection is valid only through fields owned by the class at that point in
the path. A primitive field cannot be projected further, a method is not a
field place, and member selection does not search unrelated classes.

## Ordinary initializer contract

An ordinary initializer has an implicit mutable `self`, an implicit `unit`
result, and the implemented internal parameter categories described in
[functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#parameters). Its
`self` storage exists while the body runs but is not yet a live complete
object.

The body is a straight-line sequence of direct assignments to fields of that
`self`. It cannot contain locals, nested blocks, conditionals, call statements,
explicit returns, or assignment through another root or a deeper destination.
Grouping around `self` does not change the direct destination.

Every direct field must be initialized exactly once. Fields may be initialized
in any source order. A field becomes initialized only after its complete,
type-correct initialization succeeds; an invalid right side or invalid
constructor argument does not advance its state.

Primitive fields use an expression of the exact field type:

```ska
class Counter {
    value: i64;

    init(initial: i64) {
        self.value = initial;
    }
}
```

A class field uses an ungrouped construction of its exact field class directly
in that subobject's storage:

```ska
class Leaf {
    value: i64;

    init(value: i64) {
        self.value = value;
    }
}

class Root {
    leaf: Leaf;
    total: i64;

    init(value: i64) {
        self.leaf = Leaf(value);
        self.total = self.leaf.value;
    }
}
```

The class field is not live while constructor arguments or its nested
initializer execute. It becomes a complete subobject only after that
initializer returns normally. Later statements in the enclosing initializer
may then read or mutate it according to access, call its methods, project
through its completed subobjects, or pass those places as alias arguments.

Reading an uninitialized direct field, projecting through it, using it as a
receiver or alias source, initializing a field twice, or completing the body
with a missing field is invalid. The incomplete enclosing `self` cannot be
used as a complete method receiver, alias source, copy source, or ordinary
value. The complete object becomes live only after every direct field has been
initialized and the ordinary initializer returns normally.

## Fresh construction

`Class(arguments)` selects that class's ordinary initializer. The arguments
must match its ordered parameters exactly. Destination storage is selected
before argument evaluation, arguments are evaluated from left to right, and
the initializer begins only after every argument is ready. The destination
becomes live only on normal initializer completion.

A fresh object may directly initialize an exact-class local or a direct class
field as described above. Current object-source contexts also permit fresh
objects in selected calls, assignments, and returns; their copy,
materialization, cleanup, and elision behavior belongs to the lifecycle
contract that will be consolidated separately.

Skald has no recoverable construction failure. Exceptional initialization,
partially completed object cleanup, delegation between ordinary initializers,
and construction of base subobjects are not defined by the implemented model.

## Unsupported extensions

The implemented class model does not include inheritance, base members,
interfaces, virtual dispatch, `Obj`, class conversions, shared or heap-backed
objects, `new`, nullable object references, static members, access modifiers,
`final`, abstract members, overloads, reflection, or user-defined conversions.
Their maturity is recorded in the [status matrix](STATUS.md#not-implemented),
and polymorphism decisions remain owned by the active
[polymorphism roadmap](../roadmaps/POLYMORPHISM_ROADMAP.md).

This document specifies source-visible class and initialization behavior. It
does not prescribe compiler identities, phase data structures, containment
algorithms, object offsets, hidden receiver placement, calling conventions,
frame storage, or backend address calculation.
