# Skald Types, Values, and Expressions

Status: authoritative for implemented type, value, literal, and expression
semantics and for the frozen primitive integer comparison and cast design. The
[status matrix](STATUS.md) is authoritative for feature maturity, and the
[implemented grammar](GRAMMAR.md) defines accepted source syntax.

## Type model

Every accepted expression has one exact static type. The implemented type set
contains five primitive value types, nominal class types, and the payload-free
result type `unit`:

| Type | Implemented meaning |
|---|---|
| `i64` | Signed integer values from -9,223,372,036,854,775,808 through 9,223,372,036,854,775,807. |
| `u64` | Unsigned integer values from 0 through 18,446,744,073,709,551,615. |
| `u8` | Unsigned integer values from 0 through 255. |
| `f64` | IEEE-754 binary64 floating-point values. |
| `bool` | The distinct values `false` and `true`. |
| primitive `T?` | An explicitly optional primitive owning value containing either no `T` or one complete `T`. |
| class `T?` | An explicitly optional inline owner containing either no payload or one complete exact-class `T`; postfix `!` supplies a bounded dynamically guarded payload place. |
| a class name | One exact nominal class value, including all of its inline fields. |
| `unit` | Successful completion without a result payload. |

Two separately declared classes are different types even when their fields are
identical. Owning class values remain exact: a derived-to-base owning
conversion creates an independent sliced base value through the selected copy
operation. Non-owning alias boundaries additionally support ancestor-class and
universal `Obj` views. There is no structural class compatibility.

`unit` is not a storable value type. It may appear only as a callable result in
the implemented grammar. A call returning `unit` can be used as a call
statement, but its result cannot initialize storage, satisfy a value parameter,
or serve as another expression's operand.

## Values and places

A **value** is a typed result or stored entity. A **place** designates storage.
Locals, value parameters, `self`, and field paths are places; reading a
primitive place produces a primitive value.

Class objects remain places in the implemented source model rather than
ordinary scalar expression values. A class place may be used where an exact
class object source, destination, receiver, or alias source is required. A
class-typed field selection likewise designates a subobject place; it does not
implicitly copy the subobject into an expression value. Construction and calls
that produce class objects are accepted only in the supported object
initialization, assignment, argument, and return contexts.

Initialization begins the lifetime of storage. Assignment updates an already
live value without beginning a new lifetime. Operation selection, copying,
destruction, and object materialization are
[class and lifecycle](CLASSES_AND_LIFECYCLE.md) concerns; the distinction here
explains why object places are not interchangeable with primitive values.

## Literal types and ranges

Literal spelling determines type. Expected type never reinterprets a literal,
and there is no untyped numeric-literal stage visible to the language:

| Source form | Type | Accepted value boundary |
|---|---|---|
| decimal digits without a suffix | `i64` | `0` through `9223372036854775807`; unary `-` also admits the minimum boundary described below |
| decimal digits followed by `u` | `u64` | `0u` through `18446744073709551615u` |
| decimal digits followed by `u8` | `u8` | `0u8` through `255u8` |
| decimal point or exponent form | `f64` | any spelling that rounds to a finite binary64 value |
| `false`, `true` | `bool` | the two boolean values |

