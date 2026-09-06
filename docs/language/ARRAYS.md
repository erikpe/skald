# Arrays

Status: **implemented contract on x86-64, including explicit element-list
construction**. This document is authoritative for the
source-visible array contract. The
[status matrix](STATUS.md) is authoritative for compiler availability, and the
[implemented grammar](GRAMMAR.md) remains the exact syntax currently accepted
by the compiler. Array forms receive canonical recursive identities during
resolution and exact lifecycle, place, slice, alias, and transfer plans in HIR
and verified MIR. The x86-64 target executes empty or dynamically
sized inline and shared-outer arrays of primitives, optionals, exact classes,
and recursively nested inline arrays. It includes immutable `len()`, checked
positive and negative-relative element access, named deep copy, explicit
shared-copy construction, produced-backing adoption, arbitrary-length inline
replacement, conditional optional lifecycle, class fields, internal value and
shared-owner parameters/results, checked allocation, copied slices, checked
equal-length slice assignment, and deterministic reverse destruction.
Ordinary and optional shared-owner elements execute with
one-word slots, exact per-element default allocation, conditional ownership,
secure replacement, shallow owner copying, and nested shared-array edges.
Slice bounds support omitted endpoints and signed negative-relative positions;
slice reads own distinct backing and overlapping writes have snapshot
semantics. Call-scoped read-only and mutable whole-array aliases, exact-class
element aliases, and nested-array element aliases execute with hidden backing
or shared-owner anchors.

Arrays are built-in, invariant, fixed-size sequences with an exact element
type. They may be inline values or shared allocations, may contain every
supported owning element category, and may nest recursively as jagged arrays.
The [array compiler contract](../compiler/ARRAYS.md) defines phase ownership,
storage, lowering, verification, and runtime responsibilities without making
those details source-visible.

## Core model

An array value has one immutable length and that many live, ordered elements.
Its length cannot change while that array value and backing allocation remain
the same. An owning inline array place may nevertheless be assigned another
array value of a different length: assignment replaces the complete backing
owned by that place rather than resizing the old backing. A shared array
allocation never changes length.

Arrays are typed recursively. `T[]` and `U[]` are different types unless `T`
and `U` are exactly the same type. Class inheritance, interface conformance,
and shared-target compatibility do not make array types covariant.

An array is not a class, interface, or `Obj`. It has no dynamic class, does not
participate in object casts or type tests, and does not acquire structural
behavior from methods with particular names. Construction, `len()`, indexing,
slicing, and shared array projection are intrinsic array operations.
The separately implemented
[structural indexing and slicing protocol](INDEXING_AND_SLICING.md) preserves
this precedence: class/interface sugar does not replace these operations or
make protocol-named array methods part of the array surface.

Array storage is an indirection boundary. Inline array elements belong to the
array's deterministic lifetime, but their dynamically sized backing does not
become inline class storage merely because the array value is inline. This
permits recursive declarations such as a class containing `Node[]` without
creating an infinitely sized inline class.

## Type and ownership forms

Postfix `[]` constructs an array type. Leading `shared` applies to the complete
following array type; `shared?` is shorthand for an optional owner of that
complete type.
Parentheses place ownership inside the element type:

| Type | Meaning |
|---|---|
| `T[]` | Inline array value containing inline `T` elements |
| `shared T[]` | Shared owner of one array allocation containing inline `T` elements |
| `(shared T)[]` | Inline array whose elements are non-null shared owners of `T` objects |
| `shared (shared T)[]` | Shared array whose elements are shared owners of `T` objects |
| `(shared T[])?` | Optional shared owner of a `T[]` array allocation (`shared? T[]` shorthand) |
| `((shared T)?)[]` | Inline array whose elements are optional shared owners of `T` objects (`(shared? T)[]` also accepted) |

The same rule composes recursively. `shared T[][]` is a shared outer array
whose elements are inline `T[]` values. `(shared T[])[]` is an inline array
whose elements are shared array owners.

