# Shared Optional Boxes Design Proposal

Status: draft under review; every decision in the register remains open until
explicitly confirmed and promoted into the living language and compiler
contracts. Freezing this proposal must precede an implementation roadmap.

This proposal adds non-null shared allocations whose complete pointee is an
already-supported optional value. It gives the reserved `shared P?` type an
executable meaning as `Shared<Optional<P>>` and derives `shared? P?` as the
existing shorthand for `Optional<Shared<Optional<P>>>`.

The design is intentionally narrower than generalized boxing. It reuses
Skald's canonical optional identities and lifecycle plans, ordinary non-null
shared handles, checked pointee places, owner anchors, allocation header, and
last-owner release. It does not make every inline type boxable, weaken
ordinary shared handles to nullable pointers, or reinterpret an optional
shared owner as a box.

Freezing this proposal will not make either form executable. The
[status matrix](../language/STATUS.md) remains the sole authority for compiler
availability, and the [implemented grammar](../language/GRAMMAR.md) remains
the exact accepted syntax until implementation changes it.

## Intended outcome

The frozen design should provide:

- `shared P?` as a non-null owner of one shared allocation containing an exact
  optional `P?` wrapper;
- `(shared P?)?` as an optional owner of that box, with `shared? P?` as its
  exact source shorthand;
- arbitrary existing optional layers outside either an ordinary shared owner
  or an optional-box owner, without flattening any layer;
- explicit absent and value-initialized box construction;
- ordinary shared-owner copy, transfer, assignment, fields, calls, statics,
  array elements, anchors, and last-owner release;
- explicit dereference to observe, borrow, or unwrap the boxed optional
  wrapper while keeping its absent/present state immutable after publication;
- one allocation descriptor and finalizer per canonical optional target;
- ordinary class/interface/`Obj` polymorphism for optional object boxes while
  retaining exact targets for non-object optional boxes;
- reuse of the existing recursive optional lifecycle and checked-view guard
  machinery inside the allocation; and
- no public C runtime ABI extension.

## Current boundary and architectural evidence

Skald already parses both reserved forms compositionally:

```text
shared P?   = Shared<Optional<P>>
shared? P?  = Optional<Shared<Optional<P>>>
```

Resolution currently diagnoses an optional operand in `shared` before it can
enter `ResolvedSharedTarget`. The executable shared-target families are
class, interface, `Obj`, and array. Resolved, HIR, and MIR types otherwise
already carry shared targets explicitly, and ordinary owner storage is one
non-null handle independently of the target family.

The completed optional generalization supplies the difficult pointee
semantics this feature needs:

- every executable optional has one deterministic `OptionalTypeId`;
- its metadata selects exact storage, representation, initialization,
  injection, copy, assignment, destruction, presence, unwrap, checked-access,
  and boundary plans;
- nested optionals and optional arrays recursively execute those plans;
- checked inline access uses a guard stored with the wrapper;
- optional containers reached through shared storage already compose an owner
  anchor with a payload guard; and
- `Optional<Shared<T>>` already uses a one-word zero niche without allowing
  zero into ordinary shared-owner operations.

The existing MIR place model is also close to the required boundary.
`SharedPointee(owner)` denotes the complete payload retained by a live owner,
while `SharedAllocationPayload(allocation)` denotes unpublished payload
storage under construction. These roots currently feed object and array
operations, but they can carry an exact optional type without introducing a
reference type or exposing a machine address in HIR.

The main architectural mismatch is that several shared-target consumers still
assume every non-array target is an object view. Adding an optional target
therefore requires an exhaustive audit and explicit capability splits:
generic owner operations accept optional boxes, object dispatch and casts do
not, array operations accept only array targets, and optional-place operations
accept only optional targets. This is a meaningful cross-phase change, but it
fits the existing identity and lifecycle architecture without replacing it.

The sibling Niflheim2 draft uses `shared T?` for an optional shared handle.
That spelling is not reusable here: Skald has already frozen `(shared T)?` as
`Optional<Shared<T>>` and `shared T?` as the opposite composition. Niflheim is
therefore evidence for the use case, not for Skald's type identity or syntax.

## Design principles

1. **Composition remains literal.** `shared P?` owns an optional wrapper;
   `(shared P)?` optionally owns an ordinary `P` allocation. Neither spelling
   aliases or converts to the other.
2. **The handle remains non-null.** Inner absence belongs to allocation
   payload state, never to a plain `shared P?` handle.
3. **Object polymorphism remains first-class.** Optional object boxes preserve
   the up-views expected from ordinary shared objects because their complete
   allocation payload cannot be replaced after publication.
4. **Dereference crosses the ownership edge.** Presence tests, unwrap, and
   eligible aliases operate on `*box`, not implicitly on the owner handle.
5. **One optional implementation remains authoritative.** Box construction,
   access, and finalization invoke the existing plan for the canonical optional
   identity rather than adding primitive/class/array box families.
6. **Sharing preserves one published value.** Copying a box owner shares one
   immutable optional wrapper and, when present, one mutable contained object;
   allocating a new box creates independent wrapper and payload storage.
