# Skald Polymorphism

Status: frozen design under active implementation. The compiler parses one
contextual `extends` clause, resolves its target to a class identity, rejects
cycles and inherited member collisions, and builds canonical ancestry and
member lookup. Every inheritance-shaped program still stops before HIR; base
lifecycle, inherited source access, interfaces, polymorphic views, type tests,
and narrowing remain unavailable. The [status matrix](STATUS.md) distinguishes
this boundary from executable exact-class behavior.

This document is the language authority for the restricted polymorphism
profile. It extends, rather than replaces:

- [classes and lifecycle](CLASSES_AND_LIFECYCLE.md) for inline values,
  initialization, copying, assignment, and destruction;
- [aliases and ownership](ALIASES_AND_OWNERSHIP.md) for non-owning access,
  forwarding, overlap, and non-escape; and
- [functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md) for calls,
  evaluation order, results, and normal cleanup.

## Frozen source profile

The following EBNF describes the complete surface owned by this profile. The
optional `extends` clause is now part of the
[implemented grammar](GRAMMAR.md); the remaining forms become accepted only
when their corresponding roadmap tasks land.

```text
class-declaration       = "class" identifier ["extends" identifier]
                          ["implements" identifier {"," identifier}]
                          "{" {class-member} "}"

interface-declaration   = "interface" identifier
                          "{" {interface-method} "}"
interface-method        = ["mut"] "fn" identifier parameter-list
                          "->" result-type ";"

polymorphic-method      = [method-modifier] ["mut"] "fn" identifier
                          parameter-list "->" result-type block
method-modifier         = "virtual" | "override"

base-initialization     = "super" "(" [argument-list] ")" ";"
type-test-expression    = additive-expression ["is" view-target]
view-target             = identifier | "Obj"

narrowing-statement     = "narrow" alias-binding "=" object-place block
alias-binding           = ["mut"] "ref" identifier ":" view-target
```

`extends`, `implements`, `interface`, `virtual`, `override`, `super`, `is`, and
`narrow` are contextual words. `Obj` is a contextually recognized type name.
All remain ordinary identifiers outside the exact forms above; the lexer does
not reserve them. A class header always places `extends` before `implements`.
Modifier order is `virtual` or `override`, then optional `mut`, then `fn`.
Neither modifier may be repeated or combined with the other.

Once the profile is enabled, `Obj` cannot be the name of a top-level
declaration because type positions must identify the universal root
unambiguously. It remains usable as an ordinary field, method, parameter, or
local name where a type is not expected.

`is` is non-associative and binds less tightly than arithmetic. Thus
`value + offset is T` groups the addition before the test, though semantic
typing normally rejects a primitive source. A type test cannot be chained.
`narrow` is a statement and its trailing block is the only scope of the new
alias binding.

## Hierarchies and declaration namespaces

A class has zero or one direct base class. Successive direct bases form a
single chain, and every accepted chain is acyclic. The base name must resolve
to a class. An interface name in `implements` must resolve to an interface.
Classes, interfaces, functions, and external functions share the existing
top-level declaration namespace, so a spelling cannot denote more than one of
those declarations in a compilation unit.

An interface contains method requirements only. It has no fields, lifecycle
members, bodies, default implementations, inheritance, or nested declarations.
Requirement names are unique within one interface; overloads are unavailable.
A class may list multiple distinct interfaces. Conformance is nominal and
exists only through an explicit `implements` clause or inheritance from a base
that already conforms. Repeating a direct interface or redundantly naming an
inherited conformance is invalid.

Fields and methods retain one ordinary member namespace across a complete base
chain. Lookup begins at the receiver's static class and proceeds toward the
root, selecting the nearest declaration. A derived field may not reuse any
inherited field or method name. A derived method may reuse an inherited name
only as a valid explicit `override`; otherwise field hiding, method hiding, and
implicit overriding are errors. Lifecycle slots remain class-owned special
members rather than inherited ordinary names.

Selected fields and non-virtual methods retain the identity and declaring
class of their original declaration. An override retains its own method
identity and joins the virtual family rooted at the inherited declaration.
Lower phases never recreate inheritance or ownership from source names.

