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
- explicit dereference to observe, borrow, unwrap, or mutate the boxed
  optional wrapper;
- one allocation descriptor and finalizer per canonical optional target;
- exact target identity with no box covariance, object up-view, or dynamic
  cast;
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
3. **The box target is exact.** A mutable box cannot safely gain covariance
   merely because its present payload names related classes.
4. **Dereference crosses the ownership edge.** Presence tests, unwrap, aliases,
   and replacement operate on `*box`, not implicitly on the owner handle.
5. **One optional implementation remains authoritative.** Box construction,
   mutation, and finalization invoke the existing plan for the canonical
   optional identity rather than adding primitive/class/array box families.
6. **Sharing is observable through mutation.** Copying a box owner shares one
   optional wrapper; allocating a new box creates independent wrapper state.
7. **Publication follows complete initialization.** No owner can observe an
   absent-by-accident or partially initialized allocation payload.
8. **Object metadata is not forged.** A box allocation never enters object
   dispatch, object casts, class tests, or interface witness lookup.
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
| [SB1](#sb1--eligible-box-targets) | Eligible target | Accept exactly an already-executable canonical optional type | **Open** |
| [SB2](#sb2--type-identity-spelling-and-precedence) | Type composition | Preserve literal composition; canonicalize `shared? P?` to `(shared P?)?` | **Open** |
| [SB3](#sb3--construction-syntax-and-default-state) | Construction | Add `new P?()` for an absent box and `new P?(value)` for ordinary optional initialization | **Open** |
| [SB4](#sb4--owner-copying-and-box-value-semantics) | Copy meaning | Owner copies share a box; `new P?(source)` creates an independent box payload | **Open** |
| [SB5](#sb5--explicit-access-and-inner-unwrap) | Access | Require `*box`; do not forward `is`, `!`, `.`, or `->` through the handle | **Open** |
| [SB6](#sb6--boxed-optional-replacement) | Mutation | Permit whole-pointee assignment only when the shared target is optional | **Open** |
| [SB7](#sb7--aliases-anchors-and-guards) | Borrow safety | Reuse shared anchors plus the optional wrapper's existing guards | **Open** |
| [SB8](#sb8--compatibility-casts-and-polymorphism) | Compatibility | Require exact optional target identity and exclude box casts/up-views | **Open** |
| [SB9](#sb9--stored-positions-and-default-array-elements) | Stored positions | Support ordinary shared-owner positions and distinct absent boxes for requested default array elements | **Open** |
| [SB10](#sb10--canonical-compiler-representation) | Type and IR representation | Extend shared targets with `Optional(OptionalTypeId)`; do not add a redundant box type ID | **Open** |
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

### Observation, unwrap, and mutation

The shared edge must be crossed explicitly:

```ska
if ((*box) is none) {
    *box = Item();
}

(*box)!.use_item();
*box = none;
```

`*box` denotes the existing `P?` container place in the allocation. Ordinary
consumers then reuse current optional behavior:

- `*box is some` and `*box is none` inspect the wrapper without copying it;
- `var copy: P? = *box` copies the wrapper when its payload capabilities allow;
- `(*box)!` checks and removes exactly one optional layer;
- `inspect(*box)` may bind a `ref value: P?` or `mut ref value: P?` parameter;
  and
- `*box = source` replaces the wrapper through its existing assignment plan.

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

**Recommended direction:** Accept `shared O` exactly when `O` is an existing
executable `OptionalTypeId`. This includes primitive, exact-class, array,
shared-owner, and recursively nested optional payloads already accepted by the
optional contract.

This deliberately permits `shared P??`, `shared P[]?`, and
`shared (shared P)?` without separate box families. Eligibility is inherited
from the complete optional identity; the box layer does not repair an invalid
optional payload. Consequently `shared Obj?`, `shared Interface?`, and
`shared unit?` remain invalid because those inline optional types are invalid.
An optional shared object view can instead appear as the eligible payload
`(shared Obj)?`, giving `shared (shared Obj)?` when a box around that value is
actually wanted.

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
var first: shared Item? = new Item?();
var alias: shared Item? = first;
*alias = Item();
// (*first) is some: both owners reach the same wrapper.

var independent: shared Item? = new Item?(*first);
*independent = none;
// first remains present: this allocation has a copied, independent wrapper.
```

This distinction is the purpose of the feature. No implicit operation combines
owner copying with payload copying, and no box copy-on-write behavior is
introduced.

## SB5 — Explicit access and inner unwrap

**Question:** Should optional operations be forwarded through a box owner?

**Recommended direction:** No. Prefix `*` produces the exact optional pointee
place. Presence, unwrap, copying, aliases, and mutation then apply to that
place through existing rules.

This keeps `shared? P?` unambiguous: postfix `!` always removes the outer
optional-owner layer, while `(*owner)!` removes the inner boxed optional layer.
It also preserves the established separation between handle consumers and
pointee consumers. `->` remains an object/array operation and does not acquire
optional forwarding behavior.

Reading `*box` as an owning value requires the optional target's ordinary copy
capability. Presence tests and eligible alias or checked-view consumers do not
needlessly copy the complete wrapper.

## SB6 — Boxed optional replacement

**Question:** Should `*box = source` be supported even though whole-pointee
assignment is rejected for current object and array owners?

**Recommended direction:** Yes, but only for `Shared<Optional<P>>`. Without
presence-changing replacement, an absent box cannot become useful shared
state, and the feature degenerates into an immutable allocation wrapper.

The assignment is optional-container assignment, not owner replacement. It
must:

1. evaluate the owner destination once and retain or anchor that exact
   allocation before later effects can invalidate a replaceable owner place;
2. evaluate and secure the source under the existing optional assignment
   rules;
3. reject or terminate before mutation if an active checked payload view
   guards the wrapper;
4. perform the selected secure-before-destroy optional transition; and
5. release source temporaries and any hidden owner anchor in ordinary reverse
   order.

Direct and indirect self-assignment must remain safe even when two different
handles name the same allocation. Assigning `none` requires only the existing
clear/destruction behavior; assigning a present value requires exactly the
copy-construction and copy-assignment capabilities selected by the current
optional contract.

Whole-pointee assignment for `shared Class`, `shared Interface`, `shared Obj`,
and shared arrays remains rejected. Supporting it for those targets has
different slicing, dynamic-class, length, and lifecycle questions.

## SB7 — Aliases, anchors, and guards

**Question:** How are non-owning uses protected when boxes have multiple
owners and mutable wrapper state?

**Recommended direction:** Compose the two mechanisms already implemented:

- the shared owner or a hidden copied/adopted anchor keeps the allocation live;
- the optional state word's existing guard keeps a checked present payload
  from being cleared, replaced, or destroyed; and
- both lifetimes end only after the complete immediate consumer secures its
  result.

A stable local or value-parameter owner can cover an immediate use directly.
A replaceable owner field is copied before exposing its pointee. A produced
box owner remains an owning full-expression temporary. Unwrapping an outer
`(shared P?)?` first creates an ordinary secured box owner, which then serves
as the inner allocation anchor.

`inspect(*box)` may bind an existing `ref value: P?` or
`mut ref value: P?` parameter. This borrows the always-present wrapper, not an
optional reference. A mutable alias may replace its state through existing
optional assignment. By contrast, `inspect((*box)!)` borrows or consumes the
present `P` payload under both the owner anchor and a checked optional guard.

Because the guard lives in the shared allocation payload, mutation through
any other owner observes it and follows the existing guarded-mutation failure
rule. The owner anchor also ensures last-owner finalization cannot run while a
checked view remains live. Atomic guards, thread safety, and data-race
prevention remain excluded.

Aliases whose designated type is itself a shared owner remain unsupported;
users pass `shared P?` by value and explicitly dereference it when a `ref P?`
or `mut ref P?` consumer is intended.

## SB8 — Compatibility, casts, and polymorphism

**Question:** Can boxes participate in the existing class/interface/`Obj`
shared compatibility relation?

**Recommended direction:** No. Box compatibility is exact
`OptionalTypeId` equality. A `shared Derived?` box cannot become
`shared Base?`, even though an unboxed `Derived` payload may have a Base view.
Mutation would otherwise allow a Base value to be written into storage whose
layout and lifecycle were allocated for Derived.

Box owners do not up-view to `shared Obj`, satisfy interfaces, enter virtual
dispatch, support object type tests, or use owner-preserving object casts. An
explicit box-owner cast, including a redundant same-target spelling, is not
part of the initial feature; ordinary exact assignment and argument passing
already copy or transfer the owner.

Polymorphism may still exist *inside* an eligible optional shared-owner
payload. For example, `shared (shared Base)?` is an exact box target whose
present value is an ordinary `shared Base`; initialization of that inner owner
may use the already-supported compatible shared up-view.

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

**Recommended direction:** No. Extend `ResolvedSharedTarget`,
`HirSharedTarget`, and `MirSharedTarget` with an
`Optional(OptionalTypeId)` variant. The complete shared type is then the
existing `Shared(target)` around the already-canonical optional identity.

A separate `SharedBoxTypeId` would duplicate a one-to-one mapping, introduce a
second interner and dump order, and provide no additional semantic choice in
this scoped feature. If generalized boxes later need target-specific policy,
that design can introduce a broader allocation-target identity with actual
new information.

Object-only conversions must stop accepting a generic shared target and then
assuming a view target exists. Instead, shared-target capabilities should be
queried explicitly:

```text
owner operations       class | interface | Obj | array | optional
object views/dispatch  class | interface | Obj
array operations       array
optional-place access  optional
```

HIR should add a typed optional-box allocation producer and a checked shared
optional-pointee place carrying owner provenance, access, exact optional ID,
span, and anchor strategy. It should reuse the current typed optional
initialization and assignment plans rather than lowering a generic expression
and rediscovering its lifecycle later.

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
accept the extended target. Existing optional MIR operations should act on
`SharedAllocationPayload` during construction and `SharedPointee(owner)` after
publication.

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

Each referenced optional box target receives one deterministic metadata
record and one compatible finalizer entry. The finalizer accepts the complete
optional payload address and invokes the existing destruction plan for that
`OptionalTypeId`: it does nothing for absence, conditionally destroys an
inline payload, recursively cleans nested/array payloads, or releases a
present inner shared owner. The generic last-release path then frees the exact
header once.

Box metadata is never treated as a class descriptor or interface witness.
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
counting, metadata publication, optional initialization, guarded mutation,
finalizer generation, and target checking remain compiler-owned, so the
public runtime ABI remains version 9.

Allocation failure, size overflow where handled at runtime, optional unwrap
failure, guard overflow, and guarded mutation retain the current unrecoverable,
non-unwinding behavior. Statically knowable layout overflow remains a backend
error. Invalid metadata, zero ordinary handles, double finalization, and
ownership underflow remain compiler/runtime defects.

## SB13 — Diagnostics, dumps, tests, and promotion

**Question:** What evidence is required before the design may be frozen and
then implemented?

**Recommended direction:** Preserve exact `shared`, shorthand `?`, target
`?`, grouping, `new`, dereference, assignment, and initializer spans in the
source-shaped phases. Semantic dumps use canonical type spelling and
deterministic optional identity order. Diagnostics should distinguish invalid
optional payloads, unavailable wrapper lifecycle operations, box construction
arity, object-only operations on boxes, exact-target mismatches, and remaining
unsupported whole-pointee assignment targets without freezing prose wording.

The eventual implementation needs focused evidence for:

- parsing, grouping, precedence, shorthand provenance, recovery, and canonical
  resolved/HIR/MIR dumps;
- exact target identities across modules and arbitrarily nested optional
  targets, including arbitrary layers outside grouped ordinary and box owners;
- absent, injected, `some`, copied, produced, nested, optional-array, and
  optional-shared-owner box construction;
- owner copy versus independent box allocation and deterministic last-owner
  finalization;
- presence tests, primitive extraction, inline checked views, optional-array
  access, aliases, and box replacement;
- direct and indirect self-assignment and mutation through distinct owners of
  one allocation;
- owner anchors plus guard conflicts through locals, fields, produced owners,
  and optional box owners;
- fields, internal arguments/results, statics, arrays, element lists, and
  distinct default-created box elements;
- rejected `Obj?`/interface/unit targets, box upcasts/casts/type tests,
  implicit forwarding, invalid construction arity, unavailable lifecycle,
  external signatures, and non-box whole-pointee assignment;
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
- optional `Obj` or interface values that lack an inline storage model;
- implicit owner-to-pointee dereference, presence forwarding, optional
  chaining, coalescing, or propagation;
- box covariance, object/interface/`Obj` views, dynamic casts, type tests, or
  virtual dispatch;
- whole-pointee assignment for existing object or array shared targets;
- aliases whose designated type is a shared owner, first-class references,
  optional references, or escaping pointers into a box;
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

### Add immutable boxes only

Immutable boxes could share a captured optional value, but an absent box could
never become present and owner copies would add little beyond copying the
optional value itself. Box-only whole-pointee assignment supplies the intended
shared mutable container while keeping existing object replacement questions
out of scope.

### Forward optional operations through the owner

Allowing `box!` or `box is none` to inspect the pointee would conflict with the
outer optional layer of `shared? P?` and hide allocation anchoring. Explicit
`*` makes each lifetime and failure boundary visible.

### Introduce one Rust enum family per payload category

Primitive, class, array, shared-owner, and nested box variants would duplicate
the canonical optional table and grow combinatorially. The existing
`OptionalTypeId` already owns exactly the information a box finalizer and
mutation operation need.
