# Skald Types, Values, and Expressions

Status: authoritative for implemented type, value, literal, and expression
semantics. The [status matrix](STATUS.md) is authoritative for feature maturity,
and the [implemented grammar](GRAMMAR.md) defines accepted source syntax.

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
| unary or binary arithmetic | A value of the operand type under the rules below. |
| construction | An exact-class object producer restricted to supported object contexts. |

Using a complete class binding, `self`, or class-typed field as an ordinary
scalar expression is an error. Those forms can still be valid object places in
the contexts described above. Calls through arbitrary expression values and
function values are not implemented; calls select named functions or methods.

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

No equality, ordering, logical, division, remainder, bitwise, shift,
exponentiation, indexing, or slicing operator is implemented. In particular,
even primitive equality is currently unavailable in source.

## Conversions and future value families

The language performs no primitive casts or user-defined conversions. Numeric
conversion behavior, including integer width changes and numeric/boolean
conversion, is not frozen. Object casts are defined separately in
[Object Casts](OBJECT_CASTS.md): implemented plain casts select checked object
places, while shared casts preserve existing allocations. Neither form
reinterprets bytes.

Optional values are an exploratory direction for representing absence without
making every value nullable. Their type syntax, empty value, presence checks,
extraction, conversions, payload lifetime, and lifecycle behavior are open;
older `T?` and `none` examples are not reserved syntax.

Arrays are an open design area. Element lifetime, size and mutability, storage,
construction, indexing, slicing, bounds failure, borrowing, and iteration must
be designed together. No legacy bracket form or structural protocol is a
current contract.

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