7. **Publication follows complete initialization.** No owner can observe an
   absent-by-accident or partially initialized allocation payload.
8. **Object metadata remains truthful.** A polymorphic box distinguishes its
   static object view from the exact dynamic class that selected payload
   layout, lifecycle, dispatch, and casts.
9. **The runtime remains minimal.** Allocation, finalization selection,
   optional state, guards, and ownership stay compiler-generated.
10. **The first box feature stays narrow.** General scalar/class boxes,
    user-defined box abstractions, weak ownership, and concurrency are separate
    designs.

## Decision register

The directions below are recommendations for review, not confirmed language
or implementation contracts.

| ID | Decision | Recommended direction | State |
|---|---|---|---|
| [SB1](#sb1--eligible-box-targets) | Eligible target | Accept exact value-optionals plus class/interface/`Obj` optional-box views | **Open** |
| [SB2](#sb2--type-identity-spelling-and-precedence) | Type composition | Preserve literal composition; canonicalize `shared? P?` to `(shared P?)?` | **Open** |
| [SB3](#sb3--construction-syntax-and-default-state) | Construction | Add `new P?()` for an absent box and `new P?(value)` for ordinary optional initialization | **Open** |
| [SB4](#sb4--owner-copying-and-box-value-semantics) | Copy meaning | Owner copies share a box; `new P?(source)` creates an independent box payload | **Open** |
| [SB5](#sb5--explicit-access-and-inner-unwrap) | Access | Require `*box`; do not forward `is`, `!`, `.`, or `->` through the handle | **Open** |
| [SB6](#sb6--immutable-boxed-optional-state) | Wrapper mutation | Forbid whole-pointee assignment; replace owner handles instead | **Open** |
| [SB7](#sb7--aliases-anchors-and-guards) | Borrow safety | Reuse shared anchors plus the optional wrapper's existing guards | **Open** |
| [SB8](#sb8--compatibility-casts-and-polymorphism) | Compatibility | Allow object-box up-views over one immutable fixed-target allocation | **Open** |
| [SB9](#sb9--stored-positions-and-default-array-elements) | Stored positions | Support ordinary shared-owner positions and distinct absent boxes for requested default array elements | **Open** |
| [SB10](#sb10--canonical-compiler-representation) | Type and IR representation | Separate the static box view from its exact optional allocation target | **Open** |
| [SB11](#sb11--allocation-layout-metadata-and-finalization) | Target realization | Reuse the non-null header with exact box metadata and an optional finalizer | **Open** |
| [SB12](#sb12--calling-convention-runtime-and-failure-boundary) | ABI and failure | Keep the one-word internal owner ABI, runtime ABI version 9, and current non-unwinding failures | **Open** |
| [SB13](#sb13--diagnostics-dumps-tests-and-promotion) | Quality and freeze | Freeze all rows, promote living contracts, then create an implementation roadmap | **Open** |

## Proposed source surface

### Distinct ownership and presence layers

```ska
var box: shared Item? = new Item?();
var maybe_box: shared? Item? = none;
var maybe_item_owner: shared? Item = none;
```

These declarations have three different types:

| Source type | Canonical model | Meaning |
|---|---|---|
| `shared Item?` | `Shared<Optional<Item>>` | One non-null owner of a box whose wrapper may be absent |
| `shared? Item?` | `Optional<Shared<Optional<Item>>>` | Zero or one owner of such a box |
| `shared? Item` | `Optional<Shared<Item>>` | Zero or one owner of an ordinary `Item` allocation |

For `shared? Item?`, outer and inner absence are independent:

```text
none                       no box owner
some(new Item?())          a box exists; its Item? payload is absent
some(new Item?(Item()))    a box exists; its Item? payload is present
```

Copying the outer optional when it is present retains the same box. It does
not allocate or copy the box payload.

Additional optional layers may wrap either owner composition:

```ska
var nested_owner: (shared Item)?? = some(none);
var nested_box_owner: (shared Item?)?? = some(none);
```

The first form is already supported: it is a nested optional containing an
ordinary shared `Item` owner at its deepest present layer. The second becomes
supported with this proposal: it is a nested optional containing a non-null
owner of an `Item?` box at its deepest present layer. Neither nesting is
flattened.

### Construction

The proposed forms are:

```ska
var empty: shared Item? = new Item?();
var also_empty: shared Item? = new Item?(none);
var present: shared Item? = new Item?(Item());
var explicit: shared Item?? = new Item??(some(none));
```

`new P?()` applies the existing default plan for the exact optional target and
therefore constructs outer absence. `new P?(expression)` checks its sole
expression as an ordinary owning initialization of exact type `P?`. Existing
rules consequently remain visible:

- `none` initializes the outer wrapper as absent;
- a value of type `P` performs one-layer optional injection;
- `some(expression)` explicitly constructs one present layer;
- a named `P?` source uses its available optional-copy plan; and
- a produced `P?` source may transfer its completed value under the existing
  produced-value rules.

The sole expression is a wrapper initializer, not an argument list forwarded
to `P`. For a class payload, users write `new Item?(Item(arguments))`; the
spelling `new Item?(arguments)` is accepted only when that one expression can
initialize `Item?`. More than one expression and `copy` construction mode are
not part of this proposal.

Grouped payload types compose without special syntax:

```ska
var owner_box: shared (shared Item)? = new (shared Item)?(new Item());
var array_box: shared i64[]? = new i64[]?(i64[]{1, 2});
```

The first allocation contains an optional shared owner. The second contains
an optional inline array. Neither is an object allocation.

### Observation, unwrap, and owner replacement

The shared edge must be crossed explicitly:

```ska
(*box)!.use_item();
box = new Item?();
```

`*box` denotes the existing `P?` container place in the allocation. Ordinary
consumers then reuse current optional behavior:

- `*box is some` and `*box is none` inspect the wrapper without copying it;
- `var copy: P? = *box` copies the wrapper when its payload capabilities allow;
- `(*box)!` checks and removes exactly one optional layer;
- for an exact value box, `inspect(*box)` may bind a `ref value: P?` or
  parameter; and
- `box = new P?(source)` replaces an owner variable with a distinct box while
  leaving the old allocation unchanged for its other owners.

Whole-pointee assignment is invalid for every box target:

```ska
var empty: shared Item? = new Item?(none);
*empty = Item(); // invalid: published box state is immutable
```

An object-box up-view retains dynamic dispatch after explicit dereference and
unwrap:

```ska
var box: shared Base? = new Derived?(Derived());
(*box)!.foo();
```

If `Base.foo` is virtual and `Derived` overrides it, this invokes
`Derived.foo`. The box owner anchors the allocation, `!` checks presence and
guards the payload, and the resulting `Base` view carries the box descriptor's
dynamic `Derived` metadata into ordinary virtual dispatch. If the box is
absent, unwrap terminates before dispatch. `box->foo()` remains invalid because
neither dereference nor optional unwrap is implicit.

The owner itself is not optional, so `box!` is invalid. Member forwarding is
also absent: `box.member`, `box->member`, and direct presence tests on `box`
do not implicitly dereference or unwrap. For an optional box owner, the two
layers remain explicit:

```ska
var maybe_box: shared? Item? = new Item?(Item());
(*(maybe_box!))!.use_item();
```

The first `!` secures a non-null box owner from the outer optional. `*` then
selects the boxed `Item?`, and the second `!` checks its inner presence.

## SB1 — Eligible box targets

**Question:** Which pointee types should ordinary `shared` gain through this
feature?

**Recommended direction:** Retain exact canonical `OptionalTypeId` targets for
primitive, array, shared-owner, and recursively nested value boxes. Treat an
optional object box separately as an exact class allocation with a compatible
static class, interface, or `Obj` box view, mirroring the target families of
ordinary shared objects.

This deliberately permits `shared P??`, `shared P[]?`, and
`shared (shared P)?` without separate box families. Eligibility is inherited
from the complete optional identity for these value targets; the box layer
does not repair an otherwise invalid primitive, array, or nested optional.

For object boxes, only a concrete class may be named by `new`, but owners may
up-view that allocation through a base class, conformed interface, or `Obj`:

```ska
var base_box: shared Base? = new Derived?();
var interface_box: shared Drawable? = new Derived?();
var object_box: shared Obj? = new Derived?();
```

Bare `Drawable?` and `Obj?` remain invalid owning inline values. Their use as
shared box targets denotes a presence-bearing object *view* into an allocation
whose descriptor retains one exact dynamic class and one concrete optional
payload layout. As with ordinary `shared Drawable` and `shared Obj`, the
shared form need not imply that the corresponding bare target is an owning
inline storage type.

General `shared i64`, `shared InlineClass`, and other non-optional boxes remain
outside this proposal. They would require a broader source rationale and
would make class allocation syntax overlap with box allocation without being
needed to implement the reserved forms.

## SB2 — Type identity, spelling, and precedence

**Question:** How should the two `?` positions compose and render?

**Recommended direction:** Preserve the implemented type grammar exactly:
ordinary leading `shared` consumes its complete following inline type, while
`shared? X` expands to `(shared X)?` before semantic normalization.

| Source spelling | Canonical type | Relationship |
|---|---|---|
| `shared P?` | `Shared<Optional<P>>` | Non-null optional box |
| `(shared P)?` | `Optional<Shared<P>>` | Existing optional ordinary owner |
| `shared? P` | `Optional<Shared<P>>` | Existing exact shorthand |
| `shared P??` | `Shared<Optional<Optional<P>>>` | Box containing a nested optional |
| `(shared P?)?` | `Optional<Shared<Optional<P>>>` | Canonical optional box owner |
| `shared? P?` | `Optional<Shared<Optional<P>>>` | Exact shorthand for `(shared P?)?` |
| `(shared P)??` | `Optional<Optional<Shared<P>>>` | Existing nested optional around an ordinary owner |
| `(shared P?)??` | `Optional<Optional<Shared<Optional<P>>>>` | Nested optional around a box owner |
| `(shared? P?)?` | `Optional<Optional<Shared<Optional<P>>>>` | Shorthand-containing equivalent of `(shared P?)??` |
| `shared P?[]` | `Shared<Array<Optional<P>>>` | Existing shared outer array |
| `(shared P?)[]` | `Array<Shared<Optional<P>>>` | Inline array of non-null boxes |
| `shared P[]?` | `Shared<Optional<Array<P>>>` | Box containing an optional inline array |
| `shared? P[]` | `Optional<Shared<Array<P>>>` | Existing optional shared array owner |

Semantic dumps should render canonical `(shared P?)?`, while syntax dumps
retain `shared? P?` shorthand provenance. The alias creates no conversion,
overload distinction, layout distinction, or second identity.

Postfix optionality outside a grouped owner remains arbitrarily compositional
within the ordinary syntax nesting budget. Thus `(shared P)???...` already
uses the implemented recursive optional identities, and
`(shared P?)???...` does the same once `Shared<Optional<P>>` becomes an
eligible payload. Every outer layer remains independently absent or present.

Grouping is semantically significant. In particular:

```text
(shared P)??   = Optional<Optional<Shared<P>>>
shared P??     = Shared<Optional<Optional<P>>>

(shared P?)??  = Optional<Optional<Shared<Optional<P>>>>
shared? P??    = Optional<Shared<Optional<Optional<P>>>>
shared P???    = Shared<Optional<Optional<Optional<P>>>>
```

Those five types are distinct. The `shared?` shorthand contributes exactly
one optional layer outside the complete shared target; additional outer
layers require grouping, as in `(shared? P?)?`.

## SB3 — Construction syntax and default state

**Question:** How is a new optional box allocated and initialized?

**Recommended direction:** Extend `new` so its complete target type selects
the construction family:

```text
new C(arguments)       exact class allocation
new A[](initializer)   shared outer-array allocation
new P?()               absent optional-box allocation
new P?(expression)     value-initialized optional-box allocation
```

The parser should retain one complete grouped/postfix type target for the box
form and require that its outer constructor is optional. This makes
`new P?[](...)` an array construction and `new P[]?(...)` a box construction,
matching the type precedence rather than using look-ahead accidents.

For the expression form, evaluation and initialization should follow the
existing `new` and destination-construction disciplines:

1. evaluate the initializer exactly once and establish required source
   temporaries, anchors, checked views, and selected construction arguments;
2. allocate checked storage for the exact optional target;
3. initialize the optional wrapper through its selected absent, injection,
   copy, transfer, or direct-payload plan;
4. publish only after the complete wrapper is initialized; and
5. adopt the produced count-one owner into its consumer.

Failure-capable source checks occur before destination allocation where the
existing source plan permits. Direct destination construction remains
available; the form must not impose an extra source-visible class copy merely
because the destination is boxed. Abrupt failure remains non-unwinding under
the current language contract.

For an optional object box, the class named after `new` is the exact dynamic
box target. Its initializer is checked against that exact target before any
later up-view of the produced owner:

```ska
new Derived?(Derived()) // valid exact payload construction
new Derived?(none)      // valid absent Derived box
new Derived?(Base())    // compile-time error: exact Base cannot supply Derived
```

A source object view whose dynamic class could be `Derived` may use the
existing target-directed checked-copy discipline: perform the dynamic check
before allocating the box, reject a statically impossible relation, and then
construct exact `Derived` payload storage. The allocation target is known
exactly from source syntax, and the completed wrapper is never mutated after
publication.

The zero-expression form is recommended because every eligible optional has
an unambiguous absent default and because it supplies the default plan needed
for nonempty arrays of non-null box owners. Requiring `new P?(none)` instead is
a viable narrower alternative if explicit wrapper initialization is preferred
over that symmetry.

## SB4 — Owner copying and box value semantics

**Question:** Does copying `shared P?` copy the optional payload or only the
owner?

**Recommended direction:** Reuse ordinary shared-owner semantics. A named
owner copy retains the allocation, a produced owner transfers, assignment
secures before release, and the last owner finalizes the one boxed wrapper.

```ska
var first: shared Item? = new Item?(Item());
var alias: shared Item? = first;
(*alias)!.mutate();
// Both owners reach the same present Item object.

var independent: shared Item? = new Item?(*first);
(*independent)!.mutate();
// This allocation contains a copied, independent Item payload.
```

This distinction is the purpose of the feature. No implicit operation combines
owner copying with payload copying, and no box copy-on-write behavior is
introduced.

## SB5 — Explicit access and inner unwrap

**Question:** Should optional operations be forwarded through a box owner?

**Recommended direction:** No. Prefix `*` produces the optional pointee place
or polymorphic optional-object view selected by the box target. Presence,
unwrap, copying, aliases, and contained-object access then apply explicitly at
that boundary.

This keeps `shared? P?` unambiguous: postfix `!` always removes the outer
optional-owner layer, while `(*owner)!` removes the inner boxed optional layer.
It also preserves the established separation between handle consumers and
pointee consumers. `->` remains an object/array operation and does not acquire
optional forwarding behavior.

Reading an exact `*box` as an owning value requires the optional target's
ordinary copy capability. Reading a polymorphic object box into an exact class
optional uses the target-directed slicing rule proposed in SB8. Presence tests
and eligible checked-view consumers do not needlessly copy the complete
wrapper.

## SB6 — Immutable boxed optional state

**Question:** Should `*box = source` be supported even though whole-pointee
assignment is rejected for current object and array owners?

**Revised recommended direction:** No. Construction fixes the complete
absent-or-present optional state for the allocation's lifetime. Prefix `*`
exposes that wrapper for observation and checked payload access, not as a
whole-value assignment destination.

```ska
var box: shared Base? = new Derived?(none);
*box = Derived();                 // compile-time error
box = new Derived?(Derived());    // valid owner replacement
```

The final statement evaluates and secures a distinct produced owner, releases
the variable's old owner, and installs the new handle through ordinary shared
assignment. It does not alter the old allocation. Any other owners of the old
absent box continue to observe absence until they are released.

Immutability is shallow in the same sense as ordinary shared ownership. A
present wrapper never changes presence or replaces its complete contained
object, but mutable fields and methods of that contained object remain usable:

```ska
var box: shared Base? = new Derived?(Derived());
(*box)!.mutate(); // valid mutation within the fixed Derived object
*box = none;      // invalid replacement of the complete optional wrapper
```

This rule applies equally to primitive, object, array, shared-owner, and nested
optional box targets. For a nested optional target, every wrapper layer and
the deepest stored value are fixed at publication; only mutation within an
already-present contained object or other mutable aggregate remains available.

The rule exactly matches current ordinary shared-object and shared-array
ownership: owners may be copied, transferred, cast, or repointed to another
allocation, while `*owner = replacement` remains invalid. It also removes the
covariant mutable-container conflict from SB8, so object-box up-views need no
checked store, exact-type update capability, dynamic assignment thunk, or new
runtime failure.

## SB7 — Aliases, anchors, and guards

**Question:** How are non-owning uses protected when boxes have multiple
owners and immutable wrapper state?

**Recommended direction:** Compose the two mechanisms already implemented:

- the shared owner or a hidden copied/adopted anchor keeps the allocation live;
- the optional state word's existing guard keeps a checked present payload
  valid through its immediate consumer; and
- both lifetimes end only after the complete immediate consumer secures its
  result.

A stable local or value-parameter owner can cover an immediate use directly.
A replaceable owner field is copied before exposing its pointee. A produced
box owner remains an owning full-expression temporary. Unwrapping an outer
`(shared P?)?` first creates an ordinary secured box owner, which then serves
as the inner allocation anchor.

For an exact value box, `inspect(*box)` may bind a read-only
`ref value: P?` parameter. A `mut ref value: P?` consumer is rejected because
it could replace the published wrapper. Whole-wrapper aliases from a
polymorphic object-box view remain excluded because the current alias ABI
carries only an exact raw optional address. By contrast,
`inspect((*box)!)` can borrow the present object payload with the ordinary
access supported by that object view, under both the owner anchor and a
checked optional guard.

The owner anchor ensures last-owner finalization cannot run while a checked
view remains live. Since no source operation can replace the wrapper, a guard
has no competing box-mutation path; retaining the existing guard protocol is
still a simple way to reuse optional checked access and may later be optimized
only when lifetime and failure behavior remain unchanged. Atomic guards,
thread safety, and data-race prevention remain excluded.

Aliases whose designated type is itself a shared owner remain unsupported;
users pass `shared P?` by value and explicitly dereference it when a read-only
`ref P?` consumer is intended.

## SB8 — Compatibility, casts, and polymorphism

**Question:** Can boxes participate in the existing class/interface/`Obj`
shared compatibility relation?

**Revised recommended direction:** Yes for optional object boxes. Preserve one
fixed exact dynamic box class in allocation metadata, allow the same
class/base/interface/`Obj` static up-views as ordinary shared objects. The
complete optional wrapper is immutable after publication, so these views
cannot replace it. Non-object optional box targets remain exact and invariant.

The motivating behavior is therefore valid:

```ska
var foo1: shared Base = new Derived();
var foo2: shared Base? = new Derived?();
```

Both allocations retain dynamic `Derived` identity. For `foo2`, the descriptor
also fixes `Optional<Derived>` as the physical payload and lifecycle target
even while absent. Unwrapping through the `shared Base?` view yields a guarded
`Base` object view into the complete `Derived`, so base fields, virtual calls,
interface dispatch, `Obj` use, type tests, and checked box downcasts can follow
the ordinary shared-object model without slicing the allocation.

Accordingly, a present construction and virtual call behave as follows:

```ska
var box: shared Base? = new Derived?(Derived());
(*box)!.foo(); // dispatches to Derived.foo when it overrides virtual Base.foo
```

The static method family comes from the `Base` view. The retained dynamic box
class selects the `Derived` override exactly as an ordinary
`shared Base = new Derived()` owner would.

Interface views work the same way:

```ska
// Implementation implements Interface.
var box: shared Interface? =
    new Implementation?(Implementation());
(*box)!.foo();
```

The static call is accepted through `Interface.foo`, while the allocation
descriptor retains exact dynamic class `Implementation` and selects its
implementation at dispatch. An absent `Implementation?` box may use the same
`shared Interface?` view, but `!` then fails before method dispatch.

Covariance is sound because no view can replace the complete optional payload:

```ska
var derived_box: shared Derived? = new Derived?(Derived());
var base_box: shared Base? = derived_box;
*base_box = Base();       // compile-time error
*derived_box = Derived(); // also a compile-time error
```

These are not dynamically checked stores. Whole-pointee assignment is absent
from the box capability set even through an exact view. Owner replacement is
different and remains ordinary shared assignment:

```ska
var box: shared Base? = new Derived?();
box = new Base?(Base()); // valid: replace the owner, not the old box payload
```

The right side produces a distinct exact Base box compatible with the
variable's static view. Assignment secures that new owner and releases the
variable's old Derived-box owner. Other owners of the old box, if any, still
observe the unchanged old allocation.

Construction also differs because its dynamic target is statically known:

```ska
var invalid: shared Base? = new Derived?(Base()); // compile-time error
```

The outer `shared Base?` expectation permits the produced Derived box up-view,
but it does not change the exact `Derived?` payload demanded by
`new Derived?(...)`.

The allocation descriptor retains exact dynamic box class `D` for the
allocation's complete lifetime, including while its wrapper is absent. An
absent `Derived?` box therefore remains absent forever; a variable, field, or
array element that must become present receives a newly allocated box owner.
A present box retains the same complete `Derived` object, though ordinary
mutable fields and methods may still change that object's internal state.

This deliberately does not provide a shared mutable optional cell. If that
abstraction is later needed, it should be a separate mutable-cell feature with
an explicit synchronization, variance, aliasing, and failure design rather
than an exceptional write capability attached to `shared P?`.

Checked unwrap of the object payload is much cleaner: it already has the
owner anchor, presence guard, complete-object address, static view, and dynamic
metadata needed by existing object consumers. Copying the complete wrapper
into an inline `Base?` destination deliberately slices a present dynamic
payload to exact Base under the ordinary target-directed copy rules. An
interface or `Obj` box view cannot be copied to a bare inline optional because
those owning types remain invalid.

Owner-preserving box casts should mirror object-owner casts: up-views are
static; possible downcasts check the descriptor's exact box class; impossible
relations are rejected; and no cast allocates, copies the optional payload, or
changes presence. Immutability means every compatible object-box owner has the
same non-replacing capability, so an exact view does not expose a surprising
extra update operation that disappears after an up-view.

Polymorphism may also exist *inside* an exact optional shared-owner payload.
This remains useful but is not a substitute for direct optional-box
polymorphism:

```ska
var maybe_owner: (shared Base)? = new Derived();
var owner_box: shared (shared Base)? =
    new (shared Base)?(new Derived());
```

The first value is an optional owner with no box. The second is an exact box
whose immutable optional payload contains an ordinary polymorphic owner. Both
preserve the dynamic `Derived` allocation behind the inner `shared Base` view,
but neither spelling is required merely to obtain `shared Base?` from
`new Derived?()` under the revised direction.

## SB9 — Stored positions and default array elements

**Question:** Where may box owners be stored, and are they defaultable as
array elements?

**Recommended direction:** Treat `shared P?` as an ordinary non-null shared
owner in locals, fields, internal value parameters/results, explicitly
initialized statics, temporaries, and array elements. It remains invalid in
external signatures. `(shared P?)?` additionally reuses the existing absent
zero default for optional-owner statics and elements.

A requested nonempty `(shared P?)[]` default construction should allocate one
distinct absent box per element, equivalent in payload state to repeated
`new P?()` but using the array construction's existing generated default
protocol. Slots must not share one synthesized box. This matches current
non-null shared element behavior and makes the element type genuinely
defaultable.

This remains distinct from `shared P?[]`, which is a shared outer array whose
inline elements are `P?` wrappers and therefore performs only one outer shared
allocation.

## SB10 — Canonical compiler representation

**Question:** Does a shared optional box need a new canonical identity family?

**Revised recommended direction:** Distinguish a box's static view identity
from its exact allocation target. Exact non-object boxes may continue to name
one canonical `OptionalTypeId` directly. Polymorphic object boxes need a
canonical static box-view identity whose optional depth and class/interface/
`Obj` leaf can differ from the allocation descriptor's exact class optional
identity.

Conceptually, the shared target families become:

```text
Object(class | interface | Obj)
Array(ArrayTypeId)
ExactOptional(OptionalTypeId)
OptionalObjectView(optional depth, class | interface | Obj)
```

The exact Rust shape remains open. A `SharedBoxViewId` or equivalent becomes
justified if it interns information not present in `OptionalTypeId`, especially
interface/`Obj` leaves and deterministic compatibility across nested optional
layers. It must not duplicate exact primitive, array, shared-owner, or other
invariant optional targets merely for naming symmetry.

Object-only conversions must stop accepting a generic shared target and then
assuming a view target exists. Instead, shared-target capabilities should be
queried explicitly:

```text
owner operations       class | interface | Obj | array | exact optional | optional object view
object views/dispatch  class | interface | Obj | present optional object view
array operations       array
optional-place access  exact optional | optional object view
```

HIR should add a typed optional-box allocation producer and a checked shared
optional-pointee place carrying owner provenance, access, static box view,
known exact allocation target when available, span, and anchor strategy.
Exact boxes should reuse the current typed optional initialization and
copy plans. Polymorphic object boxes additionally need explicit static-view
projection, descriptor checks for casts and dispatch, and checked present
access rather than asking MIR or the backend to infer them from a generic
expression. No published optional-box pointee has an assignment plan.

MIR should keep the allocation state transition explicit:

```text
allocated optional storage
    -> initialized complete P? wrapper
    -> published count-one box
    -> adopted ordinary owner
```

The allocation must carry a distinct optional-box source origin, as required
by the existing auditable allocation-origin policy. Generic owner copy,
adopt/move, release, call, field, static, and temporary instructions can then
accept the extended target. Existing exact optional MIR operations should act
on `SharedAllocationPayload` during construction and
`SharedPointee(owner)` after publication. Object-box view, cast, unwrap, copy,
and dispatch operations must retain both the static view and exact dynamic
descriptor dependency through verification.

## SB11 — Allocation layout, metadata, and finalization

**Question:** How does the backend distinguish a box from an object while
retaining the generic last-owner path?

**Recommended direction:** Keep one non-null handle pointing to the existing
16-byte allocation header and interpret its metadata slot as a non-null
allocation descriptor:

| Offset | Box allocation field | Representation |
|---:|---|---|
| 0 | strong count | `u64` |
| 8 | allocation metadata | pointer to exact optional-box descriptor |
| 16 | boxed payload | existing layout of canonical `P?` |

Current x86-64 optional alignments fit the existing payload offset. Layout
calculation must nevertheless use the target data-layout owner and reject
size, alignment, or addressability overflow rather than assuming every future
optional remains eight-aligned.

Each referenced exact optional box target receives one deterministic metadata
record and one compatible finalizer entry. The finalizer accepts the complete
optional payload address and invokes the existing destruction plan for that
`OptionalTypeId`: it does nothing for absence, conditionally destroys an
inline payload, recursively cleans nested/array payloads, or releases a
present inner shared owner. The generic last-release path then frees the exact
header once.

An optional object-box descriptor additionally retains the exact dynamic
class descriptor and static-view membership evidence selected by SB8.
Presence state and the complete exact-class payload remain inline after the
box header; polymorphism does not require a second source-visible owner or a
second payload allocation. Object dispatch and type tests consume the retained
class metadata only after a successful present check and guard. Construction
uses the exact optional target known at the allocation site; no dynamic
assignment thunk is needed because the published wrapper is immutable.

Primitive-only boxes may share a generated no-op finalizer implementation as
an optimization only if metadata identity, deterministic output, and the
generic release contract remain correct.

`shared? P?` retains the existing outer optional-owner layout: zero means no
box owner, and nonzero is the ordinary box header handle. Inner optional state
lives at payload offset 16 and is independent of that outer zero niche.

## SB12 — Calling convention, runtime, and failure boundary

**Question:** Does a new target family require a new owner ABI or runtime
service?

**Recommended direction:** No. `shared P?` is one integer-class owner word in
internal parameters and results, just like every ordinary shared owner.
`(shared P?)?` is the existing nullable-owner word. The boxed `P?` aggregate
itself crosses a callable boundary only through its existing value or alias
convention after explicit dereference.

Checked byte allocation and exact-base deallocation already suffice. Strong
counting, metadata publication, optional initialization, guarded access,
finalizer generation, and target checking remain compiler-owned, so the public
runtime ABI remains version 9.

Allocation failure, size overflow where handled at runtime, optional unwrap
failure, and guard overflow retain unrecoverable, non-unwinding behavior. This
feature adds no store-related runtime failure. Statically knowable layout
overflow remains a backend error. Invalid metadata, zero ordinary handles,
double finalization, and ownership underflow remain compiler/runtime defects.

## SB13 — Diagnostics, dumps, tests, and promotion

**Question:** What evidence is required before the design may be frozen and
then implemented?

**Recommended direction:** Preserve exact `shared`, shorthand `?`, target
`?`, grouping, `new`, dereference, assignment, and initializer spans in the
source-shaped phases. Semantic dumps use canonical type spelling and
deterministic optional identity order. Diagnostics should distinguish invalid
optional payloads, unavailable wrapper lifecycle operations, box construction
arity, incompatible invariant targets, impossible object-box relations,
and attempted whole-pointee assignment without freezing prose wording.

The eventual implementation needs focused evidence for:

- parsing, grouping, precedence, shorthand provenance, recovery, and canonical
  resolved/HIR/MIR dumps;
- exact target and polymorphic box-view identities across modules and
  arbitrarily nested optional targets, including arbitrary layers outside
  grouped ordinary and box owners;
- absent, injected, `some`, copied, produced, nested, optional-array, and
  optional-shared-owner box construction;
- owner copy versus independent box allocation and deterministic last-owner
  finalization;
- class/base/interface/`Obj` box up-views, checked downcasts, virtual and
  interface dispatch, and fixed dynamic class identity while absent or
  present;
- owner replacement across compatible static box views, other owners retaining
  the old allocation, compile-time rejection of statically impossible exact
  box construction, and rejection of whole-pointee assignment through both
  exact and polymorphic views;
- presence tests, primitive extraction, inline checked views, optional-array
  access, read-only aliases, and contained-object mutation;
- direct and indirect owner self-assignment and mutation of a contained object
  through distinct owners of one allocation;
- owner anchors plus guard conflicts through locals, fields, produced owners,
  and optional box owners;
- fields, internal arguments/results, statics, arrays, element lists, and
  distinct default-created box elements;
- rejected bare owning `Obj?`/interface optionals, unit targets, impossible box
  casts, invariant non-object target conversions, implicit forwarding, invalid
  construction arity, unavailable construction/copy lifecycle, external
  signatures, mutable whole-wrapper aliases, and whole-pointee assignment;
- malformed MIR for target confusion, pre-publication access, owner loss,
  guard/anchor imbalance, wrong metadata/finalizer identity, and duplicate
  cleanup;
- x86-64 layout, calling-convention pressure, allocation failure, layout
  overflow, assembly acceptance, native lifecycle traces, and runtime ABI
  stability; and
- deterministic outputs across processes plus the repository `make check` and
  supported-toolchain gates.

All decisions in this document should be confirmed together. Promotion then
updates the living grammar, optional, shared-ownership, phase/IR, backend,
runtime, status, and testing contracts while still describing the feature as
not implemented. Only after that contract change is validated should a
PR-sized implementation roadmap be created.

## Explicit exclusions

This proposal does not include:

- generalized boxes for non-optional primitive, class, array, function, or
  other inline values;
- bare owning optional `Obj` or interface values; their use as polymorphic
  shared box views does not create an inline value type;
- implicit owner-to-pointee dereference, presence forwarding, optional
  chaining, coalescing, or propagation;
- covariance for primitive, array, nested value, or other non-object box
  targets;
- whole-pointee assignment for optional boxes or existing object and array
  shared targets;
- aliases whose designated type is a shared owner, first-class references,
  optional references, mutable whole-wrapper aliases, or escaping pointers
  into a box;
- external shared-box signatures or a stable public box ABI;
- weak owners, cycle collection, explicit early release, uniqueness, or
  copy-on-write;
- atomic strong counts or optional guards, threads, or data-race safety;
- generalized placement allocation, custom allocators, or user-defined
  finalizer metadata;
- recoverable allocation/unwrap/guard failure or exceptional cleanup; and
- generics or standard-library collection redesign.

Strong cycles involving box payloads retain the existing specified leak
behavior. A later generalized-box proposal may reuse the allocation descriptor
and pointee-place work, but it must make its own source, construction,
compatibility, and lifecycle decisions.

## Alternatives not recommended

### Treat `shared P?` as another optional owner spelling

This contradicts the implemented compositional grammar, collapses
`Shared<Optional<P>>` into `Optional<Shared<P>>`, and makes
`shared? P?` impossible to interpret consistently. `(shared P)?` already has
the optional-owner role.

### Use a nullable handle for `shared P?`

This would erase the distinction between no box and a present box whose
payload is absent, weaken every plain shared-owner invariant, and send zero
into operations that currently require non-null handles.

### Add mutable whole-wrapper assignment

Allowing `*box = source` would turn the feature into a shared mutable optional
cell. Polymorphic object-box views would then require invariant targets, a
distinct replace-capable owner/view qualifier, or runtime-checked covariant
stores. Each choice adds a capability discontinuity, new failure behavior, or
both. Immutable wrapper state instead matches ordinary shared-object semantics
and leaves a mutable cell as a separate future abstraction.

### Keep every box target invariant

Exact `OptionalTypeId` equality would simplify representation, but it prevents
`new Derived?()` from satisfying `shared Base?` and removes a central property
users expect from shared objects. Requiring an inner optional shared owner
restores polymorphism only by changing the value model and adding
source-visible indirection. Because the published wrapper is immutable, the
revised SB8 direction can provide direct object-box up-views without a checked
replacement path.

### Forward optional operations through the owner

Allowing `box!` or `box is none` to inspect the pointee would conflict with the
outer optional layer of `shared? P?` and hide allocation anchoring. Explicit
`*` makes each lifetime and failure boundary visible.

### Introduce one Rust enum family per payload category

Primitive, class, array, shared-owner, and nested box variants would duplicate
the canonical optional table and grow combinatorially. The existing
`OptionalTypeId` already owns exactly the information a box finalizer and
checked-access operation need.
