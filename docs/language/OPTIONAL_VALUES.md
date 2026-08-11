# Optional Values

Status: implemented language contract for primitive, exact inline-class, and
optional shared-owner values across owning locals, fields, internal callable
boundaries, and aliases to supported inline optional containers. This document
also defines the implemented compositional type syntax, nested access and
callable behavior, and optional inline arrays. Recursive owning lifecycle and
access are executable. Optional arrays execute in every supported owning,
aggregate, internal callable, array-element, and call-scoped alias position. The
[status matrix](STATUS.md) is authoritative for availability, and
the [implemented grammar](GRAMMAR.md) remains the exact syntax currently
accepted by the compiler. This document also freezes the not-yet-implemented
[shared optional box](#shared-optional-boxes) contract.

This document defines Skald's source-level optional-value contract. Primitive
`i64?`, `u64?`, `u8?`, `f64?`, and `bool?` values and exact inline class `T?`
values now execute end to end in
owning locals, fields, internal value parameters/results, methods, interfaces,
virtual overrides, and initializer overloads. Inline class payload access
through postfix `!` executes as a bounded checked view. Optional shared owners,
written canonically as `(shared T)?`, execute through the same internal owning
positions and secure a normal non-null owner on unwrap. Read-only and mutable aliases may designate
supported inline optional containers without creating optional reference
values.
Compiler representation, verification, and ABI direction are defined in the
[optional-values compiler contract](../compiler/OPTIONAL_VALUES.md).

## Implemented owning-value slice

The current executable profile permits primitive optional owning values
initialized from `none`, an exact ordinary primitive value, or another exact
optional:

```ska
var empty: i64? = none;
var present: i64? = 41;
var copy: i64? = present;
copy = empty;
copy = 42;
```

`is some` and `is none` produce `bool`. Postfix `!` copies a present primitive
payload and terminates unsuccessfully when the optional is absent. The
optional itself never has truthiness and never implicitly becomes its payload.
The same rules apply to all five primitive types.

Exact class optionals additionally own the conditional payload lifecycle:

```ska
var empty_item: Item? = none;
var item: Item? = Item();
var copied_item: Item? = item;
copied_item = none;
```

They work in locals, fields, internal value parameters/results, produced call
results, initializer overloads, and synthesized class lifecycle. Presence
tests and checked class payload consumers execute, including `item!.field`,
`item!.run()`, aliases, casts, type tests, and owning copies.

## Core invariant

Optionality never weakens an ordinary type. Every plain primitive, inline
class, alias, and `shared T` value remains present and valid for its complete
lifetime. Absence exists only in a type whose source spelling explicitly
contains an optional marker.

An inline `T?` has exactly two source-visible states:

- **absent**, with no live `T`; or
- **present**, with exactly one complete live and valid `T`.

An optional shared owner `(shared T)?` likewise has exactly two states:

- **absent**, accounting for no strong owner; or
- **present**, containing one ordinary non-null `shared T` owner.

There is no null, invalid, dangling, moved-from, partially initialized, or
partially destroyed value of type `T`. When an optional is absent, there is no
`T` value to access.

## Type forms

The implemented profile separates optional inline payloads from optional
shared ownership. `shared? T` remains accepted as exact source shorthand for
the canonical `(shared T)?` form:

| Type | Meaning | Implemented profile |
|---|---|---|
| `T` | Always-present inline `T` | Existing contract |
| primitive `T?` | Inline optional containing zero or one primitive `T` | Owning locals, fields, and internal callable boundaries execute |
| class `T?` | Inline optional containing zero or one exact class `T` | Owning lifecycle, internal boundaries, and bounded checked payload access execute |
| `shared T` | Always-present non-null shared owner of `T` | Existing contract |
| `(shared T)?` | Optional containing zero or one `shared T` owner | Internal owning lifecycle and checked unwrap execute |
| `shared? T` | Exact shorthand for `(shared T)?` | Same type, lifecycle, layout, and ABI |
| `T[]?` | Tagged optional containing zero or one inline array | Supported owning, internal callable, aggregate, array-element, and checked-alias positions execute |
| `shared T?` | Non-null shared box containing `T?` | Frozen design; not implemented |
| `shared? T?` | Optional owner of a non-null shared box containing `T?` | Frozen design; not implemented |

In the implemented grammar, `shared?` is the contextual word `shared` followed
by the `?` punctuation token. Ordinary trivia may separate those tokens.
Source-shaped syntax inspection retains the shorthand, while semantic dumps
use canonical `(shared T)?` independently of the source spelling.

Inline `T?` is valid when `T` is a primitive, exact inline class, ordinary
shared owner, or another supported optional type.
`(shared T)?` and its `shared? T` shorthand accept the same class, interface,
`Obj`, and array targets as ordinary `shared T`.

The current compiler rejects:

- `unit?`;
- standalone optional interface or `Obj` views;
- optional function types;
- `shared T?` and `shared? T?`;
- `ref?` and `mut ref?`; and
- every optional external parameter or result.

The recursive syntax tree preserves these complete type shapes. Shared boxes
therefore fail at a focused semantic boundary rather than being discarded
during parsing. Nested optionals and optional arrays execute in owning
storage, aliases, checked access, and every supported internal callable
boundary.

The shared-box rejections are implementation staging, not an unresolved
semantic boundary. Their frozen behavior is defined below; all other listed
categories remain outside that design.

## Compositional optional types

The type construction and grouping rules in this section are implemented
syntax. Canonical optional shared owners execute through the existing owner
semantics. Recursive optional identities, nested owning lifecycle,
`some(expression)`, checked access, aliases, and internal callable integration
and optional inline arrays are implemented across their supported owning,
aggregate, callable, array-element, and checked-alias positions.

### Type construction, grouping, and precedence

Postfix `?` constructs one optional layer around the complete type expression
immediately to its left. Parentheses group any storage type before another
postfix suffix is applied. Postfix `?` and `[]` associate from left to right:

```text
T?[]   = Array<Optional<T>>
T[]?   = Optional<Array<T>>
T??    = Optional<Optional<T>>
```

An ordinary leading `shared` consumes the complete following inline type,
including its postfix suffixes. Optionality written inside that operand belongs
to the allocation target:

```text
shared T?   = Shared<Optional<T>>
shared T[]  = Shared<Array<T>>
```

The first form requires the frozen shared-box allocation kind and is not yet
implemented. To place optionality around an existing shared owner,
group the shared type first:

```text
(shared T)? = Optional<Shared<T>>
```

`shared? P` is exact shorthand for `(shared P)?`. The shorthand marker is the
`?` immediately following the contextual `shared` word; it is not a postfix
marker on `P`. Expanding the shorthand before interpreting the payload gives:

```text
shared? T   = (shared T)?
shared? T[] = (shared T[])?
shared? T?  = (shared T?)?
```

The last form contains the not-yet-implemented inner
`Shared<Optional<T>>` box.

The exact spelling and identity matrix is:

| Source spelling | Normalized type | Meaning | Current availability |
|---|---|---|---|
| `T?[]` | `Array<Optional<T>>` | Array whose elements are optional `T` values | Implemented for currently eligible `T` |
| `T[]?` | `Optional<Array<T>>` | Optional inline array value | Core local and top-level internal function value lifecycle executes |
| `(T[])?` | `Optional<Array<T>>` | Grouped spelling equivalent to `T[]?` | Exact equivalent of `T[]?` |
| `(shared T)?` | `Optional<Shared<T>>` | Optional owner of an ordinary non-null shared allocation | Implemented canonical form |
| `shared? T` | `Optional<Shared<T>>` | Exact shorthand for `(shared T)?` | Implemented alias |
| `(shared T)??` | `Optional<Optional<Shared<T>>>` | Nested optional around an optional shared owner | Owning lifecycle, checked access, aliases, and internal calls execute |
| `shared T?` | `Shared<Optional<T>>` | Non-null owner of a shared box containing `T?` | Frozen design; not implemented |
| `shared? T?` | `Optional<Shared<Optional<T>>>` | Optional owner of that shared box | Frozen design; not implemented |

Canonical documentation and semantic dumps use `T[]?` for an optional array
and `(shared T)?` for an optional shared owner. Source-shaped syntax inspection
retains the user's grouping or `shared?` shorthand for diagnostics. Alias
spelling never creates a distinct type identity or conversion.

### Recursive states and explicit presence

Every optional layer independently has an absent state and a present state
containing one complete live value of its immediate payload type. Nested
optionals are never flattened. `T??`, for example, has these three shapes:

```text
none                 outer absent
some(none)           outer present, inner absent
some(value)          outer present, inner present with T
```

The final example uses the ordinary one-layer injection from `T` to the inner
expected `T?`. More generally, `N` optional layers around `T` have `N + 1`
observable presence shapes: absence at any one of the nested layers, or every
layer present with a `T` payload.

`some(expression)` is an expected-type-directed primary expression. It is
valid only where one unambiguous expected `Optional<P>` type exists. It creates
the outer optional as present and checks `expression` against expected payload
type `P`. Like `none`, it has no universal standalone type. This makes inner
absence explicit without adding recursive implicit conversion:

```ska
var outer_absent: Item?? = none;
var inner_absent: Item?? = some(none);
var fully_present: Item?? = some(Item());
```

Implicit injection always adds exactly one layer. A source of exact type `P`
may satisfy expected `P?`; a source of `T` does not directly satisfy `T??`.
An existing `T?` may satisfy `T??` by one injection, and
`some(value_of_type_T)` may construct a fully present `T??` because its
argument is checked against the immediate payload `T?` and uses one injection
there. The compiler never searches an arbitrary chain of optional liftings.

Exact type matches outrank one-layer injection. `none` and `some(...)` use each
candidate's expected optional type during overload selection, but neither
contributes an invented payload type or breaks a remaining ambiguity. Optional
types remain exact components of virtual overrides and interface signatures.

### Recursive lifecycle and checked access

Initialization, copying, assignment, and destruction first inspect the outer
layer, then perform the already-selected operation for its immediate payload
only when that layer is present. The existing transition matrix therefore
applies recursively. Publication of a present outer layer occurs only after
its complete immediate payload, including every inner wrapper, is initialized.

Each postfix `!` evaluates its source once, checks one outer layer, and removes
only that layer. Chained `!` operations perform their checks from outermost to
innermost in source order. A failure terminates before any later check or
consumer runs.

When the immediate payload is another optional or an inline array, an owning
consumer receives the ordinary copy or transfer selected for that complete
payload. A non-owning consumer uses the same bounded checked-place discipline
as existing class payloads where a place is required. Every checked view pins
the particular optional layer whose payload it exposes. Nested views may
therefore hold nested guards; clearing, replacing, or destroying any guarded
container terminates before changing that layer. A shared-root anchor keeps
the containing allocation alive independently of every optional guard.

Supported optionals are permitted in internal locals,
fields, class-owned statics, value parameters/results, methods, interfaces,
overrides, initializer overloads, temporaries, array elements, and explicit
element-list destinations. `ref` and `mut ref` parameters may designate any
supported optional container, including nested optionals, optional arrays, and
optional shared owners. Such an alias borrows the always-present wrapper; it
does not create an optional reference. External optional signatures remain
unsupported.

Static optionals begin absent when the underlying static-field contract permits
zero-default initialization. On normal program return, a present payload uses
the ordinary recursive cleanup selected for its immediate type during exact
reverse static shutdown. Abrupt termination remains non-unwinding.

### Optional arrays and containment

`T[]?` contains either no array value or one complete live inline `T[]`
descriptor. A present empty array and an absent optional array are distinct.
The wrapper therefore uses the ordinary explicit optional state rather than
reusing the valid empty descriptor representation.

Present optional arrays retain ordinary array invariance, backing ownership,
copy construction, produced-backing transfer, assignment, element lifecycle,
and reverse cleanup. Checked unwrap copies one complete array value out after
testing the outer tag. When `array_optional!` is consumed by a `ref T[]` or
`mut ref T[]` parameter, the compiler instead exposes a checked call-scoped
payload place: the wrapper remains presence-guarded and the backing remains
anchored through the complete immediate call. Mutable writes affect the
original array, while replacement of the guarded optional terminates.
Optional arrays also execute in fields, statics, methods, interfaces,
overrides, initializer overloads, and array elements. `shared? T[]`, normalized as
`(shared T[])?`, remains an optional owner of a shared array allocation and is
not an inline optional array.

Optional containment follows the complete immediate payload. Wrapping an
inline class in any number of optional layers does not break an inline
containment cycle. An array descriptor remains an indirection boundary, and a
shared owner remains a shared edge, even when either is wrapped in optionals.

## Shared optional boxes

Status: **frozen design; not implemented**. The compiler continues to reject
the box type forms during resolution and does not yet parse their allocation
expressions. The implementation roadmap may stage support but must not reopen
the source semantics in this section.

`shared P?` is a non-null strong owner of one allocation containing a complete
optional `P?` wrapper. It is `Shared<Optional<P>>`, not an optional ordinary
owner. `(shared P?)?` is an optional owner of that box, and `shared? P?` is
exact shorthand for that canonical form:

| Source type | Canonical model | Meaning |
|---|---|---|
| `shared P?` | `Shared<Optional<P>>` | One non-null owner of an optional box |
| `(shared P?)?` | `Optional<Shared<Optional<P>>>` | Zero or one owner of that box |
| `shared? P?` | `Optional<Shared<Optional<P>>>` | Exact shorthand for `(shared P?)?` |
| `(shared P)??` | `Optional<Optional<Shared<P>>>` | Nested optional around an ordinary owner |
| `(shared P?)??` | `Optional<Optional<Shared<Optional<P>>>>` | Nested optional around a box owner |

No optional layer is flattened. Arbitrary additional postfix layers remain
valid within the ordinary syntax nesting budget. Primitive, exact-class,
inline-array, shared-owner, and recursively nested optional targets are exact.
An optional object box additionally permits class/base/interface/`Obj` static
views while retaining one exact concrete dynamic allocation class. Bare
owning interface and `Obj` optionals remain invalid; their box spellings are
views rather than new inline value types.

The allocation forms are:

```ska
var absent: shared Item? = new Item?();
var also_absent: shared Item? = new Item?(none);
var present: shared Item? = new Item?(Item());
var nested: shared Item?? = new Item??(some(none));
```

`new P?()` default-initializes the exact optional target to outer absence.
`new P?(expression)` accepts exactly one expression and initializes the exact
`P?` wrapper through its ordinary absent, one-layer injection, `some`, copy,
transfer, or direct-payload plan. The expression is not forwarded as an
initializer argument list for `P`. The allocation is published only after the
complete wrapper is initialized.

The concrete class named after `new` fixes an object box's dynamic class and
exact payload layout even while absent. Initializers must satisfy that exact
target before the produced owner is up-viewed:

```ska
var base: shared Base? = new Derived?(Derived()); // valid up-view
var bad: shared Base? = new Derived?(Base());     // invalid construction
```

Copying a named box owner retains the same allocation. A produced owner
transfers normally. `new P?(*box)` instead creates an independent allocation
containing an ordinary copy of the optional payload when that copy capability
exists.

Box access crosses the ownership edge explicitly. `*box` exposes the boxed
optional wrapper for presence tests, owning copies, read-only eligible aliases,
and checked unwrap; it does not implicitly forward `is`, `!`, `.`, or `->`
through the owner:

```ska
if (*box is some) {
    (*box)!.use_item();
}
```

The published wrapper is shallowly immutable. Its absence/presence and
complete contained value never change, so whole-pointee assignment and a
`mut ref P?` alias are invalid even through an exact box view. An owner local,
field, static, or array slot may instead be assigned a newly allocated box;
other owners keep observing the old allocation. A present contained object or
other mutable aggregate retains its ordinary internal mutation operations.
Consequently an absent box remains absent for its allocation lifetime.

Object-box views are covariant because no view can replace the complete
wrapper. Base, interface, and `Obj` views preserve the exact dynamic class for
type tests, checked owner casts, and virtual or interface dispatch:

```ska
var base_box: shared Base? = new Derived?(Derived());
(*base_box)!.foo(); // dispatches through dynamic Derived metadata

var interface_box: shared Interface? =
    new Implementation?(Implementation());
(*interface_box)!.foo(); // dispatches to Implementation
```

Static up-views preserve the owner. Possible downcasts inspect allocation
metadata, impossible relations are rejected, and casts never allocate, copy
the wrapper, or change presence. Primitive, array, nested value, shared-owner,
and other non-object box targets remain invariant.

Box owners are eligible in locals, fields, internal value parameters/results,
explicitly initialized statics, temporaries, and array elements. An optional
box owner uses the existing absent zero default. Requested nonempty default
construction of `(shared P?)[]` creates one distinct absent box per element;
slots never share a synthesized default box. External shared-box signatures
remain invalid.

Checked payload access combines an owner or hidden owner anchor with the
existing optional guard. A read-only `ref P?` may designate an exact box
wrapper for one call; mutable whole-wrapper aliases, aliases whose designated
type is a shared owner, optional references, first-class references, and
escaping pointers into a box remain invalid. This feature adds no mutable
shared optional cell, checked covariant store, store-related failure, or
atomic/thread-safety rule.

## Empty and present values

`none` is the reserved empty-optional expression. It receives its exact type
from one unambiguous expected optional boundary:

```ska
var inline_value: Item? = none;
var shared_owner: (shared Item)? = none;
```

The expected type may come from a local or field initialization, assignment,
argument, return, or initializer candidate. The implemented profile supplies
all of those boundaries for every supported optional. `none` used without
one unambiguous optional expectation is invalid. It does not have a universal
runtime type.

An ordinary value may be injected into its corresponding optional:

```ska
var inline_value: Item? = Item();
var shared_owner: (shared Item)? = new Item();
```

The source remains an ordinary valid `Item` or `shared Item`. Injection creates
or updates the surrounding optional; it does not add an absent state to the
source type.

This is the sole new implicit value conversion. An optional never converts
implicitly to its payload.

## Explicit initialization

Optional storage is initialized explicitly, just like other Skald storage:

```ska
var empty: Item? = none;
var present: Item? = Item();
```

Initializer-free storage remains invalid:

```ska
var value: Item?; // invalid
```

An optional field must be initialized exactly once by every ordinary or copy
initializer. Assigning `none` is a complete field initialization; absence is a
valid optional state rather than uninitialized storage.

Fresh ungrouped exact construction into a newly initialized optional constructs
directly in the payload destination:

```ska
var value: Item? = Item(arguments);
```

This is a specified optional-payload destination rule. It does not create a
temporary `Item`, invoke copy construction, or extend the existing optional
copy-elision permission. Other source shapes retain their ordinary materialize,
copy, assignment, and cleanup behavior.

The same initialization rules apply to explicit inline-optional array element
lists. For example, `Item?[]{none, Item(), existing}` initializes an absent
slot, directly constructs one present payload, and conditionally copies the
ordinary source into another present payload. It does not default-construct
the array elements or assign over live placeholder values.

## Presence tests

Presence tests are explicit, non-failing boolean expressions:

```ska
if (value is some) {
    // value is present on this execution path
}

if (value is none) {
    // value is absent on this execution path
}
```

Both forms have exact type `bool`. They neither bind nor copy the payload.
Optionals do not have truthiness, so `if (value)` remains invalid.

A presence test does not change the declared type. `value` remains `T?`, and
payload member access still requires an explicit unwrap:

```ska
if (value is some) {
    value!.run();
}
```

Every unwrap is semantically checked. An implementation may remove a machine
check when it proves the optional remains present, but source validity and
failure behavior never depend on flow-sensitive narrowing.

## Checked unwrap

Postfix `!` performs checked access:

```ska
var count: i64 = maybe_count!;
item!.run();
var owner: shared Item = maybe_owner!;
```

The source is evaluated exactly once. If it is absent, execution terminates
unsuccessfully before producing a value or place. If it is present, the result
has the ordinary payload category:

- a primitive payload is copied as a primitive value;
- an optional shared owner secures one ordinary non-null `shared T`; and
- an inline class payload supplies an exact object source or a bounded checked
  payload place according to its immediate consumer.

There is no unchecked unwrap. Direct optional member access, method calls,
shared dereference, and shared member access are invalid:

```ska
item.member;       // invalid; use item!.member
item.run();        // invalid; use item!.run()
*maybe_owner;      // invalid; unwrap the owner first
maybe_owner->run(); // invalid; unwrap the owner first
```

An optional shared owner is unwrapped before the ordinary shared edge is
crossed:

```ska
(maybe_owner!)->run();
inspect(*(maybe_owner!));
```

Postfix unwrap composes with calls, `.`, and `->` in the postfix-expression
chain. It does not introduce optional chaining, propagation, or member
forwarding.

## Checked payload places

An inline class payload may be consumed as a non-owning place by field access,
a method receiver, an alias argument, a cast or type test, or another existing
object-place consumer. Such an unwrap creates a bounded checked payload view.

The view begins when the unwrap source is evaluated and ends after its complete
immediate consumer. It cannot be stored, returned as an alias, captured, or
otherwise escape that consumer.

For a receiver or alias argument, the payload remains present through later
left-to-right argument evaluation and the complete call:

```ska
inspect(value!);
value!.run(argument());
```

Nested and overlapping checked payload views are valid. The implementation
dynamically guards the optional's presence while any such view is active.

## Presence guards

A presence guard prevents the lifetime of a checked payload place from ending
inside its consumer. It is a dynamic lifetime guard, not an exclusive lock:

- ordinary field and method mutation of the still-present payload remains
  valid;
- another alias may inspect or mutate that payload under the existing
  non-exclusive alias rules; but
- clearing, replacing, or destroying the optional container while a payload
  view is active terminates unsuccessfully before changing presence.

For example, if `run` reaches the same optional through another alias,
attempting `value = none` during `value!.run()` terminates rather than
invalidating the active receiver.

A primitive unwrap finishes after copying its value. An optional shared-owner
unwrap secures an independent ordinary owner before later effects. Neither
requires a continuing presence guard after that copy completes.

When an unwrap participates in an implemented short-circuit logical expression, an
inline payload view and its presence guard still end after their complete
immediate consumer; they do not extend across a later logical operand. An
ordinary owner secured from `(shared T)?` is distinct: if it is a temporary, it
follows the selected path's full-expression lifetime. A skipped logical operand
performs no unwrap and establishes no view, guard, or owner.

Keeping the optional container's storage alive is separate from keeping its
payload present. If an optional field is reached through replaceable or
produced shared storage, the existing shared-owner anchor keeps the allocation
alive while the presence guard keeps the selected optional payload present.

## Assignment and lifecycle

Assignment evaluates and secures its complete source before changing the
destination. Optional-to-optional assignment follows this matrix:

| Destination before assignment | Source | Payload operation |
|---|---|---|
| absent | absent | None |
| absent | present | Initialize or copy-construct one payload |
| present | absent | Destroy or release the old payload |
| present | present | Perform ordinary payload assignment |

Direct assignment from a non-optional payload uses the corresponding present
source row. Shared-owner assignment secures a named incoming owner by copy or a
produced owner by its existing adopt/move rule before releasing any old owner.

Copy construction copies presence. It conditionally copy-constructs an inline
payload or retains a shared owner exactly once. Destruction conditionally
destroys the inline payload or releases the shared owner exactly once. Absent
payload bytes are never read, copied, assigned, or destroyed as a `T`.

Optional payload temporaries use the existing full-expression boundaries and
reverse cleanup order. Results are secured before checked views, anchors, and
other temporaries end.

Clearing, replacing, or destroying a presence-guarded optional terminates
before the transition. Payload guard-count overflow likewise terminates. These
failures never expose a dangling view or a half-completed presence transition.

## Aliases

`ref` and `mut ref` remain binding modes rather than reference types:

```ska
fn inspect(ref value: Item?) -> unit;
fn replace(mut ref value: Item?) -> unit;
```

Both aliases designate an always-present optional container. The `ref` binding
may test presence and perform read-only checked access. The `mut ref` binding
may additionally set, clear, or replace the container when no checked payload
view is active.

Neither spelling means an optional reference. Optional reference values,
`ref?`, local reference storage, and escaping aliases remain outside the
design.

## Shared ownership

`(shared T)?` is an optional value around ordinary shared ownership, not a
nullable form of `shared T`. When present, all existing shared invariants
apply: the handle is non-null, accounts for one strong owner, retains complete
dynamic metadata, and keeps one allocation alive.

Copy, assignment, argument, result, cast, field, and cleanup behavior lift the
existing shared-owner operations conditionally:

- absence accounts for no owner and performs no retain or release;
- copying presence performs one ordinary owner copy;
- replacing presence secures the incoming owner before releasing the old one;
- clearing or destruction releases one present owner; and
- unwrap secures a normal non-null `shared T` before it is used.

A compatible shared up-view may be lifted through optionality only when the
underlying `shared T` conversion is already valid. Optionality does not create
a new class, interface, or `Obj` relation.

The implementation may represent absent `(shared T)?` with a zero machine word.
That zero belongs only to the optional representation. It is never a
source-level null and is never passed to an operation requiring `shared T`.

## Containment

An inline `T?` reserves storage for a complete `T` payload even while absent.
It therefore contributes the same containment-layout edge as an inline `T`
field:

```ska
class Node {
    next: Node?; // invalid recursive inline containment
}
```

Optionality does not make recursive inline layout finite. An optional shared
edge does:

```ska
class Node {
    next: (shared Node)?;
}
```

The latter remains subject to ordinary shared-cycle behavior: a strong cycle
may intentionally keep its allocations alive.

## Overloads and compatibility

Expected optional types participate in ordinary initializer overload
selection:

- an exact non-optional match outranks injection into an optional;
- `none` admits only optional parameters;
- `none` contributes no payload-type specificity; and
- all remaining applicable candidates must still yield one unique
  most-specific initializer.

Optional compatibility lifts only an already valid payload or shared-target
conversion. There is no implicit unwrap, numeric conversion, truthiness,
optional cast, or user-defined optional conversion.

Optional types in virtual overrides and interface requirements are exact
signature components. Optional and non-optional parameters or results are not
interchangeable merely because injection is valid at an ordinary call site.

## Failure

The implemented profile has three unrecoverable source-level failure classes:

- unwrapping an absent value;
- overflowing a dynamic presence-guard count; and
- clearing, replacing, or destroying an optional while its payload is
  dynamically guarded.

Each failure terminates unsuccessfully without returning to Skald, producing
an invalid value, or guaranteeing remaining source-level cleanup. This is the
same language boundary as failed dynamic object casts, not a catchable
exception. The [common panic policy](ERRORS.md#frozen-panic-design) reports
all three through one reporter while preserving their distinct compiler-known
reasons.

Future recoverable exceptions must end active presence guards on every
exceptional edge before they can cross optional payload consumers.

## Declaration and ABI boundary

The completed profile permits optionals in internal locals, fields, value
parameters, results, assignments, temporaries, methods, interfaces, virtual
overrides, and initializer overloads. Alias parameters may designate supported
inline optional containers as described above.

The implemented [static-field contract](STATIC_FIELDS.md) separately permits
primitive and exact-class `T?` and every currently supported `(shared T)?`
target as class-owned static storage. An initializer-free container begins as
`none`; an explicit initializer uses the ordinary absent/present construction,
copy, adoption, publication, and cleanup rules before entry. Static optionals
perform ordinary conditional replacement while the program runs. On normal
entry return, exact-reverse static shutdown conditionally destroys a present
inline payload or releases a present owner. Abrupt termination remains
non-unwinding and does not guarantee static cleanup.

External declarations continue to reject every optional parameter and result.
No C representation, calling convention, ownership transfer, or foreign
lifetime contract is defined.

## Explicit exclusions

The implemented compositional profile does not yet execute the frozen shared
optional boxes above. Neither the implemented profile nor the frozen box
design includes:

- generalized boxes for non-optional primitive, class, array, function, or
  other inline values;
- mutable shared optional cells or whole-box-pointee assignment;
- optional function values;
- first-class or optional references;
- optional equality or lifted arithmetic;
- optional chaining, coalescing, or propagation;
- failed casts returning an optional;
- implicit member access or unwrap;
- concurrency or atomic presence-guard semantics;
- recoverable optional failures; or
- external optional ABI mappings.

These exclusions are not implied language behavior. Each requires a separate
focused design before implementation. Nested optionals are not on this
exclusion list: their owning lifecycle, checked access, aliases, and internal
callable boundaries execute. Inline optional arrays are implemented across
every supported owning, aggregate, internal callable, array-element, and
call-scoped alias position.

The implemented [array design](ARRAYS.md) permits existing optional
non-array element types to default to `none` inside arrays and extends
`shared?` shorthand for exact shared array targets. Inline optional arrays may
themselves be array elements; `shared? T[]` is optional shared ownership, not
an inline optional array.

Its implemented
[explicit element-list form](ARRAYS.md#explicit-element-list-construction)
also makes each optional element position an ordinary expected optional
initialization destination. `none` initializes absence, and an ordinary
payload or shared-owner expression uses the existing optional injection and
destination rules. Optional shared-owner slots retain named owners, adopt
produced owners, and represent absence with the ordinary zero niche.
An eligible ungrouped exact-class construction initializes a present payload
directly; named and otherwise materialized sources retain their existing
conditional copy behavior. The list adds no universal `none` type, implicit
unwrap, or inline optional-array payload. Nested optional elements use the
recursive lifecycle and the same expected-destination rules.