Legal element types are:

- primitive value types;
- exact inline class types;
- supported inline optional primitive or exact-class types;
- ordinary or optional shared owners, including shared array owners; and
- inline array types recursively.

`unit`, bare interface and `Obj` views, aliases, and function types are not
array element types. Optional inline arrays are valid elements and default to
outer absence. The
[compositional optional profile](OPTIONAL_VALUES.md#compositional-optional-types)
defines `T[]?` and `(T[])?` as equivalent spellings for that tagged owning
value. This does not change `(shared T[])?` and its
`shared? T[]` shorthand, whose absence belongs to the shared owner rather than
to an inline optional array payload.

## Construction and default initialization

The implemented construction forms are:

```ska
T[]()
T[](length)
T[](copy source)

new T[]()
new T[](length)
new T[](copy source)
```

`T[]()` constructs an empty inline array. It is valid for every legal element
type because it constructs no elements. `new T[]()` constructs a distinct
non-null shared empty-array allocation.

`T[](length)` constructs an inline array and `new T[](length)` constructs a
shared array. The length expression is evaluated exactly once and must have
type `u64`. Both forms require `T` to support default initialization and
default-initialize elements in increasing index order.

Default initialization is a type capability rather than an implication that
initializer-free locals or fields become legal:

| Element type | Default element value |
|---|---|
| `i64`, `u64`, `u8` | `0`, `0u`, or `0u8` respectively |
| `f64` | Positive binary64 zero, spelled `0.0` |
| `bool` | `false` |
| Supported inline `T?` | `none` |
| Supported `(shared T)?` | `none` |
| Exact class `T` | One ordinary zero-argument `T()` construction |
| Inline array `T[]` | A valid empty `T[]` value |
| `shared T`, where `T` is a concrete exact class with an applicable zero-argument initializer | One distinct `new T()` allocation |
| Shared array owner `shared T[]` | One distinct empty `new T[]()` allocation |
| `shared Interface` or `shared Obj` | Not default-initializable |

An exact class without an applicable zero-argument ordinary initializer is not
default-initializable. A non-optional shared class element is
default-initializable exactly when its statically named concrete class has
such an initializer. Each element is a fresh allocation; default construction
does not copy one owner into every slot or choose a runtime implementation for
an interface or `Obj` target.

A shared array element has an exact allocation target and defaults to a fresh
shared empty array. This does not require its leaf element type to be
default-initializable because the new inner array contains no elements.

When the default plan for an exact inline or shared class element selects a
private zero-argument initializer, the array construction is valid only when
its source call site satisfies the authoritative
[declaring-class privacy rule](CLASSES_AND_LIFECYCLE.md#declaring-class-privacy).
The capability plan remains a stable property of the element type; access is
checked when `T[](length)` or `new T[](length)` consumes it. Empty and
explicit-copy array construction do not select or authorize an ordinary
initializer.

These rules are invoked only by an explicit array construction requesting
default elements. They do not make initializer-free locals or fields legal.
The array itself remains empty-constructible even when its element type is not
default-initializable:

```ska
var empty: NoDefault[] = NoDefault[]();     // valid
var values: NoDefault[] = NoDefault[](4u); // invalid
```

Consequently, non-optional and optional shared elements have deliberately
different allocation behavior:

```ska
// One shared outer-array allocation plus 1,000 distinct T allocations.
var shared_values: shared (shared T)[] =
    new (shared T)[](1000u);

// One shared outer-array allocation; every element is none.
var maybe_shared_values: shared (shared? T)[] =
    new (shared? T)[](1000u);

// One inline backing allocation plus 1,000 distinct T allocations.
var inline_values: (shared T)[] =
    (shared T)[](1000u);

// One inline backing allocation; every element is none.
var maybe_inline_values: (shared? T)[] =
    (shared? T)[](1000u);
```

The two non-optional forms are valid only when `T` is a concrete class with an
applicable zero-argument initializer. The optional forms do not inspect or
invoke any `T` initializer.

The length of every constructed array must be at most the largest `i64` value.
This keeps every valid element and boundary position expressible by the signed
index type. Element-count, layout, and allocation-size overflow, or inability
to allocate the required backing, terminates unsuccessfully. Common reporting
assigns these source-reachable failures their distinct reasons from the sole
[panic message catalog](ERRORS.md#frozen-panic-design).

`T[](copy source)` explicitly produces an inline deep copy.
`new T[](copy source)` produces a distinct shared allocation containing a deep
copy. The source must designate an exact `T[]` array place or value; array
copying has no inheritance or dynamic-check relation.

No fill-value, inferred array literal, or multi-dimensional shape constructor
is implemented. Executable nonempty construction accepts the existing
default-length and exact-copy modes plus explicit element lists for every
supported stored element category. The frontend accepts, resolves, and type-
checks the [frozen indexed direction](#frozen-indexed-array-construction), but
a deliberate executable-lowering gate keeps that dynamic direct-initialization
form out of MIR.

## Explicit element-list construction

The compiler implements the following source forms:

```ska
T[]{element0, element1}
new T[]{element0, element1}
```

`T[]{...}` produces one owning inline array. `new T[]{...}` produces one
non-null shared owner of a shared outer-array allocation. The explicit
`array-inline-type` determines the exact invariant element type before any
element is checked. Braces accept zero or more comma-separated expressions;
the grammar does not accept a trailing comma. `T[]{}` is equivalent in
value and lifecycle to `T[]()` without deprecating that implemented empty
form.

This is **element-list construction**, not an inferred array literal. Untyped
`[element0, element1]`, expected-type-only lists, and
`T[](element0, element1)` are not accepted forms. In particular, the existing
single-expression `T[](value)` remains default-length construction and
requires `value: u64`.

The type checker accepts both brace forms and records one exact ordered
destination plan per element in HIR. Lists of `i64`, `u64`, `u8`, `f64`,
`bool`, exact classes, supported inline optionals, recursively nested inline
arrays, shared owners, and optional shared owners execute for inline and shared
outer arrays. Their MIR
allocates checked unpublished backing before element effects, advances one
verified ordered prefix, and publishes only after completion. Exact-class
plans use ordinary direct initialization, object-result placement, or selected
copy construction in the final slot. Inline optionals reuse ordinary absence,
injection, conditional payload copying, direct payload placement, and presence
publication. A named nested array is deep-copied recursively; a produced nested
array transfers its completed backing into the current outer slot exactly once.
Shared-owner positions retain named compatible owners and adopt produced
owners; optional shared-owner positions apply the same operation only when
present and preserve the ordinary zero niche for absence.

The list length is the number of supplied expressions and must satisfy the
ordinary maximum-`i64` array-length bound. Type grouping and outer ownership
retain their existing meanings, so the form composes with nested, shared, and
optional elements:

```ska
var rows: i64[][] = i64[][]{
    i64[]{1, 2},
    i64[]{3, 4, 5}
};

var owners: (shared Item)[] = (shared Item)[]{
    new Item(1),
    new Item(2)
};

var maybe: i64?[] = i64?[]{none, 10, none};
```

Nested arrays remain jagged. `(shared T[])?` is an optional shared owner of one
complete array, while `((shared T)?)[]` is an array of optional shared owners.
The corresponding `shared?` forms remain accepted shorthand. Element-list
construction adds no optional inline-array payload and no
array covariance, common-supertype search, or implicit numeric conversion.

### Allocation, evaluation, and initialization

Abstract execution is ordered as follows:

1. determine the element count from the source list;
2. allocate one unpublished inline or shared outer backing for that count;
3. if allocation cannot complete, report the existing allocation failure
   before evaluating any element expression;
4. evaluate each element expression exactly once from left to right;
5. completely initialize that expression's previously uninitialized slot
   before evaluating the next expression; and
6. publish the complete array only after every slot is live.

An element position is an owning initialization destination of the declared
stored element type. It is not a live default value followed by assignment.
Consequently element-list construction does not require the element type's
default or copy-assignment capability. Each expression requires only the
operation selected for that source:

- a primitive expression stores its exact value;
- an eligible ungrouped fresh exact-class construction initializes its slot
  directly through the selected accessible ordinary initializer;
- an eligible exact-class result uses the slot as its final result destination;
- an existing or otherwise materialized exact-class source copy-constructs the
  slot;
- `none` or a present optional source uses ordinary optional initialization;
- a named inline-array source deep-copies while a produced inline-array source
  transfers its backing into the nested slot; and
- a named shared owner copies/retains one owner while a produced shared owner
  transfers/adopts it, including the corresponding optional-owner cases.

These are the ordinary target-directed stored-value rules. They add no
array-specific conversion. Grouping retains its existing effect on exact-class
materialization and copy elision. A named shared owner listed twice makes both
slots own the same allocation; two separate `new` expressions produce two
allocations.

The resulting array type retains its independently computed default, copy,
assignment, and destruction capabilities. Constructing one array value does
not make a later named deep copy, slice copy, element assignment, or other
operation available when its required element capability is absent. A
completed produced element-list array may nevertheless transfer its backing
to its immediate owning destination under the ordinary adoption rule.

Ordinary full-expression lifetime remains in force. A completed temporary
from one element expression remains live through the enclosing full-expression
boundary unless an existing immediate-consumer rule ends it sooner. Directly
initialized element storage belongs to the unpublished backing rather than to
the temporary sequence.

### Publication, cleanup, and failure

Construction maintains one increasing initialized prefix. Slots below the
prefix are complete live values, the next slot remains incomplete until its
selected initialization returns normally, and later slots are uninitialized
storage. Uninitialized slots may not be read, copied, assigned, destroyed,
borrowed, or published. Only a prefix equal to the list length may become a
source-visible produced array or shared-array owner.

Once published, the array follows every ordinary copy, adoption, anchor,
replacement, reverse element-destruction, and backing-release rule in this
document. Current panic and allocation failures remain non-returning and
non-unwinding, so no source-level cleanup is guaranteed for an unpublished
prefix after reporting begins. Any future recoverable construction failure
must clean already initialized elements without treating uninitialized slots
as live.

The compiler representation and unchanged runtime boundary are defined in the
[array compiler contract](../compiler/ARRAYS.md#element-list-representation).

## Frozen indexed array construction

The frozen next array-construction form is:

```ska
T[](length; index => expression)
new T[](length; index => expression)
```

The lexer, parser, resolver, and type checker accept this syntax. HIR retains
the exact `u64` length expression, inline versus shared-outer ownership, one
immutable exact-`i64` local identity, and one destination-directed element
initialization plan. The length is checked before the index local becomes
active, and the local is in scope only while checking the element expression.
An explicit executable-lowering diagnostic currently prevents this typed form
from entering MIR. Once dynamic-prefix lowering replaces that gate, the form
will evaluate the length once, validate and allocate unpublished backing once,
then evaluate the element expression once for every increasing index. A zero
length evaluates no element expression.

Each dynamic position is a previously uninitialized owning destination of the
explicit array element type. Primitive, exact-class, inline-optional, nested
inline-array, shared-owner, and optional-owner sources use the same
destination-directed operations as one explicit element-list position. Named
sources copy, eligible produced sources initialize or transfer directly, and
no default construction or copy assignment occurs merely because the result
has dynamic length.

Each element has one bounded evaluation and cleanup epoch. After its selected
initialization completes, the initialized prefix advances and non-transferred
temporaries, anchors, guards, wrappers, and owners clean before the next index
begins. Effects are therefore deterministic in increasing-index order without
retaining an unbounded number of temporaries.

Backing remains unpublished until the dynamic initialized prefix equals the
checked requested length. Inline construction then publishes one owning array;
leading `new` publishes one shared owner of the complete outer array. Normal
completed arrays retain ordinary copy, assignment, destruction, parameter,
result, and reverse-cleanup behavior. Current non-unwinding failure retains no
new partial-prefix cleanup promise.

The initial frozen form is not an iterable comprehension, fill constructor,
statement block, closure, or mutable array builder. It adds no inference,
filtering, flattening, spread, unknown-length collection, array covariance, or
runtime callback. Its primary ordinary-library adopter will be
`Vec<T>.to_array()`, whose result contains the logical live prefix rather than
capacity storage.

The compiler representation, dynamic-prefix proof, runtime boundary, rejected
alternatives, and decision history are preserved in the
[frozen design record](../archive/INDEXED_ARRAY_CONSTRUCTION_DESIGN_PROPOSAL.md).
Delivery is tracked by the
[indexed array construction roadmap](../roadmaps/INDEXED_ARRAY_CONSTRUCTION_ROADMAP.md).

## Inline array value semantics

An inline `T[]` owns its element backing. The backing need not be on the stack
and is not required to be adjacent to the local, parameter, field, or result
that owns the array value. That representation choice does not weaken value
semantics.

A named inline source is deep-copied into a distinct backing:

```ska
var first: T[] = T[](10u);
var second: T[] = first;
```

The arrays have independent lengths, backing, and element storage. Copying
recursively applies the element type's operation:

- primitives copy their value;
- exact classes use copy construction;
- nested inline arrays deep-copy recursively; and
- shared-owner elements copy one owner, so the two array backings are
  distinct while corresponding elements own the same shared allocations.

Named inline array value arguments likewise deep-copy the complete array into
the callee parameter. A named return source deep-copies into the result.

Whole-array assignment to a mutable owning inline place may change length:

```ska
var destination: T[] = T[](10u);
var source: T[] = T[](20u);
destination = source;
```

Assignment evaluates the destination place, evaluates and secures a complete
deep copy of the source, installs the new backing, then ends the destination's
ownership of the old backing. Without an active element-borrow anchor, ending
that ownership destroys the old elements in reverse index order and releases
the old backing immediately. A required hidden anchor defers both destruction
and release until its last borrow ends. The incoming copy is complete before
the old value is changed, making direct and indirect self-assignment safe.
Whole-array assignment therefore requires element copy construction, not
element copy assignment.

### Produced backing adoption

A newly produced inline array has no source binding that remains usable after
its consuming operation. An owning destination adopts that produced array's
backing instead of deep-copying it:

```ska
destination = T[](20u);
consume(T[](20u));
return T[](20u);
```

Adoption applies to inline array initialization, assignment, internal value
arguments, results, and owning temporaries. It transfers the hidden backing
ownership into the destination, invokes no element copy operation, and leaves
no source-visible moved-from value. Assignment still destroys the
destination's old elements after the produced backing is secured.

An array-returning call likewise produces one owned result backing that its
next owning destination may adopt. Explicit `T[](copy source)` must first
perform its requested deep copy from `source`; its completed result may then
be adopted by the next destination without another copy.

This array-specific backing transfer does not add a general move operator or
change inline class copy and elision rules.

## Shared array value semantics

`shared T[]` is a non-null strong owner of one fixed-size array allocation.
Copying a named shared array owner retains that allocation; produced owners
transfer under the ordinary shared-owner rules:

```ska
var first: shared T[] = new T[](10u);
var second: shared T[] = first;
```

`first` and `second` own the same array allocation. Element mutation through
either owner is visible through the other. The last strong owner destroys
elements in reverse index order and frees the allocation.

Owner assignment changes which allocation one owner place denotes. It secures
the incoming owner before releasing the old owner:

```ska
first = new T[](20u);
```

Other owners of the old allocation continue to observe that old fixed-size
array. Whole-pointee assignment remains unsupported:

```ska
*first = T[](20u); // invalid
```

In-place element or slice assignment mutates the shared allocation and is
visible through every owner. Creating a distinct shared array copy is
explicit:

```ska
var copy: shared T[] = new T[](copy *first);
```

`(shared T[])?` is an optional shared owner, not an optional inline array. It is
either `none` or contains one ordinary non-null `shared T[]` owner:

```ska
var maybe: (shared T[])? = none;
maybe = new T[](10u);
```

Unwrapping secures an ordinary owner before any array access. Copy, assignment,
cleanup, and failure otherwise follow the existing optional shared-owner
contract.

## Length, indices, and bounds

`array.len()` returns the immutable length as `u64`. Length arguments and
results are unsigned because a length is never negative.

Element indices and explicit slice bounds have exact type `i64`. A
non-negative index is measured from the beginning. A negative index is
translated once relative to the end:

```text
normalized(i, length) = i          when i >= 0
normalized(i, length) = length + i when i < 0
```

This is one relative-to-end translation, not modulo wrapping. For length five,
`-1` selects the last element, `-5` selects the first, and `-6`, `5`, and every
larger out-of-range index fail. Normalization must handle the minimum `i64`
without signed overflow.

An element index is valid only when its normalized value is in
`0 .. length`. An empty array has no valid element index.

## Element access and shared projection

`array[index]` selects one element place. The receiver and index are evaluated
once, the index is normalized and checked, and only then is the element read
or written.

The result follows the element category:

- reading a primitive element produces its primitive value;
- an exact-class or nested-array element remains a place and copies only in an
  owning value context;
- reading a shared-owner element copies one owner; and
- writing uses primitive store, exact-class copy assignment, nested whole-array
  assignment, or secure shared-owner assignment as appropriate.

Raw shared owners are not implicitly array places. The shared dereference
operators cross exactly one ownership edge:

```ska
var values: shared T[] = new T[](10u);

var length: u64 = values->len();
var value: T = values->[3];
values->[-1] = replacement;
```

The bracket projection is equivalent to explicit prefix dereference:

```ska
values->[3]   // equivalent to (*values)[3]
values->[2:7] // equivalent to (*values)[2:7]
values->len() // equivalent to (*values).len()
```

Direct `values[3]` and `values.len()` are invalid because they do not cross the
shared edge. `->` followed by a member or bracket projection evaluates its
owner expression once.

An optional shared array must be unwrapped before dereference:

```ska
var maybe: (shared T[])? = new T[](10u);
var value: T = maybe!->[3];
maybe!->[3] = replacement;
```

Absence fails before index normalization or bounds checking. The secured owner
from `!` remains live for the complete projected operation, including later
right-side evaluation in an assignment.

Each arrow in a nested expression crosses exactly one shared edge:

```ska
outer->[row][column]      // shared outer array, inline inner array
outer->[row]->[column]    // shared outer and shared inner arrays
outer->[row]!->[column]   // optional shared inner array
```

## Slices

A slice range is half-open: its normalized start is included and its normalized
end is excluded. The forms are:

```ska
array[start:end]
array[:end]
array[start:]
array[:]
```

Omitted start means zero and omitted end means the array length. Supplied
bounds have type `i64` and use the same one-time negative normalization as
indices, except that slice bounds are positions and may equal the length. A
valid range satisfies:

```text
0 <= normalized_start <= normalized_end <= length
```

Examples for an array of length ten include:

```ska
array[4:-3] // positions 4 through 6
array[:-1]  // every element except the last
array[-3:]  // the last three elements
array[:]    // the complete range
```

Reverse ranges and strides are not supported. A normalized start greater than
the normalized end fails. `empty[:]` and `empty[0:0]` are valid empty slices.

### Slice reads

Reading a slice constructs a new inline `T[]` with distinct backing and
copy-constructs its elements in increasing source-index order:

```ska
var copy: T[] = source[2:7];
var from_shared: T[] = shared_source->[2:7];
```

Nested inline arrays therefore deep-copy recursively. Shared-owner elements
copy owners. A slice read never creates a view and never returns a shared
array merely because its receiver is shared. Use `new T[](copy source[2:7])`
when a distinct shared slice allocation is required.

### Slice assignment

A slice destination preserves its array allocation and length:

```ska
var a: T[] = T[](10u);
var b: T[] = T[](20u);
var c: T[] = T[](10u);

b[5:15] = a;
b[10:15] = a[2:7];
c[:] = a;
c[:] = a[:];
```

The source length must equal the destination range length. A mismatch fails
before the first destination element is changed:

```ska
b[10:15] = a; // fails: five destination elements, ten source elements
```

Slice assignment uses the element assignment operation in increasing
destination-index order. It therefore requires copy assignment for exact
class elements; nested array elements use their whole-array assignment, and
shared-owner elements secure each incoming owner before releasing the old one.

The assignment destination, its explicit bounds, and the right side are
evaluated in source order. Every required destination bound, source bound, and
length check completes before the first destination write. A right-side slice
is a copying slice value and is therefore fully materialized before writes:

```ska
array[1:9] = array[0:8];
```

This has snapshot behavior even when ranges overlap. For lifecycle-bearing
elements, the right-side slice's copy construction, destination assignment,
and temporary destruction are source-visible operations. An implementation
may fuse storage operations only when it preserves those operations, their
order, all checks, and all failure behavior.

Invalid element indices, invalid slice bounds, and unequal slice-assignment
lengths are distinct compiler-known failures. Under the
[common panic policy](ERRORS.md#frozen-panic-design), all use one reporter
without losing those semantic distinctions. This array contract does not
duplicate the exact messages.

Whole assignment and full-range slice assignment are deliberately different:

```ska
destination = source;  // replaces backing and may change length
destination[:] = source; // preserves backing and requires equal length
```

## Nested arrays

Nested arrays are jagged arrays, not rectangular matrix types. Default
initialization of an inline array element creates an empty array:

```ska
var rows: i64[][] = i64[][](3u);
```

`rows` has length three, and each `rows[index]` is a complete valid empty
`i64[]`. The inner values are logically distinct even when empty-array
representations share no allocated storage. Replacing one row does not affect
another:

```ska
rows[0] = i64[](8u);
rows[1] = i64[](20u);
```

Inner arrays are never implicitly optional. Absence is available through an
optional inline array value such as `i64[][]?`, or through an optional shared
owner such as `(shared T[])?`. Optional arrays as array elements remain
deferred.

## Aliases, mutation, and backing anchors

Array places are eligible call-scoped alias sources:

```ska
fn inspect(ref values: T[]) -> unit
fn modify(mut ref values: T[]) -> unit
```

A read-only alias may read length and elements. A mutable alias may assign
elements and slices but cannot rebind its array root through whole-array
assignment. Passing `*shared_array` as an array alias source uses the existing
shared-owner anchor rules.

Exact-class and nested-array element places may be passed to compatible
`ref` or `mut ref` parameters. Array aliases remain non-owning,
call-scoped, nonexclusive, and non-escaping.

Whole inline-array replacement can detach an old backing while a call-scoped
element alias still designates storage in that backing. The implementation
must retain a hidden backing anchor through the complete call. Normal inline
array copying remains deep; hidden anchors are not source-visible shallow
array copies. An alias continues to designate the old element storage even if
another overlapping path replaces the owning array place during the call.

Read-only access propagates recursively through inline array and inline class
elements. It is shallow across a shared-owner element or a shared array owner,
consistent with existing shared ownership: read-only access prevents replacing
that owner place but does not make the separately shared allocation deeply
immutable.

The implemented [standard I/O API](IO.md) reuses this existing alias surface for
private `u8[]` intrinsics. Open and write receive read-only whole-array aliases;
read receives a mutable whole-array alias plus an offset. The compiler keeps
the existing backing anchor alive and passes only the checked remaining byte
range to the runtime. This does not add array descriptor passage to the
external C ABI, a public array I/O API, or new alias semantics.

## Lifetime and containment

Elements begin lifetime in increasing index order after backing allocation
succeeds. Default construction of a non-optional shared element allocates and
initializes its distinct pointee at that element's position, then adopts the
produced owner into the slot before advancing to the next index. A completed
array owns every element until whole-array replacement or array destruction.
Elements are destroyed in decreasing index order, after which the backing may
be released. When an active hidden anchor still borrows an element from a
detached inline backing, both element destruction and backing release are
deferred until the final anchor ends.

Inline array fields participate in their containing class's copy, assignment,
and destruction operations through these array rules. Because element storage
is dynamically indirect, an array edge does not participate in finite inline
class-containment rejection. Lifecycle capability analysis must nevertheless
account for recursive class/array graphs when determining whether default
construction, copying, assignment, and destruction are available.

An implementation may track an initialized element prefix during construction.
That state is not an optional element state and is never source-visible.
Current unrecoverable termination does not guarantee cleanup of that prefix.
Future recoverable exceptions must define and execute prefix cleanup before
array construction can throw.

## Static storage

The implemented [static-field contract](STATIC_FIELDS.md) permits an inline
`T[]` as class-owned static storage for every legal array element type. An
initializer-free declaration begins as the existing allocation-free empty
descriptor, constructs no elements, and therefore does not require its element
type to be default initializable. An explicit initializer uses ordinary array
construction, copy, or produced-backing adoption before entry. Later
replacement, indexing, slicing, aliases, anchors, and standard-I/O buffer use
retain this document's ordinary rules. Displaced backing receives ordinary
cleanup; reverse normal-return shutdown releases the current final backing and
destroys its elements in reverse index order. Abrupt termination does not
unwind remaining static arrays. This static-array profile is implemented
across eager initialization, replacement, projections, copied slices, call
aliases, and byte-I/O buffers.

## Failure

These source operations terminate unsuccessfully without returning a value or
guaranteeing remaining source-level cleanup:

- element index outside the valid normalized range;
- invalid or reversed slice bounds;
- slice source/destination length mismatch;
- element count or byte-layout overflow;
- a requested length greater than the maximum `i64` value;
- allocation failure; and
- shared-owner count overflow during array ownership operations.

Invalid element types, non-`u64` lengths, non-`i64` indices or supplied slice
bounds, unavailable default/copy/assignment capabilities, and unsupported
array ownership combinations are compile-time errors.

## Deferred extensions

The implemented
[compositional optional profile](OPTIONAL_VALUES.md#compositional-optional-types)
defines inline optional arrays as `T[]?`, distinct from both `T?[]` and
`(shared T[])?` (`shared? T[]` shorthand). Optional arrays reuse ordinary
array lifecycle across fields, statics, internal calls and dispatch,
initializer overloads, checked aliases, and array elements.

The following are intentionally outside the implemented array profile:

- inferred array literals, expected-type-only lists, fill-value,
  unknown-length comprehensions, spreads, repetition, and rectangular-shape
  initialization syntax; indexed construction is frozen separately above;
- capacity, resizing an existing allocation, append, insertion, removal, or
  other dynamic-buffer operations;
- non-copying slice views, reverse ranges, and strides;
- general equality, ordering, hashing, identity, or array casts and type tests;
- method-form `index_get`, `index_set`, `slice_get`, or `slice_set` aliases for
  arrays, and iteration protocols; the separately frozen structural bracket
  protocol applies only after array precedence is resolved;
- `for` iteration and iterator lifetime behavior;
- whole-pointee shared array assignment;
- array external ABI mappings;
- recoverable bounds or allocation failures and exceptional prefix cleanup;
  and
- concurrency, atomic shared counts, or synchronization guarantees.

These exclusions do not weaken the implemented value, ownership, indexing,
slicing, lifetime, and failure rules above. Their syntax and semantics require
focused design before implementation.