The current compiler implements this declaration-graph boundary: validated
base chains, subtype queries, nearest ordinary-member selection, and declaring
owners are available through one resolved identity model. Member bodies do not
yet use inherited selections because the corresponding base-subobject places
are not represented in HIR.

## Virtual methods and overrides

Instance methods are non-virtual by default. `virtual` introduces a family;
`override` extends the nearest inherited family of the same name and is itself
virtual. A virtual root must be a concrete ordinary instance method. Fields,
initializers, copy assignment, destructors, top-level functions, and interface
requirements cannot use `virtual` or `override`.

An override is valid only when the nearest inherited ordinary member with that
name is a virtual method. Compatibility is exact:

- the parameter count, ordered parameter types, and binding modes match;
- the result type matches;
- receiver mutability matches; and
- parameter names need not match.

There are no covariant results, contravariant parameters, overload selection,
implicit overrides, or return/access adaptation. A non-virtual redeclaration
cannot replace a virtual method, and `override` cannot target a field or a
non-virtual method.

A call to a non-virtual method is statically selected. A virtual-family call on
a live complete object or non-owning view selects the implementation for the
complete object's dynamic class. This remains true when a base implementation
calls a virtual method through `self` and when a view is forwarded through
nested calls. A compiler may devirtualize an exact known receiver only when the
observable target is unchanged.

Construction bodies cannot call methods under the existing initializer-body
restrictions. Copy assignment operates on a complete live object and retains
ordinary virtual dispatch. During destruction, a virtual call through `self`
dispatches no further-derived than the class whose destructor body is running;
already-destroyed derived state is therefore unreachable. A sliced inline base
is a new exact base object and dispatches with that exact dynamic class.

## Interface conformance and calls

Conformance is checked after hierarchy and override validation. Every
requirement must be satisfied by one accessible effective instance method of
the same name with exact parameter types, binding modes, result type, and
receiver mutability. A method need not be declared `virtual` to satisfy an
interface. An inherited method may satisfy a requirement, and an override is
the effective implementation for its family.

One class method may satisfy same-signature requirements from several
interfaces. Incompatible same-named requirements remain distinct, but a class
without matching overloads cannot satisfy both. A derived class inherits all
valid base conformances. Its effective override replaces the base method in
the inherited conformance map when the exact requirement remains satisfied.

Calling a requirement through an interface view dynamically selects the
effective method of the complete object's dynamic class. Read-only interface
views may call only read-only requirements. Mutable views may call either
kind. Interface values, method references, and interface-to-interface implicit
conversions are outside this profile.

## `Obj` and the complete-object view model

`Obj` is a universal semantic root for non-owning object views. It is not a
class declaration, physical base subobject, constructible value, or owner of
fields, methods, lifecycle operations, storage layout, or user-visible
metadata. Every class object and every interface view can be viewed as `Obj`.

A polymorphic view has these target-independent semantic components:

1. the identity of the original complete object;
2. that object's dynamic concrete class;
3. one static target: a class, interface, or `Obj`; and
4. read-only or mutable access.

The static target controls available members and conversions. The complete
object and dynamic class control virtual/interface dispatch, type tests, and
narrowing. A class target additionally identifies its unique base subobject
within the complete object. No conversion changes ownership, begins a
lifetime, registers cleanup, or creates a separately storable reference value.

All internal polymorphic alias calls logically carry a complete-object address
and dynamic-class metadata together. The parameter signature supplies the
static target and access mode. Forwarding preserves both runtime components;
view conversion changes only the static target or restricts access. Exact
machine words, metadata records, base adjustment, registers, and stack slots
remain backend decisions, but no backend may reconstruct lost dynamic identity
from a base-subobject address.

Inline objects need no mandatory header: when an exact owning place first
enters a polymorphic call, its statically known concrete class supplies the
dynamic metadata. Metadata identities, ancestry, virtual families, interface
conformance maps, and requirement selections are target-independent. Table
layout, slots, symbols, and address adjustment are not language rules.

## Non-owning conversions and access

