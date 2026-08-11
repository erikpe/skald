# Optional-Values Compiler Contract

Status: authoritative implemented compiler contract for primitive and exact
inline-class optionals, optional shared owners, and supported inline
optional-container aliases through HIR, MIR verification, x86-64 lowering,
and native execution. Recursive source type syntax, canonical `(shared T)?`
owners, recursive identities, nested owning lifecycle, checked access, aliases,
and internal callable boundaries are implemented. Tagged optional inline
arrays execute through every supported owning, aggregate, internal callable,
array-element, and checked-alias position. MIR uses canonical optional
identities and recursive lifecycle metadata.
The [language optional-value contract](../language/OPTIONAL_VALUES.md) defines
source meaning, the [status matrix](../language/STATUS.md) defines compiler
availability, and the [implemented grammar](../language/GRAMMAR.md) remains
authoritative for source currently accepted by the compiler. This document
also records the frozen compiler representation for
[shared optional boxes](#frozen-shared-optional-box-representation).

This document defines phase ownership, target-independent invariants, the
initial x86-64 representation and internal ABI direction, failure lowering,
and test obligations for explicit optionals. The AST contains recursive
source-shaped type nodes, while resolved IR contains deterministic interned
optional identities. Primitive and exact-class
optionals continue through explicit typed HIR and MIR places, calls, and
lifecycle and checked-view operations; optional shared owners likewise
continue through explicit owner operations; the verifier proves their storage,
aggregate-boundary, conditional ownership, guard, anchor, and failure-edge
invariants, and the x86-64 backend executes them. Supported inline optional
containers also pass through non-owning alias places with explicit access.

## Phase ownership

Each phase owns one stable responsibility:

| Phase | Optional-value responsibility |
|---|---|
| Lexing and parsing | Preserve recursive `?`/`[]` type composition, grouping, postfix `!`, reserved `none`, contextual `some`, `shared?` shorthand provenance, presence tests, precedence, trivia, and recovery spans. |
| Resolution | Normalize `(shared T)?` and `shared? T`, intern complete optional payloads bottom-up without source spans in identity keys, and retain exact optional-box targets for explicit access. |
| Type checking and HIR | Reject payload identities or positions not yet executable, then select compatibility, lifecycle, checked-view, anchor, and boundary requirements by canonical optional identity. |
| MIR lowering | Lower the canonical optional table deterministically and make storage state, conditional lifecycle, failure edges, presence guards, shared ownership, temporaries, and cleanup executable and explicit. |
| MIR verification | Prove storage, payload, owner, guard, anchor, transition, failure, and CFG invariants independently of source shape. |
| Backend | Realize only verified layouts, state transitions, calls, traps, and ownership operations for the selected target. |
| C runtime | Remain unaware of optional tags, guards, payload layout, unwrap, and conditional ownership. |

No backend may infer optional source semantics from a type use or insert
unverified conditional cleanup. No earlier target-independent phase may encode
x86-64 byte offsets, tag values, registers, or runtime symbols.

## Type identities

The supported operations still have two distinct runtime categories:

- inline optional over a primitive or exact class payload; and
- optional shared owner over a class, interface, or `Obj` shared target.

Resolution, HIR, and MIR use `OptionalTypeId` and dense canonical payload
tables. All optional uses, including optional shared owners, carry that one
identity shape. Type checking remains the eligibility boundary for payloads
that are not executable yet; repeated phase-local boolean flags and parallel
MIR type families are not used.

The type model must preserve these distinctions:

```text
T           ordinary inline payload
T?          inline optional payload
shared T    ordinary non-null shared owner
(shared T)? optional shared owner
```

`shared? T` is an exact source shorthand for `(shared T)?`; both intern to the
same resolved identity and lower to the existing optional-owner operations.
`shared T?` and `shared? T?` receive deterministic resolved, HIR, and MIR box
identities. Direct local box owners, outer optional-owner layers,
construction, compatible owner copy/replacement, and independent exact-wrapper
copying lower through verified target-independent MIR. The x86-64 backend
executes local exact boxes for primitive, class, inline-array, shared-owner,
optional-array, and recursively nested targets. Explicit exact pointee
presence, wrapper copies, read-only aliases, one-layer unwrap, and contained
value consumers execute. Stored/callable positions and polymorphic object-box views remain behind focused gates;
invalid standalone payload families retain their ordinary diagnostics. Nested optionals
are executable in owning positions, checked access, aliases, and internal
callable boundaries. Alias binding mode may designate any supported inline
optional container; it does not add a reference or optional-reference type
identity.

## Compositional optional implementation

The recursive syntax and resolved-identity portions of this direction are
implemented, including grouping, postfix chains, shorthand provenance, and
bottom-up interning. Canonical HIR and MIR identity tables, lifecycle plans,
checked access, aliases, callable integration, and optional arrays are
implemented across supported storage, aggregate, dispatch, array-element, and
checked-alias boundaries. Every payload category preserves the established
diagnostics, evaluation order, lifecycle, layout, ABI, and native behavior.

### Source shape and canonical identity

Syntax uses a recursive type node. It preserves every `?`, `[]`, `shared`, and
grouping span and records whether the user wrote the `shared?` shorthand.
Syntax identity remains source-shaped; semantic identity does not.

Resolution interns optionals bottom-up into a deterministic table:

```text
OptionalTypeId -> {
    payload: ResolvedType
}
```

The public identity and resolved-table facade is intentionally narrow. The
durable requirements are:

- resolved, HIR, and MIR types carry a small copyable optional identity rather
  than recursively owning another type node;
- an optional entry names its complete immediate payload, which may be a
  primitive, exact class, inline array, ordinary shared owner, or earlier
  optional identity;
- source spans, grouping, and shorthand provenance do not participate in
  equality or interning keys;
- inner types are interned before their wrappers, repeated spellings reuse one
  identity, and module traversal produces deterministic IDs and dumps;
- `(shared P)?` and `shared? P` normalize to one
  `Optional<Shared<P>>` identity; and
- `shared P?` is instead `Shared<Optional<P>>`; the current compiler rejects it
  before executable HIR, while the frozen box extension gives it an exact
  optional allocation target and, for object boxes, a distinct static view
  identity.

This follows the existing array-table pattern without requiring one universal
type interner. Optional and array entries may name one another through their
small IDs because finite source syntax is interned from the innermost complete
type outward. Named inline-class containment remains a separate semantic graph
and traverses through every optional payload.

### Type checking and lifecycle capabilities

Type checking owns an ID-indexed optional description whose immediate payload
selects:

- storage and representation category;
- absence and presence initialization;
- copy construction, assignment, and destruction capability;
- direct final-destination construction or produced-value transfer;
- scalar extraction, owning aggregate extraction, secured shared-owner
  extraction, or checked-place access;
- guard and shared-root anchor requirements;
- parameter, result, alias, field, static, and array-element eligibility; and
- the exact operation required when an outer transition recursively consumes
  the payload.

The description may use private enums and references to existing class and
array lifecycle plans, but it must not enumerate combinations such as
`OptionalOptionalClass` or duplicate array element lifecycle selection. A
payload capability is computed once by its ordinary owner and reused by the
optional wrapper.

Compatibility has only two relevant relations: exact optional identity and
one-layer injection from exact payload `P` to `Optional<P>`. Resolution and
type checking never search a transitive injection chain. `some(expression)` is
checked only under one unambiguous expected optional identity and supplies the
entry's payload as the expected type of its argument. `none` likewise targets
the outer expected identity. Exact overload matches outrank one-layer
injection; contextual `none` or `some` does not invent specificity.

### Typed HIR and executable MIR

Typed HIR carries canonical optional identities and explicit semantic plans
for absent/present initialization, `some`, one-layer injection, optional copy
and assignment, recursive cleanup, presence tests, one-layer unwrap, checked
views, aliases, arguments/results, statics, and array elements. HIR selects
the immediate payload operation; MIR and the backend never rediscover it from
the source type.

Typed HIR and MIR use one `OptionalTypeId` vocabulary and ID-indexed semantic
descriptions with payload, storage, representation, lifecycle, checked-access,
and boundary plans. HIR-to-MIR lowering copies those plans deterministically;
there is no compatibility adapter or parallel primitive/class/shared MIR type
family. Distinct scalar, owning-aggregate, and shared-owner instructions remain
where they express genuinely different runtime work. Nested owning lifecycle,
checked access, aliases, and callable cases use recursive outer-wrapper
operations. Optional arrays use the owning-aggregate operations and reuse the
ordinary array lifecycle selected by type checking.

MIR represents the outer wrapper state independently from the immediate
payload state. An outer operation branches on or changes only its own state,
then invokes the selected payload operation when required. Publication occurs
only after the complete immediate payload has initialized successfully.
Recursive cleanup consumes one initialized outer wrapper and conditionally
cleans one complete immediate payload; it does not flatten nested state or
infer cleanup from raw loads and stores.

Each unwrap terminator checks one wrapper. Scalar payloads are copied,
ordinary shared-owner payloads are secured into owner storage, and owning
aggregate payloads are copied or transferred according to their selected
source category. A payload place carries its exact optional root and identity.
Every active checked view guards the wrapper whose payload it exposes; nested
unwraps may therefore have multiple balanced guards. Shared-root anchors cover
container storage independently of those guards.

### Verification

Generalized MIR verification must prove, recursively and without source
inspection, that:

- every optional identity and payload identity exists and is legal for its
  storage position;
- wrapper initialization is distinct from dynamic presence and is established
  exactly once per storage epoch;
- an absent wrapper owns no live payload and a present wrapper owns exactly one
  complete compatible payload;
- each initialization, copy, assignment, cleanup, argument/result transfer,
  static transition, and array-element operation carries the capability
  selected for that immediate payload;
- nested publication and cleanup order is outer/inner state-safe and no absent
  bytes are addressed as a payload;
- each unwrap removes one layer, has distinct success and non-returning failure
  edges, and yields the declared immediate payload category;
- guards name the correct wrapper identity, are balanced around their complete
  consumers, and block every presence-changing transition at that layer;
- shared anchors outlive every optional place rooted in their allocation;
- CFG joins agree on initialized owning wrapper storage even when runtime
  presence differs; and
- external signatures and unsupported shared-box targets never enter
  executable MIR.

Malformed public or test-mutated MIR fails at this target-independent boundary
before target layout or code generation.

### x86-64 layout and internal ABI

For an inline optional without a valid niche, layout applies the existing
formula recursively to the complete immediate payload layout:

```text
state offset   = 0
payload offset = align_up(8, align_of(P))
alignment      = max(8, align_of(P))
size           = align_up(payload offset + size_of(P), alignment)
```

Every addition and alignment is checked against target limits. A nested
optional reserves the complete inner wrapper as `P`; each layer keeps its own
state and guard count. `Optional<Array<T>>` uses this tagged layout around the
complete inline descriptor because the empty descriptor is already a valid
present array. No absence niche changes array descriptor meaning.

`Optional<Shared<P>>` retains the existing one-word zero-handle niche and
direct integer-class shared-owner convention. Only that immediate
optional-of-shared identity receives the niche. Wrapping it in another
optional produces an ordinary tagged outer aggregate, so
`Optional<Optional<Shared<P>>>` is not flattened to one word.

All non-niche inline optional parameters continue to use one pointer-sized
internal argument referring to caller-prepared owning aggregate storage, and
results continue to use caller-provided destination storage. This convention
applies equally to nested and optional-array aggregates. A call-scoped alias
passes the address of the existing wrapper and owns no payload. The optimized
optional-of-shared form continues to pass and return one integer-class word.
These remain compiler-private conventions; every external optional signature
is rejected.

Static slots use the same target layout as equivalent non-static storage.
Exact-reverse normal-return shutdown recursively destroys a present payload or
releases a present owner. Abrupt termination remains non-unwinding.

### Array composition and runtime boundary

The optional type table names an existing canonical array identity as the
payload of `T[]?`. Type checking reuses that array's default, copy, assignment,
destruction, backing-transfer, alias-anchor, and element lifecycle plans.
Optional lowering may branch around those operations but does not create a
second array capability system or new array descriptor.

An optional array used as an array element defaults its outer wrapper to
absence. Explicit element lists initialize the wrapper directly and advance
the unpublished prefix only after the complete absent or present wrapper is
live. Present payload cleanup reuses the existing array release helper and
reverse element lifecycle.

The compositional extension adds no C runtime symbol and does not revise
runtime ABI version 9. Tags, guard counts, recursive state, optional-array
layout, shared-owner zero niches, and checked unwrap remain compiler-owned.
The existing allocator and deallocator see only ordinary valid array backing
requests and exact allocation bases.

## Frozen shared optional box representation

Status: **frozen design; exact local native access implemented**. Local owner
compatibility and exact wrapper construction lower through target-independent
MIR with explicit allocation, wrapper completion, publication, adoption, and
owner lifetime verification. The x86-64 backend executes every eligible exact
wrapper with a checked header-plus-target layout, deterministic exact
descriptor, and distinct recursive finalizer. Explicit access through exact
owners is implemented; polymorphic views and broader stored positions remain
deliberately gated.

`Shared<Optional<P>>` reuses the canonical `OptionalTypeId` for the exact
allocation payload. Exact primitive, array, shared-owner, nested optional, and
other non-object boxes may name that identity directly. Optional object boxes
also require a canonical static box-view identity whose optional depth and
class/interface/`Obj` leaf may differ from the allocation's exact concrete
class optional identity. The representation must distinguish these target
capabilities rather than treating every non-array shared target as an object:

```text
owner operations       object | array | exact optional | optional object view
object views/dispatch  object | present optional object view
array operations       array
optional-place access  exact optional | optional object view
```

The exact Rust enum and interner names are implementation-private. Identities,
compatibility, canonical names, and dumps must remain deterministic, and
interface/`Obj` box views must not manufacture bare owning optional types.

HIR owns a typed optional-box allocation producer. It records the exact
optional allocation target, optional initialization or copy plan, static owner
view, source-before-allocation order, owner provenance, source spans, and
publication boundary. Exact primitive, class, array, shared-owner, and nested
optional plans reuse ordinary destination initialization. Interface and `Obj`
box views carry no invalid standalone optional identity. Exact shared
optional-pointee places record their owner source, exact box target, optional
identity, and span. Stable owners borrow directly; replaceable or produced
owners retain a full-expression anchor before the place is exposed.
Published pointees have no assignment plan: whole-pointee assignment and
mutable whole-wrapper aliases are rejected before HIR.

MIR gives optional boxes a distinct auditable allocation origin and keeps this
transition explicit:

```text
allocated exact optional storage
    -> initialized complete P? wrapper
    -> published count-one box
    -> adopted ordinary owner
```

`SharedAllocationPayload` denotes the unpublished exact wrapper destination.
It is accepted only by the selected initialization operation until one
publication and adoption complete the produced owner. `SharedPointee(owner)`
denotes the immutable published wrapper reached through a verified live owner.
Existing exact
optional lifecycle instructions operate on those places rather than gaining a
parallel primitive/class/array box family. Object-box view, cast, unwrap,
copy, and dispatch operations retain both the static view and exact dynamic
descriptor dependency through verification.

Verification rejects target-family confusion, wrong or missing allocation
origins, pre-publication observation, optional initialization/publication
errors, owner loss or duplication, mismatched metadata/finalizers, invalid
casts, guard/anchor imbalance, mutable wrapper access, and duplicate or missing
cleanup. Existing optional initialization, ownership, place, lifetime, call,
array, static-lifecycle, and CFG verifiers remain the responsible owners; the
feature must not add a second monolithic box verifier.

On x86-64 a box handle remains one integer-class owner word. Its allocation is
the existing 16-byte shared header followed at offset 16 by the target-layout
placement of the canonical `P?` wrapper. One deterministic descriptor records
the exact optional target, compatible finalizer, and—only for object boxes—the
exact dynamic class and view-membership evidence. The finalizer invokes the
existing recursive optional destruction plan before exact-base deallocation.
Layout uses the target data-layout authority and reports size, alignment, or
addressability overflow instead of assuming an eight-byte payload alignment.

`(shared P?)?` reuses the existing optional-owner zero niche and conditional
owner lifecycle. Zero means no box; a nonzero word is an ordinary box handle.
The inner optional state remains in the allocation and is independent of that
outer niche. Allocation, tags, guards, metadata, strong counting, finalization,
casts, dispatch, and failures remain compiler-generated; runtime ABI version 9
and its public C symbols remain unchanged.

## Source-shaped IR

The AST preserves every optional payload as a recursive type node, including
grouping, punctuation spans, and `shared?` shorthand provenance. The current
resolved expression IR preserves:

- the `?` span on an inline type;
- the `shared` and owner-optional `?` spans separately;
- `none` as a distinct source expression;
- `is some` and `is none` as presence tests distinct from object type tests;
- postfix unwrap as a distinct postfix operation; and
- enough operator and operand spans for deterministic recovery and diagnostics.

Canonical semantic dumps use `T?` and `(shared T)?`, independent of source
trivia. Syntax dumps retain whether the source used postfix notation or the
`shared?` shorthand.
Frozen box spellings remain visible to diagnostics but do not acquire an
executable type identity until their roadmap's identity task completes.

## Typed HIR

HIR records semantic operations selected by type checking rather than asking
MIR or the backend to rediscover them from expected types. By responsibility,
it must distinguish:

- empty optional construction with its exact destination type;
- present injection from a primitive, inline object source, or shared owner;
- optional copy construction, assignment, and destruction;
- non-failing presence tests;
- checked primitive value extraction;
- checked inline payload places with exact root, access, target, projections,
  and immediate consumer;
- optional shared-owner copy, adopt/move, release, and secured unwrap;
- direct fresh construction into a new inline payload destination;
- stable, copied, or adopted shared anchors covering an optional container;
  and
- source evaluation and full-expression temporary order.

Flow-sensitive knowledge may classify a check as statically successful for
optimization, but HIR source legality never depends on that classification.
Every unwrap remains a checked semantic operation.

The implemented optional subset uses explicit HIR nodes for absent and present
initialization, exact optional copy and assignment, field places,
arguments/results, produced calls, presence tests, conditional class lifecycle,
checked primitive unwrap, guarded class payload places, optional-owner
copy/adopt/move/release, and secured shared unwrap.

## MIR optional storage model

Every source-visible optional local, field, parameter, result, owning
temporary, inline-optional static, and optional shared-owner static lowers to
initialized optional storage. Static optional containers are seeded as absent
at every callable entry and remain initialized independently of local
`StorageLive` epochs. The wrapper lifetime and the conditional payload lifetime
are distinct:

```text
wrapper storage: uninitialized -> initialized -> ended
payload state:                 absent <-> present
```

The wrapper must be initialized exactly once before use. Dynamic presence may
differ across CFG predecessors, but every predecessor must agree that the same
compatible wrapper storage is initialized and owned. Presence remains runtime
state rather than a verifier guess.

MIR represents, by semantic operation rather than necessarily by these Rust
names:

- initialize absent;
- initialize present from a primitive value;
- construct or copy-construct an inline payload;
- copy or assign an optional;
- test presence;
- branch on checked unwrap success/failure;
- begin and end a checked inline payload view;
- conditionally destroy an inline payload;
- copy, adopt/move, and conditionally release an optional shared owner;
- secure an unwrapped ordinary shared owner;
- reject a presence-changing transition while guarded; and
- terminate for absent access, guard overflow, or guarded mutation.

The backend executes those operations. It does not synthesize them from loads,
stores, or cleanup lists.

Primitive optional MIR represents absent/present initialization, optional copy
and assignment, presence tests, and unwrap as a success/failure terminator. Its
failure successor is an explicit empty block ending in
`OptionalAccessFailure`. Definite-initialization verification intersects
initialized wrapper storage at ordinary CFG joins and deliberately does not
treat dynamic presence as a static fact. Across a declared MIR path condition,
it instead retains selected alternatives until their conditional storage and
cleanup have converged. Owning exact-class and optional-shared cleanup or value
transfer consumes the selected wrapper state; a skipped alternative neither
initializes nor releases it.

Exact-class optional MIR additionally records conditional initialization,
publication after destination-directed construction, copy construction,
assignment, cleanup, and explicit begin/end checked-view operations. A
successful begin yields the exact payload projection; ordinary source access
to those bytes is valid only for its verified immediate consumer while the
matching guard is active. Cleanup or ownership transfer consumes the
wrapper's initialized owning state. Definite-initialization verification
therefore rejects duplicate cleanup and storage death while an exact-class
optional still owns its initialized absent-or-present state.

## Lifecycle state machine

Optional initialization publishes presence only after the complete payload
initialization succeeds. Optional assignment evaluates and secures its source
before changing the destination.

The executable transition matrix is:

| Destination | Source | Required MIR effect |
|---|---|---|
| absent | absent | Preserve absence |
| absent | present | Initialize or copy-construct payload, then publish presence |
| present | absent | Verify unguarded, destroy/release payload, then publish absence |
| present | present | Verify legal access and perform payload assignment |

For a direct non-optional source, the source is present. A named shared source
is secured by owner copy; a produced owner is adopted or moved according to
its existing provenance. Direct or allocation-alias self-assignment remains
safe because incoming ownership is secured before the old optional owner is
conditionally released.

Fresh ungrouped exact construction into newly initialized `T?` uses the
optional payload as the final construction destination. Other object sources
retain the ordinary copy/materialization semantics selected by HIR.

An inline-optional array element-list slot is another newly initialized
optional destination. Primitive and class absence, injection, conditional
copying, direct payload placement, and optional call-result copying use these
same operations against the current unpublished array slot. Class presence is
published only after payload completion, and the array's initialized prefix
advances only after the complete wrapper is live.

Construction, payload assignment, and destruction may execute user code. MIR
must keep the optional in an internal transition-safe state or guard its live
payload so re-entrant source operations cannot end or overwrite the payload
being consumed. Exact state encoding is backend private; no transient state is
a source-visible third optional value.

## Checked payload views

An inline unwrap used as a place lowers to a checked payload carrier plus an
active presence guard. The guard begins at source evaluation and ends after
the complete immediate consumer.

For a call:

1. evaluate the receiver or argument source;
2. establish any required shared owner anchor for the optional container;
3. check presence and begin the payload guard;
4. evaluate later arguments left to right;
5. execute the call while the container anchor and payload guard remain live;
6. secure the result;
7. end checked views; and
8. release anchors and other temporaries in reverse completion order.

A stable inline container needs no owner anchor. An optional field reached
through stable shared owner storage uses that owner. A replaceable shared field
copies an owner into `SharedAnchor`; a produced owner is adopted into its
existing full-expression owner. The shared anchor covers storage lifetime,
while the presence guard covers conditional payload lifetime.

Primitive extraction copies before later effects and needs no continuing
guard. Optional shared-owner extraction secures one ordinary non-null owner and
likewise needs no continuing optional guard. When the extraction directly
initializes a local, the checked unwrap first writes a fresh owning temporary;
a count-neutral move then installs that owner in the local on the success edge.
This protocol is identical when the optional container is a direct or
forwarded call result: the call result remains an optional temporary, the
unwrap secures a distinct ordinary owner, and normal full-expression cleanup
disposes of the consumed optional container.

Structured source-level short-circuit lowering preserves those three distinct
lifetimes. A selected inline-class payload view ends after its complete field,
method, argument, cast, or other immediate consumer and before the operand
publishes its boolean result. A primitive unwrap publishes its copied scalar
before conditional unwrap storage ends. An optional shared unwrap instead
publishes a secured ordinary owner, which remains subject to the enclosing
full-expression ownership plan. A skipped operand begins no guard, performs no
presence check, and secures or releases no owner.

## Presence-guard state

The target-independent model tracks:

- whether optional storage is initialized;
- whether a checked payload view is active;
- the view's compatible optional root and payload type;
- guard nesting/count balance;
- any owner anchor covering the container storage; and
- the normal point where the view ends.

The source-visible optional states remain absent and present. The backend state
may additionally represent one or more active guards and an internal lifecycle
transition.

Beginning a checked view:

1. verifies that the optional is present;
2. verifies that the guard count can be increased;
3. records one additional active view; and
4. yields the payload place only on success.

Clearing, replacing, or destroying the container requires no active view.
Failure occurs before any tag, payload, owner, or lifecycle change. Ordinary
mutation within the still-present payload is unaffected.

No separate source-visible lock is introduced. On x86-64 the state word may
encode presence and the active guard count together. Exact numeric encodings
and any internal transition marker remain backend private.

## MIR verification

The verifier rejects a program unless all of the following hold:

- every optional storage entry has one legal optional type and is initialized
  before any test, unwrap, copy, assignment, or cleanup;
- absent storage has no live payload or strong-owner account;
- present inline storage has one compatible live payload;
- present optional shared storage accounts for exactly one ordinary strong
  owner;
- absent payload bytes are never addressed through a `T` place;
- initialization publishes presence only after complete payload construction;
- optional assignment secures its source before changing its destination;
- each normal optional lifetime end conditionally cleans exactly one present
  payload or owner;
- every checked payload carrier has one compatible live optional root;
- begin/end guard operations are balanced and ordered around the complete
  immediate consumer;
- a checked view ends before its container anchor or owning temporary ends;
- clearing, replacement, destruction, and lifecycle transitions prove no
  active guard or branch to the matching failure;
- normal exits contain no active optional guard;
- optional failure edges terminate with their exact non-returning reason;
- zero optional-owner representation never reaches an ordinary shared
  retain, release, dereference, metadata, cast, or finalization operation;
- CFG joins agree on initialized storage, live owning temporaries, active
  guards, anchors, and lifecycle transitions; and
- optional values are absent from external signatures.

Malformed public or test-mutated MIR must fail verification before target
layout or instruction selection.

## Target-independent failures

MIR uses distinct termination reasons for:

- absent optional access;
- presence-guard count overflow; and
- clearing, replacing, or destroying a guarded optional.

Every reason is non-returning. A success and failure edge must differ, and the
failure block may not continue into ordinary cleanup or payload use.
The frozen [common reporting policy](../language/ERRORS.md#frozen-panic-design)
maps these reasons to one reporter without collapsing them in MIR. This
document deliberately does not repeat the exact message bytes.
Copying or unwrapping a present optional shared owner also uses the generic
shared retain contract: legal count exhaustion reports through that reporter,
while a non-null zero-count handle is corrupted state and hard-traps. Absence
branches around retain and release and therefore remains the zero niche.

Future recoverable exceptions must add explicit exceptional edges that end
active guards and clean initialized optional temporaries. They cannot reinterpret
the current non-returning reasons as catchable exceptions.

## Initial x86-64 inline layout

Inline primitive and class optionals use one eight-byte, eight-aligned state
word followed by the payload at its required alignment:

```text
state offset   = 0
payload offset = align_up(8, align_of(T))
alignment      = max(8, align_of(T))
size           = align_up(payload offset + size_of(T), alignment)
```

The current target supports payload alignment no greater than eight, but the
formula remains explicit so a later wider-alignment type cannot be silently
mislaid out.

The payload reserves the ordinary complete representation of `T`. Absence
does not initialize those bytes as a `T`; loads, projections, copying,
assignment, and cleanup branch on verified state before addressing them as a
payload.

An exact-class optional static uses this same layout in a separate program
slot. It adds neither an inline-class containment edge nor bytes to instances
of its declaring class. On normal entry return, generated exact-reverse static
shutdown conditionally destroys its final present payload; abrupt termination
remains non-unwinding.

Zero means absent, one means present without an active view, and values from
two through the maximum word value represent a present payload with one or
more active guards. Beginning a view traps rather than overflowing the word;
clearing, replacement, and destruction accept only zero or one and trap before
mutating guarded storage.

Fields use the same layout recursively. An optional exact class therefore
contributes the complete payload layout to containment-cycle detection even
when its runtime state may be absent.

## Initial x86-64 optional shared-owner layout

`(shared T)?` is one eight-byte, eight-aligned integer-class word:

- zero represents absence of the optional owner; and
- non-zero is the canonical header handle of an ordinary `shared T`.

Zero is never published as or converted into a plain `shared T`. Every owner
copy, release, dereference, metadata load, cast, and finalization branches on
presence or consumes a previously secured non-zero owner.

The shared allocation header, payload offset, dynamic descriptor, finalizer,
strong count, allocator/deallocator calls, and plain `shared T` representation
remain unchanged.

## Initial internal calling convention

Inline `T?` values use one uniform owning-aggregate convention:

- the caller prepares complete optional argument storage and transfers it to
  the callee through one pointer-sized internal argument;
- the callee owns the initialized parameter storage and conditionally destroys
  its payload during normal parameter cleanup; and
- an inline optional result uses caller-provided destination storage, completed
  by the callee before normal return and owned by the caller afterward.

This convention applies to primitive and exact-class inline optionals. It
prioritizes one lifecycle-correct boundary over special scalar cases; a future
optimization may specialize an ABI only with coordinated caller/callee and
documentation changes.

`(shared T)?` follows the existing direct shared-owner convention: one
integer-class argument word and one direct result word in `rax`. A present
owner is copied or transferred under the existing shared call rules; absence
transfers zero without retain or release.

An alias to inline `T?` passes one integer-class address to the existing
container storage. Unlike an object-view alias, it carries no complete-object
or dynamic-metadata components. The callee treats that address as indirect
initialized optional storage and never owns or cleans it. The caller keeps the
container alive for the complete call.

These are compiler-private conventions. External declarations reject all
optional parameters and results.

## Backend lowering

The x86-64 backend:

- computes optional layout through one shared layout owner rather than
  duplicating offsets across instruction lowering;
- executes verified state transitions and conditional lifecycle operations;
- preserves state and payload homes across calls according to the frame model;
- implements checked unwrap and guarded mutation as explicit branches;
- leaves every optional failure as a distinct MIR termination reason selected
  by the centralized reporter path;
- conditionally emits existing copy, assignment, destruction, retain, release,
  anchor, cast, and finalization sequences; and
- never calls a C helper for optional tags, guards, or unwrap.

Backend legality rejects an optional operation whose MIR type, layout, source,
destination, failure edge, guard, or ownership effect is inconsistent.
Centralized termination lowering routes these verified reasons through the
[common backend path](BACKEND.md#panic-and-hard-trap-boundary);
optional lowering does not own a private reporter.

## C runtime ABI

The currently implemented optional profile adds no C runtime symbol and
requires no runtime ABI version bump. Optional state, guard counts,
conditional ownership, and checked access remain compiler-owned. The
version-9 reporter is a common termination ABI, not an optional-specific
helper.

The current allocator and deallocator continue to receive only the same valid
nonzero sizes and exact non-null allocation bases required by the
[runtime ABI](RUNTIME_ABI.md). The optional shared-owner zero niche is never
passed to either function or to generated plain-owner operations.

If implementation evidence requires a runtime helper, the compiler, public
header, runtime implementation, version marker, C harnesses, link-mismatch
tests, and documentation must change together. An undocumented helper is not
permitted.

## Dumps and diagnostics

Syntax, resolved, HIR, and MIR dumps use semantic wording and deterministic
identity order. They distinguish inline optional storage, optional shared
owners, checked payload views, presence guards, optional temporaries, anchors,
and failure reasons without exposing source-inaccessible machine offsets.

Diagnostics should identify:

- invalid payload and target families;
- shared-box forms used in positions or views outside the implemented profile;
- missing expected type for `none`;
- implicit unwrap and direct member/dereference attempts;
- incompatible injection, assignment, argument, and return types;
- initializer-overload ambiguity;
- external optional signatures;
- recursive inline containment through `T?`; and
- invalid checked-view access or lifetime.

Exact diagnostic codes and wording remain compiler behavior rather than
portable language semantics.

## Test obligations

Focused implementation tests must cover:

- tokens, parsing, recovery, nesting limits, spans, and deterministic dumps;
- every primitive and exact-class inline optional layout;
- optional shared class/interface/`Obj` owners and the zero niche;
- absence, presence, injection, copy, assignment, cleanup, and self-assignment;
- fields, parameters, results, temporaries, overloads, overrides, and
  interfaces;
- direct payload construction and side-effect-visible lifecycle order;
- checked value extraction and every checked object-place consumer;
- nested/overlapping guards, later-argument and re-entrant invalidation,
  shared-root anchors, and normal guard cleanup;
- each non-returning optional failure;
- mutated-MIR verifier rejection;
- register, stack, hidden-destination, recursion, and interface-dispatch ABI
  pressure;
- source-to-native positive, compile-failure, and runtime-failure goldens;
- malformed-source deterministic robustness; and
- documentation, MSRV, and complete repository gates.

Tests must continue to prove that plain `T` and `shared T` never acquire absent
or zero states and that no optional feature changes the C runtime ABI
accidentally.

### Compositional test matrix

The compositional test matrix uses the narrowest test layer that owns each
contract and adds source-to-native coverage where phase tests cannot prove
observable behavior:

| Concern | Required focused evidence |
|---|---|
| Source syntax | Tokens and AST for general grouping, left-to-right `?`/`[]` suffixes, `(shared T)?`, `shared? T`, nested depth, `some(expression)`, malformed punctuation, recovery spans, and the syntax budget |
| Canonical identity | Resolution, HIR, and MIR interning for repeated, grouped, shorthand, nested, array, and cross-module spellings; deterministic IDs and dumps; focused stored-position and backend gates |
| Type and lifecycle selection | Exact payload eligibility, one-layer injection, overload ranking, `none`/`some` expectations, recursive containment, copy/assignment/destruction capabilities, aliases, statics, and array-element plans |
| HIR and MIR shape | Explicit outer-layer operations, recursive payload plans, publication order, one-layer unwrap, guards and anchors, arguments/results, optional arrays, selected-path cleanup, and deterministic dumps |
| Verification | Mutations for missing or mismatched identities, absent payload use, wrong lifecycle capability, premature publication, duplicate/missing cleanup, bad transfers, unbalanced or wrong-layer guards, invalid anchors, malformed CFG joins, and leaked box targets |
| Target layout | Depth-two, depth-five, optional-shared niche, tagged outer-over-niche, optional-array descriptor, class containment, statics, alignment, checked size overflow, frames, and helper identity |
| Internal ABI | Register/stack pressure, recursion, methods, interfaces, virtual dispatch, hidden aggregate results, direct optional-shared words, aliases, statics, and produced array results |
| Native success | Every presence depth, `some(none)`, chained unwrap, lifecycle traces, self-assignment, alias mutation, optional arrays including present-empty, array elements/lists/slices, and exact reverse cleanup |
| Native failure | Absent access at each layer, later-check suppression, guard overflow, guarded replacement, index/slice/allocation failures inside present arrays, and unsuccessful non-returning behavior |
| Robustness and determinism | Hostile nesting and punctuation, excessive depth, repeated independent compilation, source-to-assembly determinism, runtime observation determinism, documentation validation, MSRV, and complete repository gates |

Parser, resolution, and type-check suites provide positive coverage for
`shared T?`, `shared? T?`, local box allocation, construction plans, owner
copy/adoption/replacement, and exact immutable pointee access. Compile-failure
coverage verifies the remaining stored-position and polymorphic-view gates.
Nested `T??` requires positive lifecycle, access, alias, and callable
coverage. Both `(shared T)?` and `shared? T` require positive
source-to-native equivalence coverage.

## Exclusions

The implemented compiler executes recursively nested optional owning lifecycle,
expected-type-directed `some(expression)`, checked access, aliases, and
internal callable boundaries. Optional inline arrays execute across supported
owning, aggregate, callable, array-element, and checked-alias boundaries. The
frozen box design does not include generalized boxes for non-optional values,
mutable shared optional cells, optional function values, first-class references,
optional casts, equality or operator lifting, chaining/coalescing/propagation,
recoverable failures, concurrency or atomic guards, external optional ABI, or
dynamic-type-preserving cloning.

The implemented [array compiler contract](ARRAYS.md) extends optional
shared-owner targets to exact arrays and permits already-supported optional
non-array element types. Explicit array element lists reuse the same
generic-place optional initialization, conditional owner transfer, zero niche,
and cleanup. Optional inline arrays reuse that array machinery as specified
above without altering the public C runtime ABI.
