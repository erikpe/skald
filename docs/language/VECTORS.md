# Vectors

**Status:** implemented generic structural-collection profile.

This document defines the source-visible contract of the ordinary Skald
standard-library class `std::vec::Vec<T>`. Built-in fixed-size array semantics
remain defined by [Arrays](ARRAYS.md), while generic application legality is
defined by [Generic classes](GENERIC_CLASSES.md).

## Generic vector surface

`Vec<T>` is an inline class whose logical elements have the exact substituted
type `T`:

```text
public class Vec<T> {
    init();
    static fn with_capacity(capacity: u64) -> Vec<T>;

    fn len() -> u64;
    fn capacity() -> u64;
    fn is_empty() -> bool;
    mut fn clear() -> unit;
    mut fn push(value: T) -> unit;
    mut fn pop() -> T;
    fn last() -> T;
    fn index_get(index: i64) -> T;
    mut fn index_set(index: i64, value: T) -> unit;
    fn slice_get(start: i64?, end: i64?) -> Vec<T>;
    mut fn slice_set(start: i64?, end: i64?, replacement: Vec<T>) -> unit;
}
```

It is explicitly imported and always applied with a complete type argument:

```ska
from std::vec import Vec;

var numbers: Vec<i64> = Vec<i64>();
var names: Vec<Str?> = Vec<Str?>();
var owners: Vec<shared Readable> = Vec<shared Readable>();
```

The default initializer creates an empty vector with capacity four.
`with_capacity` creates an empty vector with exactly the requested capacity,
including zero. Capacity is retained by `pop` and `clear`; there is no
automatic shrinking operation.

## Representation and admitted element types

The implementation is ordinary generic Skald source with private `T?[]`
storage. The outer optional layer records whether a capacity slot is occupied:

| Application | Private storage |
|---|---|
| `Vec<Str>` | `Str?[]` |
| `Vec<Str?>` | `Str??[]` |
| `Vec<shared Str>` | `(shared Str)?[]` |
| `Vec<shared Readable>` | `(shared Readable)?[]` |

For an optional element type, outer absence means an unused capacity slot and
outer presence containing inner absence is a logical `none` element. These
states are not flattened.

There is no vector-specific argument whitelist or lifecycle annotation.
Storage and the complete method bodies infer their requirements through the
ordinary generic-class rules. In particular, the implemented API copies
elements for reads and structural vector copies, assigns elements for
replacement and growth, and destroys occupied slots on removal. Primitive,
`Str`, supported exact inline classes, optional values, arrays, shared exact
owners, and shared interface owners satisfy these operations when their
ordinary capabilities do. A bare interface such as `Vec<Readable>` is not
storable through `T?`; `Vec<shared Readable>` is.

## Indexing, growth, and failure

`push` appends one element, growing before insertion when necessary. Growth
starts at capacity four when the old capacity is smaller, then doubles until
the requested length fits. Allocation limits and failures inherit the built-in
array contract.

`index_get`, `index_set`, and bracket indexing accept non-negative indices
from the beginning and negative indices relative to the current logical
length. `last` selects the final logical element. Capacity slots outside
`0..len()` are never elements and cannot be observed through the public API.

Invalid indices terminate through `std::error::panic` with
`Vec.index_get: index out of bounds` or `Vec.index_set: index out of bounds`.
`pop` and `last` on an empty vector use `Vec.pop: empty vector` and
`Vec.last: empty vector`.

## Slices

Vector slices are half-open logical ranges. Each supplied bound is normalized
once relative to `len()`: a negative bound adds the logical length. An omitted
start selects zero and an omitted end selects the logical length. After
normalization a valid range satisfies `0 <= start <= end <= len`. Capacity is
never a valid bound and does not contribute elements.

`slice_get` and `values[start:end]` return an independent `Vec<T>`. The result
has logical length and initial capacity equal to the selected range length;
each element follows the ordinary `T` copy or owner-retain behavior. Mutating,
growing, clearing, or destroying either vector cannot change the other's
length, capacity storage, or inline elements. Corresponding shared elements
remain independent owners of the same pointee allocation.

`slice_set` and slice assignment preserve the destination length and capacity.
The replacement logical length must exactly equal the selected range length.
After both checks succeed, elements are assigned in increasing destination
index order. The replacement is an owning value parameter: ordinary call
preparation deep-copies a named vector before the method body and transfers a
produced slice result into the parameter. The complete replacement therefore
exists before the first destination write, including `values[:] = values` and
overlapping forms such as `values[1:4] = values[0:3]`. The parameter and its
temporary element copies are cleaned promptly when the call returns.

Invalid slice bounds terminate with `Vec.slice_get: invalid bounds` or
`Vec.slice_set: invalid bounds`; a replacement length mismatch terminates with
`Vec.slice_set: length mismatch`. Bounds are validated before replacement
length and both are validated before any destination element changes. Under
ordinary call evaluation, however, the replacement expression and any
required argument copy complete before the method body performs those checks.

## Ownership and lifetime

Arguments and results follow the ordinary value rules for the substituted
`T`. `push`, `index_set`, and slice replacement secure the new value before
their parameters or element temporaries are cleaned. `index_get`, `last`,
`pop`, and slice reads return independent inline copies or secured shared
owners as appropriate. `pop` clears the removed slot before returning, and
`clear` clears all logical slots in reverse order. Removed exact values are
destroyed and removed last shared owners are finalized promptly; spare
capacity retains no removed value.

When `T` is an exact inline class, the result of `index_get`, `last`, or `pop` may
directly receive a read-only method call. For example, a string vector can use
`snapshot.last()[index]` without a receiver-only `Str` local. The result
is secured in the ordinary caller-owned temporary, remains live through the
method call, and is then cleaned at the full-expression boundary. A mutable
method on that unnamed result remains invalid.

The same exact result may expose a readable field directly. A primitive field
is loaded before the vector-result temporary is cleaned, while class,
optional, array, and owner fields retain their ordinary bounded or owning
consumer rules. The standard `Str.join` implementation uses this composition
to read `values[index]._length` without a staging `Str` local; declaring-class
privacy is unchanged.

Copying or assigning a named `Vec<T>` deep-copies its inline backing array.
Vector length, capacity storage, and slots are independent afterward. Inline
elements follow their selected copy operations, while corresponding shared
elements retain the same pointee allocations under independent owner handles.

## Language, compiler, and runtime boundary

The vector is ordinary standard-library source composed from classes, arrays,
optionals, loops, casts, panic, and shared ownership. Its four
[structural bracket](INDEXING_AND_SLICING.md) entry points are ordinary methods:
the compiler adds no vector identity check, intrinsic, IR instruction, target
operation, or runtime ABI entry. Closed specializations select those methods
before HIR like any other generic class. Heterogeneous shared-object
collections use `Vec<shared Obj>` through the same generic implementation.

Index reads and writes are `O(1)`. A slice read is `O(n)` in the selected
length and allocates one exact-capacity result backing. Slice replacement
performs `O(n)` element assignments after ordinary argument preparation. A
named replacement's independent vector copy additionally costs time and
storage proportional to its capacity; a produced slice is transferred without
a second complete-vector copy.

## Deliberate limits

The implemented profile does not include insertion or removal at arbitrary
positions, append, iteration protocols, sorting, function-valued algorithms,
capacity reservation after construction, explicit shrinking, allocators, or
small-vector optimization.
