# Array Compiler and Runtime Contract

Status: **implemented contract on x86-64, including explicit element-list
representation and execution**.
This document is authoritative for the compiler representation, lowering,
verification, target, and runtime responsibilities required by the
[array language contract](../language/ARRAYS.md). The compiler lowers all
typed array operations through verified, layout-independent MIR. The x86-64
backend executes primitive, optional, exact-class, and recursively nested
inline and shared-outer construction, access, lifecycle, deep copy,
produced-backing adoption, replacement, class fields, shared/optional-shared
owner boundaries, internal value boundaries, ordinary or optional shared-owner
element lifecycle, copied slices, checked equal-length slice assignment, and
call-scoped whole-array and exact-element aliases.
Availability remains authoritative in the
[status matrix](../language/STATUS.md).

The design borrows Niflheim's useful recursive type syntax, invariant typing,
fixed-size allocation, checked indexing, copied slices, and explicit array IR.
It deliberately does not inherit Niflheim's nullable reference elements,
garbage-collected array identity, raw reference copying, or runtime-owned
element semantics. Skald arrays must preserve inline value copying,
deterministic destruction, non-null shared ownership, explicit shared
dereference, optional owners, and call-scoped anchors.

## Responsibility split

| Layer | Array responsibility |
|---|---|
| Lexing and parsing | Preserve brackets, recursive type grouping, construction modes, index/slice shapes, omitted bounds, `->` bracket projection, trivia, recovery spans, and nesting limits. |
| Resolution | Intern canonical recursive array identities, resolve element and shared target types, retain construction mode and exact source structure, and assign stable identities without deciding layout. |
| Type checking and HIR | Decide element eligibility, invariance, default/copy/assignment capability, inline versus shared ownership, named versus produced provenance, access, normalized operation shape, alias anchors, and exact intrinsic operation. |
| MIR lowering | Make allocation, construction prefix, deep copy, produced-backing adoption, element assignment/destruction, bounds failure, owner operations, temporaries, cleanup, and anchor lifetime executable and explicit. |
| MIR verification | Prove exact types, storage state, owner accounting, initialized ranges, checked projections, lifecycle compatibility, failure edges, and cleanup ordering before target lowering. |
| Backend | Choose descriptor and allocation layout, checked arithmetic, calling-convention realization, generated helper bodies, and instruction selection for verified MIR. |
| C runtime | Continue to allocate and free checked byte blocks without knowing array types, lengths, elements, lifecycle, reference counts, or slices. |

No later layer may infer array semantics from bracket syntax, a raw pointer,
physical element size, or an expected type.

The implemented MIR boundary carries a canonical array declaration table,
owning descriptors, unpublished and produced backing roles, shared owners,
slice temporaries, positions, and anchors. Lowering emits explicit generated
blocks for construction and copy prefixes; checked allocation, normalization,
projection and slice failure edges; named deep copy and produced adoption;
whole replacement; element lifecycle operations; destruction and release.
Verification is split by array structure, storage, ownership, projection, and
anchor responsibility. It checks exact lifecycle capabilities, initialized
prefix shape, publication and consumption, owner state at control-flow joins,
checked projection, slice checks before writes, and exact terminating failure
edges. No MIR operation contains a descriptor layout, element stride, header
offset, target register, or runtime ABI fact.

The current x86-64 profile executes empty and dynamically sized inline or
shared-outer arrays containing primitives, primitive optionals, exact classes,
exact-class optionals, recursively nested inline arrays, and ordinary or
optional shared owners of exact classes and arrays. It implements
immutable `len()`, checked element access, increasing-index default and copy
construction, whole replacement, produced-backing adoption, conditional
optional lifecycle, deep jagged copying, and decreasing-index recursive
destruction. Class fields, synthesized class lifecycle, internal value
parameters and results, shared and optional-shared array owners, and recursive
class/array graphs use the same owner model. Shared elements use exact
per-slot default allocation, one-word retain/release operations, zero-niche
optional absence, and secure-before-release assignment. Slice reads allocate
distinct inline backing and copy-construct in increasing order; slice writes
validate both ranges and equal lengths before assigning in increasing order.
Right-side slice temporaries are fully materialized before writes, which gives
overlapping assignments snapshot semantics. Whole-array and element aliases
execute through non-owning internal ABI addresses. Checked element aliases
capture their selected address before later argument effects, and hidden
inline-backing or shared-owner anchors keep that address live through the
call. The type checker and MIR lowering do not use an array-wide unsupported
diagnostic.