Alias parameters extend to class, interface, and `Obj` targets. They remain
call-scoped, non-owning, non-storable, non-returnable, non-rebindable, and
non-exclusive. Eligible sources remain live object places or forwarded aliases;
fresh and returned objects are not made borrowable by polymorphism.

These implicit view conversions are available:

- a class view to the same class or any ancestor class;
- a class view to an interface it effectively conforms to;
- a class or interface view to `Obj`; and
- mutable access to read-only access for the same converted target.

The source complete object and dynamic class are preserved. Read-only access
cannot become mutable. Without interface inheritance, there is no implicit
interface-to-interface conversion. There is no implicit downcast, cross-cast,
`Obj`-to-class/interface conversion, or conversion from an unrelated class.
Because Skald has no overload sets, conversion ranking is unnecessary: the
declared parameter target determines the one required conversion.

Different views may overlap or designate the same complete object. Existing
non-exclusivity remains: no identity or overlap check is inserted, and effects
occur in source evaluation order.

## Inline slicing

An inline derived-to-ancestor conversion slices. It creates or updates an
independent exact ancestor value from the selected base subobject; it does not
retain the source's complete-object identity or dynamic class.

Slicing is implicit only where an expected owning class already exists:

- local or field initialization;
- internal value-parameter initialization;
- an exact-class return destination; and
- whole-object assignment to a live ancestor destination.

Initialization, value arguments, and returns use the target ancestor's selected
copy constructor. Assignment uses its selected copy-assignment operation. The
source is the unique target base subobject of the derived object. The required
target operation must be available, and source/destination evaluation keeps the
existing order. No interface or `Obj` inline destination exists.

Slicing is never one of the two exact-class constructor-elision cases. A fresh
or returned derived source is first completed under the existing temporary or
result rules, then sliced, then cleaned normally. An existing derived place is
used directly as the copy source without an intermediate owning object.

## Base construction and lifecycle composition

Every derived complete object owns exactly one direct base subobject and its
direct fields. The base is part of the same complete-object lifetime and never
has an independent lexical cleanup registration.

An ordinary derived initializer must place exactly one `super(arguments);` as
its first statement. Root initializers cannot contain `super`. The call selects
the direct base's sole ordinary initializer, evaluates arguments left to right,
and initializes the base before any derived-field initialization. After normal
completion, inherited members operate on the live base while the enclosing
derived `self` remains incomplete until all direct fields are initialized.
Missing, duplicate, or later `super` calls are invalid. There is no implicit
zero-argument base call.

Because `super(...)` is first, its arguments may use initializer parameters and
other ordinary expressions but cannot read or alias any part of incomplete
`self`. Argument temporaries remain live through base initialization and are
cleaned at the end of the `super(...)` statement before derived-field
initialization continues.

Copy lifecycle composes automatically; source code does not spell `super` in a
copy constructor or copy-assignment body:

- derived copy construction first copy-constructs the base from the source's
  base subobject, then runs the user copy-constructor body for direct fields or
  the synthesized direct-field sequence;
- derived copy assignment first assigns the base from the source's base
  subobject, then runs the user assignment body or synthesized direct-field
  sequence; and
- destruction runs the derived user destructor body, destroys direct fields in
  reverse declaration order, then runs the complete base destruction sequence.

A user copy constructor requires the selected base copy constructor even when
its direct-field body does not copy every field. A user copy assignment likewise
requires base copy assignment. Synthesized copy construction or assignment is
available only when the base and every direct field support that operation.
Root classes retain the implemented exact-class rules with an empty base step.

Base construction, copying, assignment, and destruction are explicit selected
semantic operations before MIR. They are not inferred from physical prefix
layout and never become an untyped aggregate copy. The base step precedes direct
fields for construction and assignment; destruction reverses complete-object
construction by placing the base sequence last.

The existing destination, temporary, result, registration, and permitted
exact-class elision rules otherwise remain unchanged. Construction and copying
have no recoverable failure in this profile. Failed construction, unwinding,
and partial-copy cleanup remain outside it.

## Type tests

`source is Target` evaluates the source place once and produces `bool`. `Target`
must denote a class, interface, or `Obj`; the source must be a live class place
or a class/interface/`Obj` alias. The operation never changes ownership,
access, lifetime, or dynamic identity.

