# Vectors

**Status:** implemented generic and shared-object compatibility profiles.

This document defines the source-visible contract of the ordinary Skald
standard-library classes `std::vec::Vec<T>` and `std::vec::VecObj`. Built-in
fixed-size array semantics remain defined by [Arrays](ARRAYS.md), while generic
application legality is defined by [Generic classes](GENERIC_CLASSES.md).

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
    fn get(index: i64) -> T;
    mut fn set(index: i64, value: T) -> unit;
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

`get` and `set` accept non-negative indices from the beginning and negative
indices relative to the current logical length. `last` selects the final
logical element. Capacity slots outside `0..len()` are never elements and
cannot be observed through the public API.

Invalid `get` or `set` indices terminate through `std::error::panic` with
`Vec.get: index out of bounds` or `Vec.set: index out of bounds`. `pop` and
`last` on an empty vector use `Vec.pop: empty vector` and
`Vec.last: empty vector`.

## Ownership and lifetime

Arguments and results follow the ordinary value rules for the substituted
`T`. `push` and `set` secure the new value before their parameters are cleaned.
`get`, `last`, and `pop` return independent inline copies or secured shared
owners as appropriate. `pop` clears the removed slot before returning, and
`clear` clears all logical slots in reverse order. Removed exact values are
destroyed and removed last shared owners are finalized promptly; spare
capacity retains no removed value.

Copying or assigning a named `Vec<T>` deep-copies its inline backing array.
Vector length, capacity storage, and slots are independent afterward. Inline
elements follow their selected copy operations, while corresponding shared
elements retain the same pointee allocations under independent owner handles.

## `VecObj` compatibility profile

`VecObj` remains public and unchanged for source compatibility and for
heterogeneous collections erased to non-null `shared Obj`:

```text
public class VecObj {
    init();
    static fn with_capacity(capacity: u64) -> VecObj;
    fn len() -> u64;
    fn capacity() -> u64;
    fn is_empty() -> bool;
    mut fn clear() -> unit;
    mut fn push(value: shared Obj) -> unit;
    mut fn pop() -> shared Obj;
    fn last() -> shared Obj;
    fn get(index: i64) -> shared Obj;
    mut fn set(index: i64, value: shared Obj) -> unit;
}
```

Its capacity, growth, indexing, structural-copy, and prompt-release behavior
matches the corresponding `Vec<shared Obj>` mechanics, but the two classes
have distinct nominal identities and no implicit conversion. `VecObj` retains
its established panic messages prefixed by `VecObj.`. New homogeneous code can
prefer `Vec<T>` to preserve its exact element type; existing `VecObj` users do
not require migration.

## Language, compiler, and runtime boundary

Both vectors are ordinary standard-library source composed from classes,
arrays, optionals, loops, casts, panic, and shared ownership. Neither adds an
indexing protocol, compiler intrinsic, IR instruction, target operation, or
runtime ABI entry.

## Deliberate limits

The implemented profile does not include insertion or removal at arbitrary
positions, append, slicing, iteration protocols, indexing syntax, sorting,
function-valued algorithms, capacity reservation after construction, explicit
shrinking, allocators, or small-vector optimization.