The lexical forms, suffix spelling, and malformed-token behavior are defined
by the [grammar](GRAMMAR.md#literals). Range checks occur before a valid program
is accepted.

A leading `-` is an operator, not part of a numeric token. The otherwise
out-of-range magnitude `9223372036854775808` is accepted only as the operand of
unary negation, through any number of grouping parentheses, to form the `i64`
minimum. Larger positive or negative integer magnitudes are errors.

Decimal floating literals round once to the nearest binary64 value, with ties
to an even significand. Subnormal results and underflow to positive zero are
valid. A spelling that rounds to infinity is a range error. Infinity and NaN
have no literal spellings, although floating arithmetic can produce them.
Unary negation can turn positive zero into negative zero.

## Exact-type requirements

The implemented language performs no implicit conversion, promotion, or
expected-type literal inference. These boundaries require the
actual and expected types to be identical:

- primitive local initialization;
- primitive value arguments;
- return values;
- assignment to primitive fields;
- both operands of a binary arithmetic operator.

`bool` is not an integer type. It has no numeric operations or implicit
truthiness. A condition must already have type `bool`; integer, floating,
class, and `unit` values do not convert to it.

Exact class identity is likewise required wherever the implemented object
model accepts a class source and destination. That rule does not make a class
place an ordinary expression value.

## Expressions

The implemented expression families have these value effects:

| Expression family | Semantic result |
|---|---|
| literal | A value of the spelling-selected primitive type. |
| primitive binding | The stored primitive value, with the binding's declared type. |
| grouping | The inner expression's type and value; grouping remains source-significant for the limited object materialization rules. |
| primitive field selection | The field's stored primitive value. |
| direct function call | The declared primitive or `unit` result; an exact-class result is an object producer restricted to object contexts. |
| method call | The declared primitive or `unit` result; an exact-class result has the same object-context restriction. |
| shared dereference | A bounded non-owning class, interface, or `Obj` place selected from a `shared T` owner; it does not copy or transfer ownership. |
| unary or binary arithmetic | A value of the operand type under the rules below. |
| construction | An exact-class object producer restricted to supported object contexts. |

Using a complete class binding, `self`, or class-typed field as an ordinary
scalar expression is an error. Those forms can still be valid object places in
the contexts described above. Calls through arbitrary expression values and
function values are not implemented; calls select named functions or methods.

Prefix `*owner` explicitly selects the object place behind a `shared T`
handle. Postfix `owner->member` selects one member through exactly one shared
edge and is semantically equivalent to `(*owner).member`; the owner expression
is evaluated once. These forms support direct fields, class and interface
methods, inline subobject paths, field mutation, alias arguments, checked
casts and type tests, and owning inline-copy consumers. `.` never crosses a
shared edge: a raw `shared T` handle must be dereferenced before it can be
consumed as an object place.

Precedence, associativity, grouping syntax, and the accepted postfix chain are
defined by the [grammar](GRAMMAR.md#expressions). Statement legality and
evaluation order are defined by
[Functions and Control Flow](FUNCTIONS_AND_CONTROL_FLOW.md).

## Operators

The implemented arithmetic surface is deliberately exact-type:

| Operator | Accepted operand types | Result |
|---|---|---|
| binary `+`, `-`, `*` | two operands of the same type among `i64`, `u64`, `u8`, and `f64` | that same type |
| unary `-` | `i64` or `f64` | the operand type |

`u64` arithmetic wraps modulo 2^64. `u8` arithmetic wraps modulo 2^8, so every
result remains in `0..=255`. Signed `i64` overflow behavior is not yet a
language contract; code must not depend on a particular overflow result.

`f64` arithmetic follows IEEE-754 binary64 addition, subtraction,
multiplication, and negation in the default round-to-nearest, ties-to-even
environment. Signed zeroes, subnormals, infinities, and NaNs can result. An
unchanged value retains its binary64 value, but arithmetic does not guarantee a
particular NaN payload.

No equality, ordering, logical, division, remainder, bitwise, shift, or
exponentiation operator is implemented. Integer equality and ordering have the
frozen but not yet implemented contract below; floating equality and ordering
remain deferred. Built-in array indexing and slicing are intrinsic operations
rather than general operators; non-shared inline element access currently
executes for primitives, optionals, exact classes, and nested arrays on
x86-64. The same element categories execute in shared outer arrays, and copied
slices plus checked equal-length slice assignment execute for inline, shared,
and optional-shared receivers. Call-scoped whole-array and exact class or
nested-array element aliases execute with their declared read-only or mutable
access.

## Frozen primitive integer comparisons and casts

This section freezes the complete source-visible integer-only profile.
Comparison syntax, exact-type checking, typed HIR, and verified
target-independent MIR are implemented. The x86-64 target currently rejects
comparison MIR until backend realization lands, and primitive-keyword cast
targets remain unaccepted. The
[status matrix](STATUS.md#not-implemented) records availability separately
from the language contract.

### Integer comparisons

The comparison operators `==`, `!=`, `<`, `<=`, `>`, and `>=` accept exactly
two operands of the same type among `i64`, `u64`, and `u8`. Every comparison
produces `bool`.

Equality and inequality compare complete values. Ordering is signed for `i64`
and unsigned for `u64` and `u8`. Operand spelling does not affect the selected
operation after type checking.

Comparisons never promote, narrow, or reinterpret an operand. Mixed integer
types are errors, including otherwise representable literal values:
`1 == 1u` is invalid because its operands are `i64` and `u64`. A programmer
must cast one operand explicitly before comparing different integer types.

The two operands evaluate exactly once from left to right. All six operators
share one non-associative precedence level below arithmetic and above
contextual `is`; consequently an ungrouped chain such as `a < b < c` is a
syntax error. The [grammar](GRAMMAR.md#expressions) records the exact accepted
source shape.

### Explicit integer casts

An integer cast has unary syntax `(T) source`, where `T` is exactly `i64`,
`u64`, or `u8` and `source` has one of those same three types. All nine
source/target pairs are valid, including same-type identity casts. Casts do not
appear implicitly at initialization, assignment, argument, return, arithmetic,
or comparison boundaries.

Skald defines integer casts using fixed-width two's-complement bits. Casting to
an `N`-bit integer retains the source value modulo `2^N`, then interprets the
retained bits using the target signedness:

- an unsigned target denotes the retained value directly;
- an `i64` target denotes a retained value below `2^63` directly and otherwise
  denotes that value minus `2^64`.

This gives the complete conversion matrix:

| Source | `i64` target | `u64` target | `u8` target |
|---|---|---|---|
| `i64` | identity | preserve all 64 bits | retain the low 8 bits |
| `u64` | preserve all 64 bits and interpret the sign bit | identity | retain the low 8 bits |
| `u8` | zero-extend | zero-extend | identity |

For example:

```ska
(i64) 18446744073709551615u // -1
(u64) -1                   // 18446744073709551615u
(u8) 258u                  // 2u8
(u8) -1                    // 255u8
```

Every integer cast is total and evaluates its operand exactly once before
conversion. It cannot diagnose a target-range error, terminate, allocate,
invoke runtime support, or introduce exceptional control flow. A literal must
first be valid for the type selected by its spelling; applying an explicit
cast adds no further range check. The conversion rule is portable language
meaning, not a promise about memory layout, endianness, registers, or the
external ABI. It does not settle signed `i64` arithmetic overflow.

### Deferred conversion and comparison work

This contract does not define:

- floating-point comparisons or casts between floating and integer types;
- conversions between `bool` and numeric types;
- implicit numeric conversions or mixed-type comparisons;
- checked, saturating, or user-defined conversions;
- object, optional, array, `Obj`, or `unit` conversion through primitive casts;
- logical operators or equality for objects, owners, optionals, or arrays.

Those areas require separate design. In particular, no checked variant is
implied by the total integer cast syntax.

## Other conversions and future value families

The current compiler performs no primitive casts or user-defined conversions.
Integer comparisons are implemented through verified target-independent MIR;
native execution and the frozen integer-cast matrix remain later roadmap
steps. All other numeric conversion behavior remains deferred. Object casts
are defined separately in [Object Casts](OBJECT_CASTS.md): implemented plain
casts select checked object places, while shared casts preserve existing
allocations. Neither form reinterprets bytes.

Optional values have an [implemented contract](OPTIONAL_VALUES.md) for
representing absence without making every value nullable. Primitive and
exact-class `T?` and optional shared-owner `shared? T` values cross owning
locals, fields, and internal parameters/results with `none`, exact-value
injection, optional copy and assignment, initializer ranking, presence tests,
conditional lifecycle, and checked access. Alias parameters may designate
supported inline optional containers. Optionals have no truthiness and never
implicitly convert to their payload; optional references, aliases to optional
shared owners, and external optional signatures remain rejected.

Arrays have an [implemented contract](ARRAYS.md). The
built-in invariant `T[]` and `shared T[]` families distinguish deep-copying
inline values from shared allocations, use `u64` lengths and signed
negative-capable indices, copy slice reads, and preserve deterministic element
lifetime. Inline and shared-outer arrays support every documented owning
element category, construction, length, indexing, mutation, named deep copy,
produced-backing adoption, arbitrary-length inline replacement, copied slices,
call-scoped aliases, class fields, internal parameters/results, and cleanup on
x86-64. No structural indexing or iteration protocol is implied.

An immutable language-facing string value remains an exploratory direction,
but its type name, literal syntax and encoding, byte/text semantics, copying,
slicing, storage, and library boundary are not frozen. No representation or
literal-lowering strategy is a language guarantee.

Function values are not implemented or frozen. Shared ownership's implemented
non-null value type, compatible views, and copy/adopt/release behavior are
defined in [Shared Ownership and Heap Allocation](SHARED_OWNERSHIP.md).
Remaining exclusions are recorded in the
[status matrix](STATUS.md#not-implemented).
Implemented polymorphic views, slicing, tests, and checked casts are separated
into the [polymorphism profile](POLYMORPHISM.md). Legacy examples are not
usable syntax or settled semantics.

## Implementation boundary

Type meaning does not depend on storage size, alignment, register class, C
mapping, compiler IR, numeric parsing algorithm, or dump representation. Those
are compiler, backend, runtime, and debugging concerns. Implementations must
preserve the source-visible values and errors above, but may represent them in
any conforming way.
