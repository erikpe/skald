# Compiler crash: function-valued fields in class arrays

Status: resolved. The x86-64 raw-address class-copy helper now copies
function-valued fields as stored scalar words, with source-to-execution
regression coverage for class-array copying, assignment, slicing, mutation, and
indirect invocation.

## Impact

At the defective revision, a legal array of class values whose class contains a
capture-free function value crashed the compiler during x86-64 assembly
emission. No executable was needed to trigger it. The input did not contain an
explicit copy operation.

This blocked a natural representation of Doom state/action tables: an array of
records containing a typed callback. An ordinary, non-array class instance with
the same function-valued field compiled and ran in the feasibility probe.

This is distinct from intentionally unsupported arrays of function values:
`State[]`, where `State` has a function field, is the relevant composition;
`(fn(i64) -> i64)[]` is outside the language contract. Rejecting the former as
though it were the latter would unnecessarily narrow existing class semantics.
See [function-value storage](../language/FUNCTION_VALUES.md#storage-and-lifecycle)
and [array element types](../language/ARRAYS.md#type-and-ownership-forms).

## Reproduction

Reproduced on 2026-09-11 at Skald revision
`e044178afb2ddbeb36c2a230c62e3a7f794a1f5c`, target `x86_64-sysv`, using the
compiler built from the checkout. Save this as `/tmp/callback_array.ska`:

```ska
fn action(value: i64) -> i64 { return value; }

class State {
    callback: fn(i64) -> i64;
    init() { self.callback = action; }
}

fn main() -> i64 {
    var states: State[] = State[](2u);
    return states[1].callback(0);
}
```

From the repository root:

```sh
RUST_BACKTRACE=1 cargo run --quiet --locked -p skac -- \
    /tmp/callback_array.ska --emit asm -o /tmp/callback_array.s
```

Expected: successful assembly emission; linking and running the resulting
program should return 0.

Observed: compiler exit status 101 with a Rust panic:

```text
crates/skald-compiler/src/backend/x86_64_sysv/lower/array/lifecycle.rs:523:14:
internal error: entered unreachable code: verified scalar copy has a primitive payload
```

The relevant backtrace path is:

```text
lower::array::lower_helpers
  -> lower_class_copy_helpers
  -> lower_class_copy
  -> emit_field_copy
  -> emit_scalar_copy
```

The first four class-copy functions are in
[array lifecycle emission](../../crates/skald-compiler/src/backend/x86_64_sysv/lower/array/lifecycle.rs).
This is a compile-time backend panic, not a Skald runtime panic, a failed C link,
or evidence that indirect calls themselves are generally broken.

## Cause and why construction alone triggers it

The inspected implementation establishes this path:

1. `required_class_copy_helpers` scans the program's array types. A class element
   requires its address-based copy helper; nested class and base dependencies
   are followed as well.
2. `lower_class_copy` emits the synthesized copy constructor's field operations.
3. `emit_field_copy` routed a `MirSynthesizedFieldCopy::Primitive` operation to
   `emit_scalar_copy`, passing the field's actual MIR type.
4. At the defective revision, `emit_scalar_copy` accepted `U8`/`Bool` with byte
   moves and `I64`/`U64`/`F64` with word moves. Its fallback was the observed
   `unreachable!`; it omitted function-valued fields.

Function values already have non-null, trivial, eight-byte code-pointer
representation on this target. Copying one duplicates its bits; it does not
retain an owner or invoke a callback. See the
[function-value backend contract](../compiler/BACKEND.md).

The helper requirement is driven by array element types, not only by source
copy expressions. That explains why the reproducer fails despite merely
constructing the array and calling one element's callback. The runtime array
copy need not execute: generating its helper already reaches the missing case.

The evidence identified a missing scalar case in this private native helper.

## Resolution and validation

The repair remains with the x86-64 array lifecycle owner and preserves the
language and runtime ABI contracts. The MIR copy-plan variant is now named
`Scalar`, matching the existing HIR category and verifier predicate instead of
implying that function values are primitives. `emit_scalar_copy` now includes
`MirType::Function(_)` in the ordinary full-word load/store path, matching both
the function-value target representation and the existing ordinary class-copy
selector. Its invariant message now says stored scalar rather than primitive,
which matches the MIR verifier: function values are scalar values but are not
one of the five primitive types.

The source-to-execution regression
`tests/golden/function_values/class_array_fields.ska` exercises observable
callback identity rather than successful compilation alone. It:

- constructs a class array containing function-valued fields and calls them;
- explicitly copies the array, changes the source callback, and proves the copy
  retained its original target;
- performs whole-array assignment and slicing, then independently mutates
  callbacks and scalar fields in the related arrays; and
- calls each retained callback and checks exact results.

The existing compile-fail case for direct arrays of function values remains in
the same golden specification and continues to pass. The focused function-value
suite passed 26 cases, including the new regression. The complete repository
`make check` gate passed: 3,100 compiler unit tests and all other workspace,
runtime, documentation, and golden suites; the complete golden run passed 629
cases. No MSRV gate was needed because the repair changes neither manifests nor
supported Rust syntax.

## Related investigation

Found while probing [Doom port feasibility](../roadmaps/DOOM_PORT_FEASIBILITY.md). The
successful companion probe exercised an ordinary callback field, binary I/O,
mutable byte-array filling and primitive-only calls into C. That contrast
narrows the defect to the class-array lifecycle composition and does not
establish that all other callback containers work.