## Canonical type model

Array identity is recursive and invariant. The target-independent compiler
should intern each exact element sequence behind a stable dense
`ArrayTypeId`, rather than recursively embedding heap-owned type nodes in every
copyable phase type. A canonical array identity records its exact element
type. Outer ownership remains a type constructor around that identity:

```text
InlineArray(ArrayTypeId)
Shared(Array(ArrayTypeId))
OptionalShared(Array(ArrayTypeId))
```

This shape is conceptual rather than a required Rust declaration. It preserves
the existing ability for phase types and identities to be cheap, stable,
deterministic values while supporting arbitrary source nesting within the
compiler's ordinary nesting budget.

`shared T[]` and `(shared T)[]` resolve differently. The former is a
shared owner whose target is an exact array identity; the latter is an inline
array identity whose element is a shared owner. `(shared T[])?` wraps shared
array ownership in absence, with `shared? T[]` as exact source shorthand; no
phase represents it as an inline optional array payload. The
[compositional optional implementation](OPTIONAL_VALUES.md#compositional-optional-implementation)
direction represents `T[]?` as an optional identity whose payload is an array.
Core optional-array HIR, MIR, verification, layout, and x86-64 lowering use
this canonical inline array identity and reuse this table's lifecycle plans.
Optional arrays use the same identity and lifecycle plans in aggregate,
dispatch, array-element, static, and checked-alias positions.

Array types do not enter class hierarchy, interface conformance, `Obj`,
dynamic metadata relation, cast, or type-test tables. Compatibility is exact
recursive equality. Array targets extend ordinary and optional shared target
vocabulary only with an exact non-polymorphic array case.

Element capability computation must handle recursive class/array graphs. A
cycle through an array edge is legal because backing is indirect, but default,
copy-construction, copy-assignment, and destruction availability may depend on
the complete strongly connected type graph. The compiler must use a
terminating fixed-point or equivalent capability solver rather than recursive
layout expansion.

Default capability has explicit shared cases. `shared C` is
default-initializable only when `C` is an exact concrete class with one
selected applicable zero-argument initializer; its operation is a distinct
`new C()` per element. An exact shared array target is default-initializable
as a distinct empty shared array allocation. Shared interface and `Obj`
targets are not default-initializable because no exact allocation class can be
selected. Optional shared owners are always default-initializable as absent
and select no target initializer or allocation.

## Syntax and resolution

The lexer recognizes `[` and `]` punctuation without changing the meaning of
existing tokens. Syntax retains:

- postfix array type suffixes and grouping;
- inline versus `new` construction;
- empty, default-length, and explicit-copy construction modes;
- element index versus slice, including independently omitted bounds;
- ordinary bracket projection versus `->` bracket projection; and
- exact spans for each bracket, colon, arrow, length, bound, and copy source.

`owner->[index]` and `owner->[start:end]` are source nodes for one explicit
shared dereference followed by an array projection. Resolution normalizes them
to one owner evaluation and one array-pointee projection without synthesizing
a source `*` span. `(*owner)[...]` reaches the same typed operation through the
general explicit-dereference form.

Resolution assigns an `ArrayTypeId` after resolving the exact element type. It
retains whether `shared` or `shared?` applies outside the array identity or
inside a grouped element type. It does not decide default capability,
copyability, access, index normalization, allocation, or storage.

Construction retains one of these source modes:

```text
Empty
DefaultLength(expression)
ExplicitCopy(source)
Elements(ordered destination plans)
```

It also retains inline versus shared allocation separately. Ordinary
default-length construction never falls back to explicit copy construction,
and `copy` in the dedicated position is not an ordinary element initializer.

## Element-list representation

The source forms `T[]{...}` and `new T[]{...}` add one distinct construction
mode implemented from syntax through native execution:

```text
Elements(ArrayElementList)
```

`ArrayElementList` retains the optional construction-level `new`, explicit
array type, both brace spans, every comma, ordered element expressions, and
complete construction span through its owning construction node. Resolution
assigns the exact recursive `ArrayTypeId` before resolving the elements and
mirrors their source order and punctuation without selecting lifecycle,
layout, or target operations. Empty braces retain empty element and comma
vectors and require no element capability. The public syntax and resolution
facades expose these phase nodes without exposing parser tokens.

Type checking now replaces every resolved element expression with one exact
stored-value initialization plan. The plan retains its element type and span;
the list retains brace and comma spans in source order. Direct exact-class
producers are distinguished from copied materialized sources, optional class
payload placement is distinguished from conditional copying, and nested arrays
and shared owners retain named-versus-produced transfer provenance.

Type checking treats each listed position as one previously uninitialized
owning destination of the array's exact stored element type. HIR records one
ordered destination-directed plan per element, selecting the applicable
primitive store, exact-class direct initialization or copy construction,
optional initialization, named nested-array deep copy, produced nested-array
adoption, named shared-owner copy, produced shared-owner adoption, or
optional-owner operation. A plan names every required initializer, copy
operation, shared target, nested array identity, and access decision. It never
recovers those semantics from expression shape below HIR.

The list does not require one uniform array-wide default or assignment plan.
Each source requires only its selected initialization operation. The canonical
array declaration nevertheless retains its independently computed default,
copy, assignment, and destruction capabilities for later operations on the
completed value.

Every element-list HIR-to-MIR plan implements this abstract sequence:

1. materialize the constant list count and perform checked backing allocation;
2. establish unpublished backing with an initialized prefix of zero;
3. evaluate each element expression exactly once in source order;
4. initialize the current backing slot through its selected owning plan;
5. advance the prefix only after that initialization completes normally; and
6. publish inline produced backing or one shared-array owner only when the
   prefix equals the list count.

MIR represents the executable element-list categories with
`AllocateElements`, which carries the constant source count and establishes a
zero `u64` prefix. Primitive
sources use one `InitializeElement` per position. Exact-class sources reuse
ordinary `Initialize`, object-result `Call`, `StringInitialize`, or
`CopyConstruct` operations against the final array-element place, followed by
`CompleteElement` only after that operation returns normally. Grouped produced
sources use ordinary full-expression temporary materialization and cleanup.
Inline optionals reuse their conditional payload operations. Nested inline
arrays deep-copy named sources through their exact recursive element copy plan,
or consume a completed produced descriptor through `Adopt`; `CompleteElement`
then advances the outer prefix. Shared-owner slots copy/retain named compatible
owners and adopt produced allocations, calls, casts, or exact shared-array
owners through the ordinary typed temporary transfer path. Optional shared
slots reuse zero-niche absence and conditional copy/adopt operations. Both
owner categories publish a completed slot before `CompleteElement`, and outer
inline/shared array ownership remains independent from every element strong
count. Every category finishes through ordinary inline or shared publication.
Element expression lowering may introduce explicit CFG, but it does not
introduce the uniform default/copy loop or live placeholders.

The MIR vocabulary may use immediate semantic positions, explicit index
storage, ordinary category-specific initialization instructions, or a focused
array initialization instruction. It may emit linear operations or structured
control flow. Those choices are private, but MIR must retain evaluation order,
full-expression temporaries, source consumption, and prefix state explicitly
enough for verification. It may not lower the form to default construction
followed by assignment.

Verification must prove:

- allocation precedes every element effect and failure reporting precedes the
  first element when allocation cannot complete;
- each position below the declared length is initialized exactly once in
  increasing order;
- the operation and source type match the exact stored element type and its
  selected lifecycle or ownership identity;
- named sources are copied and produced sources are consumed exactly as their
  category requires;
- uninitialized slots are never projected, copied, assigned, destroyed,
  borrowed, or published;
- only the complete prefix is published, and each produced backing or shared
  owner is consumed or cleaned exactly once; and
- completed element-expression temporaries and anchors remain in the enclosing
  full-expression plan.

The backend lowers verified element destinations through its existing
primitive, class, optional, nested-array, and shared-owner selection
machinery. The list count is source-derived but does not imply static data,
stack storage, unrolled machine code, or a new descriptor layout. The C runtime
continues to allocate and free checked byte blocks without knowing element
types, list expressions, initialized prefixes, or lifecycle operations. No
public runtime entry point, metadata format, or ABI-version change is part of
element-list construction.

Syntax, resolved IR, and HIR implement the complete typed representation.
Verified MIR and x86-64 implement every legal element-list family for inline
and shared outer ownership. The verifier proves exact values, class and
conditional payload destinations, presence-before-prefix ordering, nested
array identity and produced-source consumption, compatible shared targets,
named-versus-produced owner accounting, source-position order, normally
completed construction before prefix advancement, publication, backing
consumption, and storage lifetime. The
[status matrix](../language/STATUS.md) remains the concise availability view.

## Typed HIR

HIR must contain explicit semantic array operations rather than desugaring
brackets to method names. At minimum it distinguishes:

- empty inline production and empty shared allocation;
- default-length inline production and shared allocation;
- per-element default shared allocation with one selected exact initializer,
  or absent optional shared initialization without allocation;
- explicit inline and shared deep-copy construction;
- named inline deep-copy sources and produced backing sources;
- inline backing adoption into an owning destination;
- shared owner copy, adoption, release, and optional-owner unwrap;
- immutable length;
- checked element place selection;
- copied slice production;
- checked slice assignment;
- whole inline array assignment and destruction;
- whole-array and element alias sources; and
- inline-backing, shared-owner, or optional-owner anchor requirements.

Every operation records its exact `ArrayTypeId`, element type, source span,
access, and selected element lifecycle capabilities. HIR element places retain
their backing provenance so aliases and later owning consumers do not recover
lifetime facts from expression shape.

HIR distinguishes a named inline array place from a produced array owner.
Named initialization, value arguments, returns, explicit copying, and
whole-array assignment select recursive deep copy. A produced inline array
selects backing adoption in every owning destination. Explicit
`T[](copy source)` remains a deep-copy operation even though its completed
result is itself a producer that a later destination may adopt.

Element reads and writes are category-specific semantic operations:

- primitive load/store;
- exact-class checked place, copy construction, or copy assignment;
- nested-array place, deep copy, adoption, or whole-array assignment; and
- shared-owner copy/adopt/secure-replace operations.

Shared bracket projection consumes explicit dereference in HIR just as shared
object member access does. Lower phases see one checked array-pointee place
with stable or anchored owner provenance; they do not implement a second arrow
indexing pipeline.

The typed frontend materializes these operations directly. Supplied indices
and bounds are checked as exact `i64`, omitted bounds remain absent in HIR,
slice reads carry their element copy plan, and destinations carry distinct
whole-replacement, element-write, or equal-length slice-write plans. Receiver,
bound, and source evaluation order, terminating failure reasons, access, and
the required inline/shared/optional anchor category are all explicit. Every
typed operation crosses the verified MIR boundary before x86-64 instruction
selection.

## MIR storage and operations

MIR remains target-independent and must not contain target byte offsets or a
runtime array-kind tag. It requires explicit storage roles for:

- owning inline array descriptors;
- produced inline array temporaries;
- unpublished inline or shared backing allocations;
- shared and optional shared array owners;
- copied slice temporaries;
- construction and copy destinations;
- hidden inline-backing anchors; and
- hidden shared-owner anchors.

The executable vocabulary must represent:

- checked element-count and allocation-size calculation;
- backing allocation and deallocation;
- initialized-prefix advancement and completed publication;
- default element construction in increasing index order;
- exact per-element shared allocation, initialization, publication, and
  produced-owner adoption;
- element copy construction in increasing index order;
- element copy assignment in increasing destination-index order;
- element destruction in decreasing index order;
- inline backing adoption and owner release;
- shared strong-owner copy/adopt/release;
- index and slice-bound normalization without signed overflow;
- bounds and slice-length failure edges;
- checked element projection;
- copied slice construction; and
- whole and slice assignment with explicit temporary and cleanup boundaries.

An unpublished backing is not a source-visible array value. Publication occurs
only after every required element is initialized. MIR may use an initialized
prefix while constructing or copying, but no element outside that prefix is a
live value and the prefix is never exposed as an optional or partially
initialized array.

Default initialization of a `shared C` slot reuses ordinary exact-class shared
allocation in the element loop. MIR allocates one unpublished `C`, invokes the
selected zero-argument initializer, publishes one produced owner, and adopts
that owner into the current slot before advancing the initialized prefix.
Default initialization of `shared? C` instead writes the absent zero state and
creates no allocation. A default shared-array element publishes and adopts one
distinct empty shared-array allocation.

### Named copy and produced adoption

For a named inline source, MIR allocates a distinct destination backing and
copy-constructs every element before publishing it. Whole assignment secures
that complete new value before releasing the destination's old backing.
Direct and indirect self-assignment therefore cannot invalidate the source.

For a produced inline source, MIR transfers its one backing ownership account
into the destination. The producer storage becomes consumed compiler state,
not a source-visible moved-from value, and must not later destroy or release
that backing. Assignment secures the producer first, installs it, then releases
the old destination. This transfer invokes no element copy or destruction for
the incoming array.

Value parameters use caller-created owning array storage. A named argument is
deep-copied at its left-to-right argument position. A produced argument
transfers its backing into the parameter. The callee destroys the parameter in
normal reverse parameter cleanup. Results use one caller-owned destination:
named returns deep-copy, while produced returns transfer their backing after
callee cleanup has preserved the completed result.

### Whole assignment

Whole inline assignment ends ownership of the old backing and installs a
complete new backing, so source and destination lengths need not match. It is
not element-wise copy assignment. An owning local, value parameter, or
supported field may be a destination. An alias root may not be rebound.

The destination place is selected before the right side. A named right side is
fully deep-copied; a produced right side is fully completed and secured. Only
then does the destination end its old backing-owner account. Without a hidden
element-borrow anchor, old elements are destroyed in decreasing index order
and the backing is released immediately. With an anchor, both destruction and
release wait until its final dependent borrow ends.

Shared owner assignment remains ordinary secure-before-release handle
assignment. MIR has no whole-shared-array-pointee replacement operation.

## Index and slice lowering

Lengths remain `u64` in target-independent IR. User-provided indices and slice
bounds remain `i64`; the compiler must not silently cast them to `u64` before
negative normalization.

Normalization branches on the sign and computes a checked position without
negating the minimum `i64`. Every accepted array length is at most
`i64::MAX`, so a valid normalized position is representable in both signed and
unsigned forms. Element projections require a position strictly below length;
slice positions may equal length.

The checked projection operation carries the exact backing, normalized index,
element type, access, and anchor dependency. Backends may fold checks or
address arithmetic only after preserving the required failure edge and
single-evaluation behavior.

A slice read allocates a new inline backing and copy-constructs elements in
increasing order. A right-side slice in slice assignment is therefore an
ordinary owning temporary completed before destination writes. This gives
overlap snapshot semantics and fixes lifecycle effects for nontrivial
elements.

Slice assignment verifies destination bounds, completes and verifies its
source, and checks equal element counts before its first write. It then invokes
the selected element assignment operation in increasing destination order.
Primitive copies may use an overlap-safe bulk operation. Shared-owner copies
must secure incoming owners before releasing overwritten owners. Class and
nested-array operations must preserve their selected lifecycle calls and
temporary cleanup.

An optimization may fuse slice production and assignment only when the
observable execution remains identical, including copy construction,
assignment, destruction, order, alias behavior, bounds failure, and allocation
failure. Correctness never depends on such fusion.

## Backing lifetime and anchors

Normal inline copying never shares backing. Hidden backing anchors exist only
to preserve a borrowed array or element place across a call when another
nonexclusive path can replace the owning array value.

An inline backing therefore has two conceptual lifetime accounts:

- exactly one source-visible inline owner while attached to a live array
  value; and
- zero or more compiler-generated borrow anchors.

Replacing or destroying the inline owner ends its account and logically
destroys the array value. If anchors remain, the backing and the element
lifetimes required by those borrowed places remain available until the final
anchor ends. No source operation can retain such an anchor, observe its count,
or turn it into a shallow array copy.

An alias to a whole stable array descriptor continues to observe replacement
of that descriptor. An alias to an element or nested backing remains tied to
the selected backing and requires the backing anchor described above.

Shared array places reuse ordinary shared-owner anchoring. Stable shared local
and value-parameter owners borrow directly; replaceable fields and produced
owners use copied or adopted hidden owners. Optional shared array unwrap first
secures one ordinary non-null owner. Each anchor remains live through bound
evaluation, right-side evaluation, selected element lifecycle work, and the
complete call or immediate checked consumer.

Within the structured short-circuit MIR representation, produced arrays,
partially initialized backings, element lifecycle state, aliases, and anchors
remain attached to the path condition that established them. Conditional
full-expression cleanup releases only the selected arrays and ends only the
selected anchors, in reverse completion order; the skipped alternative
performs no allocation, bounds check, element operation, release, or anchor
operation.

## Target layout direction

Exact descriptor and allocation bytes are target implementation details, but
every representation must support these invariants:

- an inline array value owns one immutable length and either no backing for an
  empty value or one backing ownership account;
- an empty inline array is a complete valid value even if represented with a
  zero pointer niche;
- a nonempty inline backing carries enough state for hidden anchors and exact
  element destruction;
- a shared array owner is one non-null handle to an allocation with a strong
  count, immutable length, exact element finalizer information, and aligned
  element storage;
- a shared empty array still has a distinct non-null allocation identity;
- element addresses satisfy the target alignment of the exact element type;
  and
- all header, padding, stride, and total-size arithmetic is checked before
  allocation or projection.

The compiler may place length in the owning descriptor, the backing header, or
both when consistency is verified. It may place element data in the same
allocation as the header or in a separately owned block. It may share an
empty-array sentinel among inline values because no source-visible identity or
element storage exists. These choices cannot introduce nullable source values
or source-visible shallow inline copies.

Generated helpers should be specialized by canonical `ArrayTypeId` and
selected element lifecycle plan. Primitive arrays may lower to compact loops
or bulk operations. Class, nested-array, shared-owner, and optional elements
require exact typed operations; no erased runtime element-kind switch may
replace those semantics.

On x86-64, both an ordinary shared-owner element and an
optional shared-owner element occupy one eight-byte slot. The optional form
uses zero for absence and needs no separate tag. A present slot points to its
independently allocated pointee; that pointee's header and payload are not part
of the array slot. Thus a 1,000-element shared or optional-shared payload uses
8,000 element bytes plus the one outer array header and alignment padding.
Default non-optional shared construction additionally performs one pointee
allocation per element, while default optional shared construction performs
none.

### x86-64 inline layout

An executable inline descriptor is one aligned eight-byte backing pointer.
Zero is the complete allocation-free empty representation and implies length
zero. A nonzero descriptor points to one allocation:

| Offset | Width | Meaning |
|---:|---:|---|
| 0 | 8 | inline-owner plus backing-anchor account count |
| 8 | 8 | immutable `u64` element length |
| 16 | varies | first element, aligned for its exact element type |

Element stride is the aligned target size of the exact element: one or eight
bytes for primitives, the complete optional or class layout for those values,
and one descriptor word for a nested inline array. An array field inside a
class is always one descriptor word, so a recursive `Node`/`Node[]` edge does
not recursively expand layout. Header, element alignment, stride, length
multiplication, and total allocation size are checked before `ska_rt_alloc`.
Length never exceeds `i64::MAX`; a stricter arithmetic ceiling applies when
stride and header cannot fit in `u64`. Allocation failure remains the runtime
allocator's existing unsuccessful-termination contract.
The frozen common-reporting policy retains distinct allocation-request,
bounds, and slice-length termination reasons and maps them centrally to the
sole [language message catalog](../language/ERRORS.md#frozen-panic-design).
Array MIR and this contract do not embed copies of those bytes.

Generated initialization, copy-element, whole-clone, destroy-element, release,
and exact-class copy wrappers have deterministic private symbols specialized
by canonical identities. Construction and copying visit increasing indices;
release destroys decreasing indices before freeing the backing. Exact-class
operations invoke the selected user or synthesized lifecycle, optionals branch
on presence, and nested arrays recursively clone or release their descriptors.
Named synthesized field copying uses the clone helper. Produced adoption
installs the existing descriptor without invoking a copy helper. The zero
descriptor performs no runtime call. The count and header length retain the
state required by detached backing anchors without making empty arrays
allocate.

An inline-backing anchor increments the same header account used by the
source-visible owner. Whole replacement releases only the visible owner
account; destruction and deallocation therefore wait until the final hidden
anchor releases its account. A checked class or nested-array element alias
uses a separate compiler-owned address carrier so its selected address is
captured at argument or receiver evaluation rather than recomputed after later
effects. A whole inline-array alias continues to carry the source descriptor
address and consequently observes descriptor replacement. Shared whole-array
aliases use a hidden inline-layout-compatible descriptor pointing into the
already-secured shared allocation; the strong owner anchor remains responsible
for lifetime.

Inline-backing anchor retain reports `ownership count overflow` when the
account is already `u64::MAX`. A zero count on a nonempty backing contradicts
verified lifetime state and remains a hard trap. Shared-owner array elements
use the generic shared retain contract: `u64::MAX - 1` reports exhaustion,
the verified `u64::MAX` immortal sentinel is a no-op, and null or zero-count
live handles hard-trap.

### Initial x86-64 shared-outer layout

A `shared T[]` or present `(shared T[])?` owner is one non-null handle word.
Every construction allocates one contiguous exact array block, including a
zero-length construction:

| Offset | Width | Meaning |
|---:|---:|---|
| 0 | 8 | strong owner count |
| 8 | 8 | exact array metadata/finalizer table pointer |
| 16 | 8 | immutable `u64` element length |
| 24 | varies | first element, aligned for its exact element type |

Count and metadata are published only after the increasing initialized prefix
is complete. The metadata selects an exact `ArrayTypeId` finalizer; last-owner
release destroys elements in decreasing index order and then the generic
shared release path frees the same outer block. Zero remains reserved for
optional absence and is never an ordinary shared array value.

Local element access evaluates the descriptor and exact-`i64` index once. A
negative index adds the `u64` length once using wrapping machine arithmetic;
the following unsigned comparison rejects both an excessive negative
magnitude, including `i64::MIN`, and every position at or beyond the length.
Only the successful control-flow edge may form
`backing + element_offset + normalized_index * stride`. Primitive loads and
stores use their exact width; class, optional, nested, and subsequent field
projections reuse the aligned element address and selected lifecycle
operation. The checked-position MIR and its reason-specific terminating edge
remain layout-independent.

## MIR verification

The array verifier must establish at least:

- every array type identity is declared, canonical, recursively well formed,
  invariant, and legal in its ownership position;
- every construction length is `u64`, every user index/bound is `i64`, and
  every normalized position is produced by a compatible checked operation;
- no allocation uses unchecked element count, stride, alignment, padding, or
  byte-size arithmetic;
- unpublished storage is never read, aliased, returned, or released as a
  completed array;
- every completed backing has exactly its declared length of live elements;
- initialized-prefix advancement agrees with construction order;
- every default non-optional shared element names a concrete exact target,
  selects one compatible zero-argument initializer, publishes one produced
  owner, and adopts it exactly once into the next prefix slot;
- default optional shared elements enter the absent state without allocation
  or an ordinary owner operation;
- named inline sources deep-copy and produced sources transfer exactly one
  backing account;
- consumed produced storage cannot be read, copied, destroyed, or released
  again;
- every normal inline local and value-parameter lifetime ends with exactly one
  owner release, caller argument storage transfers exactly once, and every
  array result is initialized on each normal return path;
- array initialization, replacement, release, argument transfer, and result
  state agree at control-flow joins and across repeated storage epochs;
- element destruction proceeds in decreasing index order, immediately or after
  the final anchor as required;
- shared owner copy/adopt/release state agrees at control-flow joins and absent
  optional handles never enter ordinary owner operations;
- every element or slice place has compatible type, access, bounds evidence,
  and a live descriptor, backing, or owner anchor;
- slice assignment completes all checks before its first write and uses an
  exact equal-length source;
- element construction and assignment capabilities match the operation;
- all hidden anchors end after their last dependent place and before their
  storage is released;
- alias roots are never rebound through whole-array assignment;
- whole shared pointee assignment has no MIR form; and
- every array failure edge terminates without producing a value or joining a
  successful ownership state.

Array verification extends the existing initialized-storage and ownership
analyses. A backend must reject unsupported verified array MIR structurally
until its complete layout and lowering are implemented.

## Internal ABI and runtime boundary

Arrays remain excluded from external declarations. No C array layout,
parameter/result convention, element ownership transfer, or foreign lifetime
contract is frozen.

The internal ABI must preserve language ownership independently of physical
register or stack choices:

- named inline value arguments arrive as distinct deep-copied array owners;
- produced inline arguments transfer one backing account;
- inline results use one caller-owned result destination and the same
  named-copy versus produced-transfer distinction;
- `ref` and `mut ref` array parameters are non-owning checked places with any
  required hidden anchor; and
- shared and optional shared arrays use ordinary non-null and optional shared
  owner conventions extended to exact array targets.

The existing public runtime allocation boundary is sufficient:

```c
void *ska_rt_alloc(uint64_t byte_count);
void ska_rt_free(void *allocation);
```

The compiler and generated code own array headers, length, checked index
normalization, element operations, backing anchors, strong counts, slice
loops, finalizers, and cleanup. The C runtime must not learn array type
identities, element kinds, reference scanning, lifecycle callbacks, bounds,
or slice semantics. The currently implemented design therefore requires no
new public C symbol or runtime ABI version change. Central termination
lowering calls the version-9 panic reporter for array failure reasons without
exposing an array layout or adding an array-specific helper. Any other
implementation that needs a new public symbol must revise
this contract and the versioned runtime boundary before relying on it.

## Diagnostics, dumps, and tests

Diagnostics must distinguish at least malformed syntax, illegal element type,
ownership grouping mistakes,
non-default-initializable elements, unavailable copy or assignment capability,
wrong length/index/bound type, raw shared indexing, optional owner use before
unwrap, invalid alias rebinding, and unsupported whole shared-pointee
assignment. A non-defaultable shared element diagnostic must distinguish a
concrete class without an applicable zero-argument initializer from an
interface or `Obj` target with no exact allocation class. Runtime failures
remain reason-distinct in MIR even when the x86-64 target uses the
same unsuccessful process boundary.

Deterministic syntax, resolved, HIR, and MIR dumps must expose recursive array
identity, outer ownership, construction mode, exact element operation,
named-versus-produced provenance, adoption, bounds shape, slice mode, access,
and anchor category without exposing unstable private table addresses.

Focused implementation tests must cover:

- recursive syntax, grouping, recovery, spans, and nesting limits;
- canonical type identity, invariance, element eligibility, and capabilities;
- every default element category and empty non-defaultable arrays, including
  distinct per-slot allocations for non-optional shared class and array
  targets, zero-allocation absent optional owners, and rejected
  interface/`Obj` targets;
- named deep copies and produced backing adoption in locals, fields,
  assignments, arguments, results, and temporaries;
- self-assignment, indirect alias overlap, cleanup order, and lifecycle-visible
  copy/assignment/destruction effects;
- shared and optional shared arrays, owner copy/adopt/release, last-owner
  finalization, and explicit `*`, `->member`, and `->[...]` projection;
- zero, positive, negative, boundary, minimum-`i64`, and out-of-range indices;
- omitted, negative, empty, reversed, and mismatched slices;
- overlapping copied slices and full-range assignment;
- nested jagged arrays with inline, shared, and optional shared edges;
- whole-array, element, nested-element, shared-backed, and optional-backed
  alias anchors;
- byte arithmetic, maximum length, allocation, bounds, and count failures;
- malformed MIR rejection for every verifier invariant;
- internal register/stack/result pressure and generated helper legality;
- deterministic diagnostics and phase dumps; and
- source-to-native success, compile-failure, and runtime-failure goldens.

## Private implementation choices and deferred extensions

The implemented contract leaves these non-semantic implementation choices
private:

- generated helper granularity, visibility, and inlining thresholds;
- which trivial operations use loops, `memcpy`, or `memmove`;
- exact diagnostic codes and dump field spelling; and
- optimization of empty arrays, bounds checks, copies, and slice temporaries.

The compiler design also excludes the language extensions listed in
[Arrays](../language/ARRAYS.md#deferred-extensions), including richer element
initialization, slice views, resizing, iteration protocols, external ABI,
recoverable failures, and concurrency. The separately frozen
[optional-array direction](OPTIONAL_VALUES.md#array-composition-and-runtime-boundary)
does not alter current availability; it will reuse canonical array identities,
lifecycle plans, descriptors, and helpers rather than infer a second array
model from the current representation.
