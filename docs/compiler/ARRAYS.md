# Array Compiler and Runtime Contract

Status: **frozen design; source syntax, canonical resolution, and construction
HIR implemented**.
This document is
authoritative for the proposed compiler representation, lowering,
verification, target, and runtime responsibilities required by the
[array language contract](../language/ARRAYS.md). The compiler now types array
owners and construction into explicit HIR, but it does not lower arrays to MIR
or execute them. Availability remains authoritative in the
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

The implemented HIR boundary includes exact canonical array types in
declarations and owning positions, inline/shared construction mode,
default/copy/assignment/destruction element plans, and named deep-copy versus
produced-backing adoption. A fixed-point analysis propagates unavailable copy
operations through recursive class/array graphs while treating array backing
as a finite-containment boundary. Indexing, slicing, whole replacement, and
array aliases remain at the structured `TYP035` type-checking gate. Programs
whose valid typed HIR contains arrays stop at the separate deliberate
HIR-to-MIR `TYP035` driver gate; direct MIR lowering asserts the same boundary.

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

`shared T[]` and `(shared T)[]` must resolve differently. The former is a
shared owner whose target is an exact array identity; the latter is an inline
array identity whose element is a shared owner. `shared? T[]` wraps shared
array ownership in absence. No phase may represent it as an inline optional
array payload.

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

The planned lexer adds `[` and `]` punctuation without changing the meaning of
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
```

It also retains inline versus shared allocation separately. Ordinary
default-length construction never falls back to explicit copy construction,
and `copy` in the dedicated position is not an ordinary element initializer.

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

On the initial x86-64 target, both an ordinary shared-owner element and an
optional shared-owner element occupy one eight-byte slot. The optional form
uses zero for absence and needs no separate tag. A present slot points to its
independently allocated pointee; that pointee's header and payload are not part
of the array slot. Thus a 1,000-element shared or optional-shared payload uses
8,000 element bytes plus the one outer array header and alignment padding.
Default non-optional shared construction additionally performs one pointee
allocation per element, while default optional shared construction performs
none.

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
- every normal inline lifetime ends with one owner release and decreasing-index
  element destruction, immediately or after its final anchor as required;
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
or slice semantics. The frozen design therefore requires no new public C
symbol or runtime ABI version change. An implementation that later needs a
new public symbol must revise this contract and the versioned runtime boundary
before relying on it.

## Diagnostics, dumps, and tests

Diagnostics must distinguish at least malformed syntax, illegal element type,
unsupported inline optional array payload, ownership grouping mistakes,
non-default-initializable elements, unavailable copy or assignment capability,
wrong length/index/bound type, raw shared indexing, optional owner use before
unwrap, invalid alias rebinding, and unsupported whole shared-pointee
assignment. A non-defaultable shared element diagnostic must distinguish a
concrete class without an applicable zero-argument initializer from an
interface or `Obj` target with no exact allocation class. Runtime failures
remain reason-distinct in MIR even when the initial native target uses the
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

## Deferred implementation choices and extensions

The frozen contract intentionally leaves these non-semantic implementation
choices to the eventual roadmap and implementation:

- exact descriptor/header field order, widths beyond required value ranges,
  padding, and whether header and data share one allocation;
- generated helper naming, granularity, visibility, and inlining thresholds;
- which trivial operations use loops, `memcpy`, or `memmove`;
- exact diagnostic codes and dump field spelling; and
- optimization of empty arrays, bounds checks, copies, and slice temporaries.

The compiler design also excludes the language extensions listed in
[Arrays](../language/ARRAYS.md#deferred-extensions), especially inline optional
array payloads, richer element initialization, slice views, resizing,
iteration protocols, external ABI, recoverable failures, and concurrency.
Those require explicit language and compiler contract revisions rather than
being inferred from the initial representation.
