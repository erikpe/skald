# Skald Classes and Lifecycle

Status: authoritative for the implemented inline class, ordinary-initializer
overload, explicit-copy, and base-subobject lifecycle model, including the
frozen path-dependent logical-expression temporary extension. It also defines
how implemented produced read-only aliases compose with initializer selection
and owning full-expression temporaries. The [status matrix](STATUS.md) records
the current compiler boundary.

The [status matrix](STATUS.md) defines feature maturity, the
[grammar](GRAMMAR.md#class-declarations) defines accepted source shape,
[types and values](TYPES_AND_VALUES.md#values-and-places) defines the
value/place distinction, and
[functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md) defines general
call and evaluation-order rules.

## Exact nominal classes

A class declaration introduces one nominal type. Two classes are different
types even when their declarations have identical members. A class may have
one direct base. The compiler validates the canonical hierarchy and carries
base identity through typed lifecycle operations, static member access,
verified MIR, and target lowering. Opt-in virtual method behavior is defined
separately by [polymorphism](POLYMORPHISM.md#virtual-methods-and-overrides).

A class value is one complete inline object containing all of its direct
fields. A class-typed field is a complete inline subobject of its containing
object, not a pointer, nullable handle, alias, or separately allocated object.
Two fields of the same class contain two distinct subobjects.

Classes may be declared in any top-level order. A field, parameter, or result
may name a class declared later in the same source file.

## Members and namespaces

The implemented class model uses these member categories:

| Category | Contract |
|---|---|
| Field | Named inline storage with a primitive or exact-class type. |
| Ordinary initializer | One member of the required overload set that establishes a new complete object. |
| Method | An instance operation with a read-only or mutable receiver; polymorphism defines direct and virtual selection. |
| Static method | A class-owned operation with no receiver or `self`; it is selected through a class spelling. |
| Copy constructor, copy assignment, destructor | Optional or synthesized lifecycle operations defined by this document. |

The implemented [static-field contract](STATIC_FIELDS.md) adds a
separate class-owned storage category with independent declaration identity,
typed places, verified program-owned MIR roots, and target-private storage. A
static field is never an instance field, lifecycle slot, or receiver
operation.

The shared top-level namespace is defined by
[modules and foreign interoperation](MODULES_AND_INTEROP.md#top-level-namespace).
Within each class, fields and ordinary methods share one
non-overloaded member namespace. A field and method in the same class cannot
have the same name, and methods cannot be overloaded. The same member name may
be declared independently by unrelated classes.

Lifecycle declarations occupy dedicated class-owned slots rather than the
ordinary member namespace. `init`, `copy`, `assign`, and `destroy` are
contextual in their declaration or construction-marker shapes and remain
valid ordinary field, method, parameter, local, and top-level function names
elsewhere.

Every class has one or more explicit ordinary `init` declarations, including
an empty class. Those declarations form one class-owned overload set. Copy
construction instead occupies the separate `copy` lifecycle slot; an
`init(ref source: T)` declaration is an ordinary initializer. Direct
construction and direct-base `super(arguments)` use the same overload
selection engine. `T(copy source)` selects the separate copy-construction slot
and never participates in ordinary initializer overload resolution.

Fields, methods, and ordinary initializer overloads are public by default and
may instead begin with the contextual modifier `private`. Copy constructors,
copy assignments, and destructors do not declare visibility. Static methods
use `static fn`; `private static fn` composes the same visibility rule with
receiverless method kind. Ordinary initializers and the other lifecycle
declarations cannot be static. Static-field declarations and inherited
namespace identity are implemented; executable access, lifetime, and
supported zero-state validation remain staged according to
[Static Fields](STATIC_FIELDS.md).

### Declaring-class privacy

A selected private field, method, or ordinary initializer overload is
accessible exactly when the accessing callable is lexically owned by that
declaration's class. This is the authoritative private-member access rule. It
applies uniformly inside ordinary methods, static methods, ordinary
initializers, copy constructors, copy assignments, and destructors, and to
every receiver or construction form. Receiver spelling, dynamic class,
module, import, inheritance, and same-package relationships do not grant
access. Consequently, every body owned by a class may use that exact class's
private declarations, while a derived class, unrelated class, and top-level
function may not.

Lookup still selects the nearest declaration before checking access. A private
inherited declaration therefore remains part of the complete ordinary-member
namespace: it cannot be hidden or redeclared, and an attempted use diagnoses
privacy rather than reporting an unknown member. Resolution preserves
visibility and its source span long enough to diagnose these decisions. Type
checking selects ordinary initializer overloads before checking access, so an
inaccessible most-specific overload does not fall back to a less-specific
public overload. No-match and ambiguity errors retain precedence because they
produce no selected initializer. Typed HIR and later phases contain only
already-authorized member identities and no visibility.

Direct `T(arguments)`, shared `new T(arguments)`, and a derived initializer's
`super(arguments)` enforce the same rule. A derived class therefore cannot
call a private base initializer. Default-length class-element array
construction also checks access at its source call site; the detailed array
capability rule is defined by
[arrays](ARRAYS.md#construction-and-default-initialization).

Static methods use the same rule: a private static method is accessible only
from a body lexically owned by its exact declaring class. It is not a separate
privacy capability.

Privacy does not alter storage or lifecycle. Private fields participate in
finite containment, layout, definite initialization, synthesized and
user-defined copying, assignment, and destruction exactly like public fields.
Private methods always use direct dispatch and cannot implement interface
requirements. The virtual and conformance rules are owned by
[polymorphism](POLYMORPHISM.md#virtual-methods-and-overrides).

Resolution selects inherited ordinary members and rejects implicit
redeclarations across a base chain. A public method may instead explicitly
extend an inherited virtual family with `override`; the exact rules are owned
by [polymorphism](POLYMORPHISM.md#virtual-methods-and-overrides). Typed HIR
projects statically selected receivers through each direct base to the
member's declaring class.

### Static methods

`Class.method(arguments)` selects a public static method from `Class`'s
complete inherited ordinary-member namespace.
`module_binding::Class.method(arguments)` first resolves the visible qualified
class and then applies the same selection. A derived class may select an
inherited public static method while retaining the base declaration's
`MethodId`; inherited private methods remain selected and then fail the
declaring-class privacy check outside their owner.

A static method has no receiver access mode and no `self`. It may call other
static methods, including private helpers allowed by declaring-class privacy,
and it may access private instance members only through an explicit object
value. Static methods are direct, cannot be `mut`, `virtual`, or `override`,
and do not satisfy interface requirements.

The class spelling in a static call is a declaration path, not an evaluated
expression. An unqualified local binding shadows an unqualified class name,
so `name.method()` remains object selection when `name` is a local. Calling a
static method through an object, calling an instance method through a class,
using a field as a static callable, and using a static method as a value are
errors.

## Fields and finite containment

A field has one stored type: `i64`, `u64`, `u8`, `f64`, `bool`, or an exact
class. `unit` has no payload and cannot be a field type. A named field type must
resolve to a class rather than a function or another declaration kind.

Inline containment must be finite. The directed relation from each class to
the classes of its direct fields must be acyclic. Direct self-containment and
indirect cycles are invalid. Forward references, repeated fields of the same
class, acyclic diamonds, and empty contained classes are valid.

Resolved base subobjects participate in the same finite-containment analysis.
A cycle formed by any combination of class fields and direct bases is rejected
before HIR.

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
a mutable receiver place; a read-only method accepts either access level. HIR
selects non-virtual and exact owning calls directly. Calls through forwarded
aliases or `self` to declarations marked `virtual` or `override` name the
virtual family, stable slot, statically selected declaration, and complete-
object origin. A sliced inline base is an independent exact base object and
therefore selects a direct call. MIR retains and verifies dynamic calls and
their receiver origins; the x86-64 backend executes the verified selection.

The root binding determines access for an entire inline path:

- an owning local, owning value parameter, or `mut ref` parameter is mutable;
- `self` has the access declared by its current member body;
- a `ref` parameter is read-only.

Every class-field projection preserves that access. Projection does not create
a new const-qualified type or perform a runtime conversion. Detailed alias
forwarding, overlap, and non-escape rules are defined by
[aliases and ownership](ALIASES_AND_OWNERSHIP.md).

## Object places and projections

An object place identifies existing class storage. It consists of a root and
zero or more class-field projections. Implemented live roots are owning locals,
owning value parameters, `self`, and `ref` or `mut ref` parameters. Grouping
around a root or projection preserves the same place.

Each intermediate projection selects either a direct base identity or a
class-typed field. A class-typed endpoint remains an object place and may be
used in a supported copy, assignment, receiver, or alias context. It is not an
ordinary scalar value.
Selecting a final primitive field reads or writes that field according to the
surrounding expression or statement and the root's access.

### Frozen produced receiver places

The implemented
[produced exact-class receiver contract](FUNCTIONS_AND_CONTROL_FLOW.md#frozen-produced-exact-class-method-receivers)
extends only read-only instance-method receiver selection. An eligible exact-
class producer establishes one hidden caller-owned root whose lifetime is
bounded by the enclosing full expression. That root is read-only, retains the
producer's exact complete-object origin and dynamic class, and may be
canonically projected to the declaring base or an implemented interface
without copying or slicing.

The extension does not turn arbitrary expressions into object places. In
particular, fields cannot be selected from the temporary for direct reading or
writing, and a mutable method cannot use it as a receiver. The compiler accepts
this source form through the ordinary produced-object temporary path; other
temporary member operations remain outside the current boundary.

For example, in `root.branch.leaf.value`, `root` is the root place,
`branch` and `leaf` select complete inline subobjects, and `value` selects the
final primitive field. In `root.branch.leaf.read()`, the `leaf` endpoint is the
method receiver. In `inspect(root.branch.leaf)`, it may be an alias argument
when the parameter expects that exact class.

Field projection is valid only through fields owned by the class at that point
in the path; base projection follows only canonical direct-base edges. A
primitive field cannot be projected further, a method is not a field place,
and member selection does not search unrelated classes.

## Ordinary initializer contract

An ordinary initializer has an implicit mutable `self`, an implicit `unit`
result, and the implemented internal parameter categories described in
[functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#parameters). Its
`self` storage exists while the body runs but is not yet a live complete
object. Each overload is public by default and may be declared
`private init(...)`; visibility does not participate in its signature.

For a root class, the body is a straight-line sequence of direct assignments
to fields of that `self`. A derived initializer must begin with exactly one
`super(arguments);`, followed by the same direct-field sequence. A root
initializer, copy constructor, or other callable cannot contain `super`.
Initializer bodies cannot otherwise contain locals, nested blocks,
conditionals, call statements, explicit returns, or assignment through
another root or a deeper destination. Grouping around `self` does not change
the direct destination.

The base call selects one applicable ordinary initializer from the direct
base's overload set using the same static applicability and specificity rules
as direct construction, then records its stable identity. Arguments evaluate
left to right and may use initializer parameters and ordinary expressions,
but cannot read or alias incomplete `self`. The base becomes live only after a
valid call; direct derived fields cannot be initialized first. Argument
temporaries remain explicit call arguments and end at the `super(...)`
statement boundary.

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

## Ordinary initializer overloads

`Class(arguments)` selects one ordinary initializer from the named class.
`new Class(arguments)` uses the same overload selection when shared allocation
is implemented; `new` changes destination storage and ownership, not which
ordinary initializer is applicable.

An ordinary initializer signature is its ordered parameter types and binding
modes. Parameter names do not participate. Two declarations with the same
arity and ordered parameter types are invalid when their signatures are
identical or differ only by binding mode. In particular, a class cannot use
`init(value: Dog)` and `init(ref value: Dog)` as separate overloads. This
prevents overload selection from silently choosing between copying and
borrowing the same declared type.

A candidate is applicable when its arity matches and every argument can bind
to the corresponding parameter under the ordinary call rules. Constructor
overloading introduces no additional numeric conversion, object conversion,
runtime downcast, or ownership conversion. Among applicable candidates, the
compiler selects the unique most-specific ordered parameter-type sequence:
one candidate is more specific than another when every parameter type is the
same as or a subtype of the corresponding type and at least one is a strict
subtype. Primitive types are comparable only when exact. Binding mode is never
a specificity tiebreaker.

No applicable candidate is a compile-time error. More than one applicable
candidate without a unique most-specific candidate is ambiguous and is also a
compile-time error. Diagnostics identify the supplied static argument types
and competing declared signatures. Selection depends only on static source
types; a source's runtime dynamic class never selects an overload. After
unique selection, the authoritative
[declaring-class privacy rule](#declaring-class-privacy) checks the selected
overload. An explicit checked cast may refine a source before ordinary
overload selection.

For example:

```ska
class Kennel {
    init(ref animal: Animal) {}
    init(ref dog: Dog) {}
}
```

An exact `Dog` place selects the `Dog` overload. A forwarded `ref Animal`
selects the `Animal` overload even when its complete runtime object is a
`Dog`; `Kennel((Dog) animal)` performs the explicit checked cast before
selecting the `Dog` overload.

Constructor delegation between ordinary initializers is not part of this
profile. Every derived ordinary initializer still begins with exactly one
`super(arguments);`, and that call independently selects one overload from
the direct base.

Under the produced read-only alias rule, an exact-class producer
makes a compatible `ref` candidate applicable under these same static rules;
it never makes a `mut ref` candidate applicable. Applicability analysis does
not evaluate or materialize the producer. After unique selection and access
checking, execution evaluates the source exactly once while binding the
selected initializer argument.

## Fresh construction

After overload selection, the arguments must satisfy the selected
initializer's ordered parameters. Destination storage is selected before
argument evaluation, arguments are evaluated from left to right, and the
initializer begins only after every argument is ready. The destination
becomes live only on normal initializer completion.

A fresh object may directly initialize an exact-class local or a direct class
field as described above. Explicit array element-list construction also makes
one element position a direct destination for an eligible ungrouped fresh
construction. A fresh object is also an
object source in the copy, assignment, argument, and return contexts below.

Skald has no recoverable construction failure. Exceptional initialization,
partially completed object cleanup, and delegation between ordinary
initializers are not defined by the implemented model.
The constraints on any future exceptional construction path are owned by
[errors and exceptional control flow](ERRORS.md#cleanup-and-abrupt-termination).

## Lifecycle declarations

Copy construction, copy assignment, and destruction occupy independent
class-owned slots. Each slot may have at most one explicit declaration. These
members have an implicit `unit` result, do not accept `private`, and are not
ordinary callable methods.

| Operation | Declaration | Receiver state | Body contract |
|---|---|---|---|
| Copy construction | `copy(ref source: T) { ... }` inside `T` | Mutable, incomplete `self` | Straight-line initialization of every direct field exactly once. |
| Copy assignment | `assign(ref source: T) { ... }` inside `T` | Mutable, live `self` | General mutable `unit`-method statements; may update any supported subset of fields. |
| Destruction | `destroy { ... }` | Mutable, live `self` | General mutable `unit`-method statements; `return;` and fallthrough complete the body. |

The source parameter name is arbitrary, but its read-only binding mode and
exact enclosing class are required for both `copy` and `assign`. Those
declarations take no modifiers, explicit result, semicolon, or additional
parameters. A malformed declaration does not become an overload or a
different lifecycle operation. In particular, malformed `copy` never falls
back to an ordinary initializer, while `init(ref source: T)` remains an
ordinary initializer.

A copy-constructor body follows the ordinary initializer's definite-field
rules. A primitive field is initialized from an exact primitive expression. A
class field may be freshly constructed in place or copied from a live place of
its exact class. The source parameter is complete and read-only; `self` is not
complete until every direct field has been initialized.

A destruction body runs while the complete object and all its fields are
live. It may create and copy object locals, use fresh-object temporaries, and
perform supported assignments. It cannot explicitly invoke a special
destructor, end a lifetime early, or return a value. An ordinary method named
`destroy` is a separate callable with no lifecycle effect.

## Base-subobject lifecycle

One derived complete object owns its direct base subobject and direct fields
under one lifetime. The base is not a separately registered lexical cleanup
root. HIR and MIR record the selected ordinary base initializer, the base copy
constructor and assignment capabilities, and an ordered destruction plan.

Copy construction and assignment process the direct base before the derived
body or synthesized direct fields. This requirement applies to user-defined
derived copy operations as well as synthesized ones. Destruction runs the
derived user body, direct class fields in reverse declaration order, and then
the direct base's complete recursive destruction plan. These are semantic
operations, not aggregate prefix copies or inferred physical layout.

This contract reaches verified MIR. Direct-base metadata, base projections,
selected base copy steps, terminal base destruction, and owning slices are
explicit and checked before target lowering. The x86-64 backend embeds the
direct base before derived fields and mechanically lowers those selected
operations without aggregate copying.

## Copy capabilities

Copy construction and copy assignment are separate capabilities. An explicit
declaration supplies that operation's complete user body. It neither requests
nor receives an implicit field-wise prefix or suffix.

For a derived class, the direct base capability is checked first. A user
operation is unavailable when its required base operation is unavailable,
even if its body does not use every direct field. When a declaration is
absent, the compiler synthesizes the operation exactly when the base and every
direct field support it. Primitive fields support both capabilities. An
exact-class field recursively uses its class's selected operation. Empty root
classes and primitive-only root classes therefore support both operations;
one unavailable base or field capability makes only the corresponding
containing capability unavailable.

A synthesized copy constructor initializes direct fields in source declaration
order. It copies primitive payloads exactly and copy-constructs class fields.
A synthesized assignment uses the same declaration order, assigning primitive
payloads and invoking class-field assignment. Floating-point payload copying
preserves the stored bits. User lifecycle effects selected anywhere in these
sequences remain observable.

Declaring a destructor does not suppress either synthesized copy capability.
Capability selection is determined before lowering and does not depend on a
target representation.

## Copy construction and object sources

Copy construction establishes a distinct new object from one exact-class
source. A source may be:

- an existing live object place, including an owning local, value parameter,
  receiver, alias, or supported field projection;
- a bounded pointee place explicitly selected from a shared owner with
  `*owner`;
- a fresh construction of the exact class; or
- the exact-class result of an internal function or method.

For an existing place, the destination is reserved first and the selected copy
constructor runs once with a read-only view of that source. The destination
becomes live and acquires cleanup responsibility only after the operation
completes. Reserving storage alone does not begin a lifetime or register
cleanup.

Fresh and returned sources may instead require the temporary or direct-result
rules below. Skald has no moves: producing one object does not implicitly end
another object's lifetime or transfer its cleanup registration.

Implicit owning contexts continue to select copy construction where this
document already requires it:

```ska
var copy: T = source;
```

If the source is shared, the owning context still requires explicit pointee
selection: `var copy: T = *owner;`. This copies the selected object into
independent inline storage; it does not copy or replace the shared handle.

The explicit construction form is:

```ska
var copy: T = T(copy source);
```

`copy` in this position selects the copy-constructor capability directly. It
is not an ordinary initializer argument and takes exactly one source
expression. Conversely, `T(source)` participates only in ordinary initializer
overload resolution and never falls back to copy construction when no
ordinary initializer matches.

The target `T` makes `T(copy source)` a target-directed checked-copy context.
The compiler statically selects an exact or ancestor `T` place when guaranteed,
performs the existing object-view runtime check when a forwarded
class/interface/`Obj` source can dynamically supply `T`, and rejects a
statically impossible source. An exact inline base value cannot dynamically
recover a derived object that slicing already discarded. A successful
ancestor selection may deliberately slice into the exact `T` destination.
The source is evaluated once, remains live through the selected copy
constructor, and is released or cleaned only after the destination is live.

An explicit place cast inside the copy source remains meaningful as an
additional refinement, for example `Animal(copy (Dog) source)`. The inner cast
selects and checks the `Dog` view; the outer construction still copies an exact
`Animal`. Explicit copy construction is not eligible for copy elision and
does not preserve an arbitrary runtime dynamic class.

## Assignment to a live object

Whole-object assignment updates an already-live object without beginning or
ending its lifetime. The destination is selected before the source is
evaluated. It must be mutable and rooted at an owning local, owning value
parameter, or a class field reached through a mutable owning root or live
mutable `self`.

The complete `self` object cannot be replaced from within its member body.
An alias parameter cannot be rebound, and whole-object replacement through an
alias-rooted path is unsupported even when the alias is mutable. These rules
do not prevent mutation of primitive fields or supported nested operations
through `mut ref`.

The source must be a live or produced object of the exact destination class.
The selected assignment operation runs once. Assignment does not destroy,
reconstruct, unregister, or reregister the destination.

Source and destination may designate the same object. The compiler inserts no
identity test: user assignment runs normally, and synthesized assignment runs
its declaration-ordered field sequence. This preserves user effects in both
the enclosing operation and recursively selected field operations.

Assignment from a fresh construction first materializes a temporary, assigns
from it, then destroys it at the statement boundary. It is not copy elision.

## Owning value parameters

An internal exact-class value parameter owns a distinct callee object. For
each call argument, from left to right, its parameter destination is selected
and initialized before evaluation proceeds to the next argument. An existing
place is copy-constructed directly into that destination. A produced source is
materialized, copied into the parameter, and remains live through the call.
The callee begins only after all arguments and owning parameters are complete.

On a normal exit, the callee destroys body temporaries and lexical locals
first, then owning class value parameters in reverse parameter order. Alias
parameters are non-owning and are never cleaned. Exact-class value parameters
are not supported in external declarations.

## Object results

An exact-class result from an internal function or method is initialized in a
distinct semantic result destination supplied by the caller. Returning an
existing place copy-constructs the result before callee cleanup. A produced
object follows the materialization and elision rules below. The result becomes
live only after its selected operation completes.

The callee does not destroy a completed result. Ownership passes to the caller,
which either uses it as a final initialization destination or owns it as a
temporary. Callee cleanup therefore cannot invalidate the result. A read-only
or mutable alias may be a copy source, but the alias itself is never returned.

An object-returning call directly initializes an exact-class local when that
local is its final destination. Under explicit array element-list
construction, an exact-class element position is likewise the final destination
for an eligible listed call. In other source contexts it materializes a
temporary. These are source-visible destination and lifetime rules; they do
not prescribe an implementation calling convention. The array-element case is
implemented through type checking and execution.

## Temporaries and full expressions

A fresh construction or object-returning call materializes an owning temporary
when it cannot use an eligible final destination directly. Copying from an
existing place does not create an intermediate temporary. A temporary becomes
live only after successful completion and is destroyed exactly once, in reverse
completion order, at the end of its full expression.

The implemented full-expression boundaries are:

- one complete local initializer;
- the complete right side of an assignment statement;
- one effect-only call statement, including its arguments; and
- one return expression;
- one complete `if` or `elif` condition; and
- one complete `while` condition evaluation.

Argument temporaries remain live through the call and are cleaned after its
result has been secured. A newly initialized local becomes live and registered
before its initializer temporaries are cleaned. On return, the result is
completed first, then expression temporaries are destroyed, followed by
lexical locals and owning value parameters.

Produced read-only aliases use this same temporary
category and boundary. Its exact-class producer initializes one hidden
caller-owned temporary directly at the argument position; alias binding adds
no copy and no callee-owned parameter object. The temporary remains live
through later arguments and the complete call, then is destroyed with other
full-expression temporaries. `mut ref`, produced primitives, optional
containers, arrays, and raw shared handles do not acquire this rule. The
complete eligibility and non-escape contract is defined by
[aliases and ownership](ALIASES_AND_OWNERSHIP.md#implemented-produced-read-only-alias-arguments).

Grouping does not change an existing place, but it does change whether a fresh
construction matches the restricted elision forms below.

The implemented `&&` and `||` contract introduces path-dependent temporary
ownership inside a full expression. A selected right operand may complete
owning temporaries that a skipped right operand does not. Every completed
temporary remains live until the common enclosing boundary and is destroyed
exactly once; no cleanup is scheduled for a temporary that never became live.
The compiler representation must either retain that path dependence through
the boundary or keep continuations distinct until their lifetime states are
compatible. Source semantics never clean a logical operand early merely to
simplify a join.

## Permitted copy elision

The implemented compiler elides the copy for an ungrouped fresh construction
of the exact destination class in exactly these forms:

```ska
var value: T = T(arguments);

fn make() -> T {
    return T(arguments);
}
```

The non-elided abstract execution would construct a temporary, copy-construct
the destination, and destroy the temporary. A valid copy constructor is still
required. With elision, the ordinary initializer runs once in the final
destination; the copy-constructor operation and the omitted temporary's
destruction are absent, including their possible user effects. The current
compiler makes this choice for every eligible occurrence, deterministically.

Grouping the construction prevents elision. Assignment, call arguments,
copying from an existing place, initialization from a function result, and a
named-return optimization are not eligible. Direct construction of a class
field is its initialization rule, and direct placement of an object-returning
call into a local is result placement; neither is an additional elision case.

The explicit array element-list contract adds one destination rule
rather than a general class move or elision facility:

```ska
var values: T[] = T[]{T(arguments), make_t()};
```

The eligible ungrouped `T(arguments)` initializes its
uninitialized element slot directly, and the eligible exact-`T` result from
`make_t()` uses its element slot as the final result destination. Grouping a
fresh construction continues to prevent direct placement and therefore
requires the ordinary materialize, copy-construct, and destroy behavior. A
named or otherwise non-elided source likewise requires the selected copy
constructor. The rule neither requires a default constructor or copy
assignment nor makes the resulting array copyable when `T` lacks copy
construction. Detailed ordering and publication belong to
[Arrays](ARRAYS.md#explicit-element-list-construction).

## Lifetime registration and normal cleanup

The owning roots are completed class locals, class value parameters, and
materialized class temporaries. A completed object result becomes caller-owned.
Receivers and aliases are non-owning. An inline class field is destroyed with
its containing object rather than registered as a separate lexical owner.

The produced read-only alias rule specifies its hidden exact-class storage as
one of these ordinary materialized class temporaries. Type checking and HIR
identify that source, and verified MIR establishes the registration described
here. The alias parameter itself remains non-owning and receives no independent
registration.

An owning place is registered only after its complete initialization or copy
construction finishes. On normal fallthrough, each scope destroys registered
objects in reverse completion order. Only the executed conditional arm
registers objects, and its child scope is cleaned before control reaches the
join.

On `return`, the result is completed first. Return-expression temporaries are
then destroyed in reverse completion order, followed by lexical locals from
innermost scope to outermost and in reverse completion order within each
scope. Owning value parameters are destroyed last, in reverse parameter order,
before the completed result transfers to the caller. A `unit` fallthrough uses
the same cleanup rule without a result.

These rules cover normal exits only. The current language has no exceptions,
recoverable failed construction or copying, loop exits, or explicit early
destruction. Process termination and future exceptional cleanup are separated
in [errors and exceptional control flow](ERRORS.md#cleanup-and-abrupt-termination).

## Complete-object destruction

Destroying one complete object performs the following sequence exactly once:

1. run its user `destroy` body, if declared;
2. clean all objects owned by that body before the body completes;
3. destroy exact-class fields recursively in reverse source declaration order;
4. for a derived object, run the direct base's complete destruction sequence;
5. end the complete object's lifetime.

Primitive fields require no destruction step. An absent user declaration is
an empty first step and does not suppress field cleanup. Field order is based
on declaration order, independent of initializer statement order. Finite
containment makes recursive destruction finite.

The receiver remains complete and live throughout the user body. Once field
cleanup begins, no source code can observe a partially destroyed receiver.
Destruction ends the inline object's lifetime but does not require heap
deallocation or any particular storage operation.

## Unsupported extensions

Closed generic class specializations now participate in the typed lifecycle
model described above. After substitution, exact fields, recursive optionals,
arrays, shared owners, optional owners, shared optional boxes, and an ordinary
direct base contribute the same selected copy, assignment, and reverse-order
destruction steps as equivalent hand-written classes. A closed specialization
does not require a type argument to be copyable merely because it exists;
copy construction or assignment becomes necessary only when a substituted
declaration or member body uses that operation. Closed inheritance and
conformance likewise use the ordinary exact-class pipeline after
specialization.

The implemented executable class model includes primitive and exact-class
optional fields in ordinary initialization and synthesized copy
construction/assignment. They must be initialized exactly once and may hold
`none`; a present exact-class payload is destroyed conditionally in reverse
field order. Checked optional-class payload access uses bounded presence
guards. Optional `(shared T)?` fields also execute: absence owns nothing,
presence owns one strong handle, synthesized copy retains conditionally, and
assignment/destruction conditionally release in the ordinary reverse field
order. The model executes dependency-ordered eager static initialization for
ordinary stored class values and complete-object destruction during exact-
reverse normal-return static shutdown. It does not include lifecycle-member
visibility, protected access, `final`, abstract members,
method overloads, reflection, or user-defined conversions. Exact shared
allocations, owners, calls, results, and owning fields execute; shared fields
follow the ordinary target layout, copy lifecycle, and derived-to-base
destruction plan. Ordinary direct and base-initializer overloads,
the distinct `copy` declaration, and target-directed `T(copy source)`
construction execute.
The implemented [optional-values contract](OPTIONAL_VALUES.md) uses explicit `T?`
rather than nullable plain class values. Primitive optional fields use inline
state plus payload storage and participate in the class lifecycle described
above. Exact-class `T?` reserves inline payload storage and applies the existing
payload copy/assignment/destruction operations conditionally, so it does not
make recursive inline containment finite; executable `(shared T)?` is the finite
optional-owner form.
Direct-base syntax, hierarchy validation, inherited selection and lifecycle,
class/interface/`Obj` alias views, slicing, virtual dispatch, interface
dispatch, type tests, and checked object casts execute on x86-64. Their
maturity is recorded in the
[status matrix](STATUS.md), and the
[polymorphism profile](POLYMORPHISM.md) owns their language contract.

Shared-field semantics and lifecycle are specified by
[Shared Ownership and Heap Allocation](SHARED_OWNERSHIP.md). Shared edges are
excluded from finite inline containment, must be initialized exactly once,
participate in user and synthesized copy operations, and appear as distinct
reverse-order release steps in target-independent destruction plans. Their
target layout and execution are implemented. The explicit
`new T(copy source)` copy-allocation form invokes this document's selected
exact-`T` copy constructor once from a target-directed checked `T` place and
is not eligible for copy elision.
Preserving an arbitrary source dynamic class through cloning remains deferred.

This document specifies source-visible class and initialization behavior. It
does not prescribe compiler identities, phase data structures, containment
algorithms, object offsets, hidden receiver placement, calling conventions,
frame storage, or backend address calculation.
