# Generic Array Copy Lifecycle Discovery

Status: **resolved** on 2026-08-15.

## Problem

An explicit user copy constructor on a generic class could pass resolution and
type checking but panic during preliminary MIR lowering when a type parameter
field was specialized to an array. For example:

```ska
class Box<T> {
    value: T;

    init(value: T) {
        self.value = value;
    }

    copy(ref source: Box<T>) {
        self.value = source.value;
    }
}

fn main() -> i64 {
    var original: Box<i64[]> = Box<i64[]>(i64[]{1});
    var copied: Box<i64[]> = original;
    return copied.value[0];
}
```

The resulting verifier failure was `array replacement destination is not live`
inside the specialized copy body. The equivalent class using synthesized copy
lifecycle verified and executed.

## Resolution

Field-assignment typing now applies the common receiver-initialization policy
to arrays. Ordinary initializers and copy constructors emit array-field
initialization HIR, while copy-assignment bodies and mutable methods continue
to emit replacement HIR for already-live destinations. This is decided after
generic specialization, so no generic-only lifecycle operation or MIR rule is
needed.

Focused HIR coverage distinguishes the specialized explicit copy constructor
from copy assignment. Native deterministic coverage verifies primitive arrays,
owning-element arrays and cleanup, optional arrays, and recursively nested
generic array fields through both copy construction and assignment.
