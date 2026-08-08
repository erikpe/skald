# Optional Values

Status: implemented language contract for primitive, exact inline-class, and
optional shared-owner values across owning locals, fields, internal callable
boundaries, and aliases to supported inline optional containers. The
[status matrix](STATUS.md) is authoritative for availability, and
the [implemented grammar](GRAMMAR.md) remains the exact syntax currently
accepted by the compiler.

This document defines Skald's source-level optional-value contract. Primitive
`i64?`, `u64?`, `u8?`, `f64?`, and `bool?` values and exact inline class `T?`
values now execute end to end in
owning locals, fields, internal value parameters/results, methods, interfaces,
virtual overrides, and initializer overloads. Inline class payload access
through postfix `!` executes as a bounded checked view. Optional shared owners
execute through the same internal owning positions and secure a normal
non-null owner on unwrap. Read-only and mutable aliases may designate
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

An optional shared owner `shared? T` likewise has exactly two states:

- **absent**, accounting for no strong owner; or
- **present**, containing one ordinary non-null `shared T` owner.

There is no null, invalid, dangling, moved-from, partially initialized, or
partially destroyed value of type `T`. When an optional is absent, there is no
`T` value to access.

## Type forms

The implemented profile separates optional inline payloads from optional shared
ownership:

| Type | Meaning | Frozen profile |
|---|---|---|
| `T` | Always-present inline `T` | Existing contract |
| primitive `T?` | Inline optional containing zero or one primitive `T` | Owning locals, fields, and internal callable boundaries execute |
| class `T?` | Inline optional containing zero or one exact class `T` | Owning lifecycle, internal boundaries, and bounded checked payload access execute |
| `shared T` | Always-present non-null shared owner of `T` | Existing contract |
| `shared? T` | Optional containing zero or one `shared T` owner | Internal owning lifecycle and checked unwrap execute |
| `shared T?` | Non-null shared box containing `T?` | Reserved and rejected |
| `shared? T?` | Optional owner of a non-null shared box containing `T?` | Reserved and rejected |

`shared?` is the contextual word `shared` followed by the `?` punctuation
token. Ordinary trivia may separate those tokens, although documentation and
dumps use `shared? T` as the canonical spelling.

Inline `T?` is valid when `T` is a primitive or exact inline class type.
`shared? T` accepts the same class, interface, and `Obj` targets as ordinary
`shared T`.

The first profile rejects:

- `unit?`;
- standalone optional interface or `Obj` views;
- nested `T??`;
- optional array and function types;
- `shared T?` and `shared? T?`;
- `ref?` and `mut ref?`; and
- every optional external parameter or result.

These exclusions are deliberate. In particular, `shared T?` requires a
generalized non-null shared box whose allocation, payload metadata, mutation,
and finalization are separate from optional ownership. This design reserves
the spelling without defining or implementing that box.

## Empty and present values

`none` is the reserved empty-optional expression. It receives its exact type
from one unambiguous expected optional boundary:

```ska
var inline_value: Item? = none;
var shared_owner: shared? Item = none;
```

The expected type may come from a local or field initialization, assignment,
argument, return, or initializer candidate. The implemented profile supplies
all of those boundaries for every supported optional. `none` used without
one unambiguous optional expectation is invalid. It does not have a universal
runtime type.

An ordinary value may be injected into its corresponding optional:

```ska
var inline_value: Item? = Item();
var shared_owner: shared? Item = new Item();
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
ordinary owner secured from `shared? T` is distinct: if it is a temporary, it
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

`shared? T` is an optional value around ordinary shared ownership, not a
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

The implementation may represent absent `shared? T` with a zero machine word.
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
    next: shared? Node;
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

The frozen profile adds three unrecoverable source-level failure classes:

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

The implemented [zero-default static-field contract](STATIC_FIELDS.md)
separately permits primitive and exact-class `T?` and every currently
supported `shared? T` target as class-owned static storage. Such a container
begins as `none`, remains initialized for process lifetime, performs the
ordinary conditional replacement operations while the program runs, and does
not clean a final present payload or owner at process exit. That lifetime rule
is defined authoritatively by the static-field contract and does not change
the local, instance-field, or callable-boundary optional contract.

External declarations continue to reject every optional parameter and result.
No C representation, calling convention, ownership transfer, or foreign
lifetime contract is frozen.

## Explicit exclusions

The frozen profile does not include:

- generalized `shared T?` boxes or `shared? T?`;
- inline optional array payloads;
- nested optionals;
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
focused design before implementation.

The frozen [array design](ARRAYS.md) separately permits existing optional
non-array element types to default to `none` inside arrays and extends
`shared?` to exact shared array targets. It continues to exclude inline
optional array payloads; `shared? T[]` is optional shared ownership, not an
inline optional array.

Its implemented
[explicit element-list form](ARRAYS.md#frozen-explicit-element-list-construction)
also makes each optional element position an ordinary expected optional
initialization destination. `none` initializes absence, and an ordinary
payload or shared-owner expression uses the existing optional injection and
destination rules. Optional shared-owner slots retain named owners, adopt
produced owners, and represent absence with the ordinary zero niche.
An eligible ungrouped exact-class construction initializes a present payload
directly; named and otherwise materialized sources retain their existing
conditional copy behavior. The list adds no universal `none` type, implicit
unwrap, nested optional, or inline optional-array payload.