The compiler classifies each test:

- **static success** when every possible dynamic class of the source satisfies
  the target, including any object tested against `Obj`;
- **static failure** when single inheritance and known conformance make the
  target impossible; or
- **runtime** when a permitted descendant or implementing dynamic class may
  satisfy the target.

Class targets use ancestry of the dynamic class. Interface targets use its
effective conformance map. Static cases still evaluate the source place but
need no metadata query. `is` has no binding side effect and cannot test
primitive, `unit`, function, or owning interface/`Obj` values.

## Checked narrowing

Checked narrowing has one scoped statement form:

```ska
narrow ref dog: Dog = value {
    dog.speak();
}

narrow mut ref editable: Editable = value {
    editable.update();
}
```

The source is evaluated once before the new name is in scope. The source and
target use the same class/interface/`Obj` compatibility relation as `is`.
The target must be a class or interface; narrowing to `Obj` is an ordinary
upcast. A statically impossible narrowing is a compile-time error. A static
success creates the view without a runtime test. Otherwise execution checks
the dynamic class and either enters the block with the new alias or terminates
the process unsuccessfully.

Runtime narrowing failure is unrecoverable in this profile. It does not return
to Skald, does not run remaining source-level cleanup, and exposes no catchable
value. Exact diagnostic text, exit status, signal, and whether the backend uses
an inline trap or runtime helper are implementation contracts, not portable
language behavior.

The narrowed alias preserves the original complete object and dynamic class.
It may retain or reduce source access but never increase it. Its binding exists
only in the trailing block, may shadow an enclosing binding under ordinary
scope rules, cannot escape through a return or stored value, owns no cleanup,
and cannot outlive the source call-scoped alias. Forwarding from the block uses
the normal polymorphic view rules.

## Deterministic validation order

Implementations may recover and report independent errors, but selection
within each dependency group is deterministic:

1. collect top-level names and class/interface headers in source order;
2. validate direct base and `implements` targets in header/source-list order;
3. report one inheritance-cycle diagnostic per cyclic component, ordered by
   the earliest class declaration in each component;
4. validate inherited member collisions and overrides by class declaration,
   then member source order;
5. validate each class's direct conformance list in source order and each
   interface's requirements in declaration order; and
6. resolve and type-check body uses in existing source evaluation order,
   checking a receiver or narrowing source before later explicit arguments or
   nested body work.

An earlier invalid dependency does not authorize lower phases to invent a
placeholder hierarchy, dispatch family, or conformance. Diagnostics remain
structured compiler behavior; exact codes and wording are not language
guarantees.

## Exclusions

This profile excludes:

- multiple class inheritance and interface inheritance;
- access modifiers, `final`, abstract methods/classes, default interface
  bodies, interface fields, overloads, and covariant overrides;
- standalone inline `Obj` or interface values, fields, value parameters, or
  results;
- local/general reference values and narrowed aliases that outlive their block;
- `shared`, heap allocation, reference counting, borrow anchors, and dynamic
  shared destruction;
- external polymorphic/object ABI and cross-module metadata coalescing;
- arrays, optionals, closures, generics, statics/globals, and reflection;
- exceptions, failed-construction unwinding, and partial-copy cleanup;
- unsafe or unchecked casts, user-visible dispatch tables, and user-defined
  conversions; and
- `super` field/method access, qualified base calls, and explicit destructor
  calls.

Their exclusion keeps this implementation profile bounded and does not settle
their eventual design.

## Implementation boundary

Resolution owns hierarchy, member, virtual-family, interface, conformance, and
target identities. HIR owns static targets, selected conversions, dispatch
kinds, view access, lifecycle operations, and static/runtime test decisions.
MIR owns explicit executable views, calls, checks, places, cleanup, and failure
edges and verifies them before target lowering.

The language does not prescribe compiler ID encodings, IR node names, object
offsets, metadata layouts, table slots, symbols, registers, stack locations,
pointer-adjustment algorithms, or runtime allocation headers. Backends may
choose those details only after target-independent IR has preserved the frozen
complete-object, dynamic-class, access, lifecycle, and failure semantics.
