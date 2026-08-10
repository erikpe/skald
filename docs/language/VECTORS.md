# Object Vectors

**Status:** implemented initial object-owner profile.

This document defines the source-visible contract of the ordinary Skald
standard-library class `std::vec::VecObj`. Availability is tracked in
[Language status](STATUS.md). Built-in fixed-size array semantics remain
defined by [Arrays](ARRAYS.md).

## Public surface

`VecObj` is an inline class whose logical elements are non-null `shared Obj`
owners. This synopsis lists signatures without source bodies; it is not class
declaration syntax:

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

It belongs to logical module `std::vec` and is not part of the prelude:

```ska
from std::vec import VecObj;
```

The default initializer creates an empty vector with capacity four.
`with_capacity` creates an empty vector with exactly the requested capacity,
including zero. Capacity is retained by `pop` and `clear`; there is no
automatic shrinking operation.

## Elements, indexing, and growth

`push` appends one owner, growing before insertion when necessary. Growth
starts at capacity four when the old capacity is smaller, then doubles until
the requested length fits. Allocation limits and failures inherit the
built-in array contract.

`get` and `set` accept non-negative indices from the beginning and negative
indices relative to the current logical length. `last` selects the final
logical element. Capacity slots outside `0..len()` are never elements and
cannot be observed through the public API.

An invalid `get` or `set` index terminates through `std::error::panic` with
`VecObj.get: index out of bounds` or `VecObj.set: index out of bounds`.
`pop` and `last` on an empty vector similarly use `VecObj.pop: empty vector`
and `VecObj.last: empty vector`.

## Ownership and lifetime

The `push` call copies a named argument or transfers a produced argument into
its value parameter under the ordinary shared-owner rules. The method then
secures an owner in the destination slot before its parameter is cleaned.
`get`, `last`, and `pop` return a secured non-null owner. `set` secures the
replacement before releasing the displaced owner.

`pop` clears the removed capacity slot before returning, and `clear` clears
all logical slots in reverse order. Removed objects therefore remain alive
only when another shared owner still retains them; spare capacity does not
retain removed objects.

Copying or assigning a named `VecObj` follows its synthesized inline-class and
inline-array lifecycle. The destination receives independent vector storage,
while corresponding logical elements retain the same shared object
allocations. Mutating one vector's length or slots does not mutate another
vector copied from it. Mutating a shared pointee remains visible through every
owner of that pointee.

The implementation uses an inline `(shared? Obj)[]` field. Present slots in
the logical prefix contain ordinary `shared Obj` owners; capacity slots in the
tail are `none`. This optionality is private storage state and does not make
the public element type optional.

## Language, compiler, and runtime boundary

`VecObj` is ordinary standard-library Skald source composed from classes,
arrays, optional shared owners, loops, casts, panic, and shared ownership. It
adds no syntax, structural indexing protocol, compiler intrinsic, IR
operation, target behavior, or runtime ABI entry.

## Deliberate limits

The initial profile does not include primitive vector classes, exact inline
class elements, optional logical elements, insertion or removal at arbitrary
positions, append, slicing, iteration protocols, indexing syntax, sorting,
function-valued algorithms, capacity reservation after construction, or
explicit shrinking. Generic declarations and specialization remain a separate
language design question.
