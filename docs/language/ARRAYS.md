# Arrays

Status: **implemented contract on x86-64**. This document is authoritative for
the source-visible array contract. The
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

Array storage is an indirection boundary. Inline array elements belong to the
array's deterministic lifetime, but their dynamically sized backing does not
become inline class storage merely because the array value is inline. This
permits recursive declarations such as a class containing `Node[]` without
creating an infinitely sized inline class.

## Type and ownership forms

Postfix `[]` constructs an array type. Leading `shared` or `shared?` before an
unparenthesized array spelling applies to the complete following array type.
Parentheses place ownership inside the element type:

| Type | Meaning |
|---|---|
| `T[]` | Inline array value containing inline `T` elements |
| `shared T[]` | Shared owner of one array allocation containing inline `T` elements |
| `(shared T)[]` | Inline array whose elements are non-null shared owners of `T` objects |
| `shared (shared T)[]` | Shared array whose elements are shared owners of `T` objects |
| `shared? T[]` | Optional shared owner of a `T[]` array allocation |
| `(shared? T)[]` | Inline array whose elements are optional shared owners of `T` objects |

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
array element types. Inline optional arrays are deliberately not part of the
contract: no source form makes an array type itself an inline optional
payload. This does not prevent `shared? T[]`, whose absence belongs to the
shared owner rather than to an inline optional array payload.

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
| Supported `shared? T` | `none` |
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

No fill-value, per-index generator, array literal, or multi-dimensional shape
constructor is implemented. Nonempty construction therefore
requires a default-initializable element type or an exact copy source.

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

`shared? T[]` is an optional shared owner, not an optional inline array. It is
either `none` or contains one ordinary non-null `shared T[]` owner:

```ska
var maybe: shared? T[] = none;
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
var maybe: shared? T[] = new T[](10u);
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
optional shared owner, for example an array whose element type is
`shared? T[]`. Inline optional array payloads remain deferred.

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

## Frozen static storage

The frozen [zero-default static-field contract](STATIC_FIELDS.md) permits an
inline `T[]` as future class-owned static storage for every legal array element
type. It begins as the existing allocation-free empty descriptor, constructs
no elements, and therefore does not require its element type to be default
initializable. Later replacement, indexing, slicing, aliases, anchors, and
standard-I/O buffer use retain this document's ordinary rules. Displaced
backing receives ordinary cleanup, while final backing is deliberately not
cleaned at process exit. Static array syntax is not yet implemented.

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

The following are intentionally outside the implemented array profile:

- inline optional array payloads and their eventual source spelling;
- fill-value, per-index generator, array literal, and rectangular-shape
  initialization syntax;
- capacity, resizing an existing allocation, append, insertion, removal, or
  other dynamic-buffer operations;
- non-copying slice views, reverse ranges, and strides;
- general equality, ordering, hashing, identity, or array casts and type tests;
- structural `index_get`, `index_set`, `slice_get`, `slice_set`, or iteration
  protocols;
- `for` iteration and iterator lifetime behavior;
- whole-pointee shared array assignment;
- array external ABI mappings;
- recoverable bounds or allocation failures and exceptional prefix cleanup;
  and
- concurrency, atomic shared counts, or synchronization guarantees.

These exclusions do not weaken the implemented value, ownership, indexing,
slicing, lifetime, and failure rules above. Their syntax and semantics require
focused design before implementation.
