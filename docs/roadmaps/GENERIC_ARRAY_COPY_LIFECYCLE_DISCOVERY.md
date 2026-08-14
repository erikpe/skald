# Generic Array Copy Lifecycle Discovery

Status: **pending follow-up**.

## Problem

An explicit user copy constructor on a generic class can pass resolution and
type checking but panic during preliminary MIR lowering when a type parameter
field is specialized to an array. For example:

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

The resulting verifier failure is `array replacement destination is not live`
inside the specialized copy body. The equivalent class using synthesized copy
lifecycle verifies and executes, so structural indexing can cover array-valued
generic results and replacements without depending on this defect.

## Evidence and likely owner

The failure occurs after closed specialization, while lowering the direct
`self.value = source.value` copy-body assignment. The likely owner is the
type-checking or MIR-lowering distinction between first initialization of a
copy destination field and replacement of an already-live array field.

Priority is **high** because accepted source reaches an internal compiler
panic, although the affected explicit generic-lifecycle form has a synthesized
lifecycle workaround.

## Follow-up boundary

A focused fix should:

- preserve copy-body destination initialization after substituting array,
  optional-array, and recursively owning type arguments;
- keep ordinary mutable-method assignments as replacements;
- add generic explicit copy and assignment tests for primitive arrays,
  owning-element arrays, optionals, and nested generic fields; and
- turn any remaining unsupported specialization into a source diagnostic
  before HIR rather than an internal MIR failure.

This is generic-class lifecycle work, not a structural indexing protocol or
lower-IR change.
