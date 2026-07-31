# Skald Types, Values, and Expressions

Status: authoritative for implemented type, value, literal, and expression
semantics, the implemented primitive operator profile, the implemented
complete explicit primitive cast matrix, and the exact type rule for primitive
binding reassignment. The [status
matrix](STATUS.md) is authoritative for feature maturity, and the [implemented
grammar](GRAMMAR.md) defines accepted source syntax.

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
- assignment to primitive `var` locals;
- assignment to primitive value parameters;
- both operands of a binary arithmetic operator.

Primitive binding reassignment applies the same rule: the right-hand
expression's actual type must be identical to the `var` local or value
parameter's declared primitive type. A literal keeps the type selected by its
spelling; the destination does not reinterpret it.

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

### Implemented surface

The implemented arithmetic surface is deliberately exact-type:

| Operator | Accepted operand types | Result |
|---|---|---|
| binary `+`, `-`, `*` | two operands of the same type among `i64`, `u64`, `u8`, and `f64` | that same type |
| binary `/` | two operands of the same type among `i64`, `u64`, `u8`, and `f64` | that same type |
| binary `%` | two operands of the same type among `i64`, `u64`, and `u8` | that same type |
| unary `-` | `i64` or `f64` | the operand type |
| prefix `!` | `bool` | `bool` |
| prefix `~` | `i64`, `u64`, or `u8` | the operand type |
| binary `&`, `|`, `^` | two operands of the same type among `i64`, `u64`, and `u8` | that same type |
| `<<`, `>>` | left operand of `i64`, `u64`, or `u8`; right operand exactly `u64` | the left type |
| `==`, `!=` | two operands of the same type among `i64`, `u64`, `u8`, `f64`, and `bool` | `bool` |
| `<`, `<=`, `>`, `>=` | two operands of the same type among `i64`, `u64`, `u8`, and `f64` | `bool` |
| `&&`, `||` | two `bool` operands | `bool` |

Integer arithmetic wraps modulo its width: `i64` and `u64` retain the low
64 bits, while `u8` retains the low 8 bits and remains in `0..=255`. The
complete overflow contract is stated in the operator profile below.

`f64` arithmetic follows IEEE-754 binary64 addition, subtraction,
multiplication, division, and negation in the default round-to-nearest,
ties-to-even environment. Signed zeroes, subnormals, infinities, and NaNs can result. An
unchanged value retains its binary64 value, but arithmetic does not guarantee a
particular NaN payload.

Integer and floating equality and ordering plus boolean equality, inequality,
logical negation, and short-circuit `&&` and `||` are implemented as specified
below. Floating remainder and exponentiation are not implemented. Built-in
array indexing and slicing
are intrinsic operations rather than general operators; non-shared inline
element access currently executes for primitives, optionals, exact classes,
and nested arrays on x86-64. The same element categories execute in shared
outer arrays, and copied slices plus checked equal-length slice assignment
execute for inline, shared, and optional-shared receivers. Call-scoped
whole-array and exact class or nested-array element aliases execute with their
declared read-only or mutable access.

### Implemented primitive operator profile

The complete primitive operator profile below is an implemented language
contract. The [status matrix](STATUS.md) records its maturity, and the
[grammar](GRAMMAR.md#implemented-primitive-operator-expressions) records its
accepted syntax and precedence.

The unary matrix is:

| Operator | Operand | Result | Meaning |
|---|---|---|---|
| `-` | `i64` | `i64` | Wrapping two's-complement negation |
| `-` | `f64` | `f64` | IEEE-754 sign negation |
| `!` | `bool` | `bool` | Logical negation |
| `~` | `i64`, `u64`, or `u8` | Operand type | Bitwise complement within the operand width |
| `*` | supported `shared T` owner | Existing pointee-place result | Existing explicit shared dereference |

Unary `+` is not part of this profile.

The binary primitive matrix is:

| Operators | Left operand | Right operand | Result |
|---|---|---|---|
| `+`, `-`, `*`, `/` | One numeric type | The identical type | The operand type |
| `%` | One integer type | The identical type | The operand type |
| `&`, `|`, `^` | One integer type | The identical type | The operand type |
| `<<`, `>>` | `i64`, `u64`, or `u8` | `u64` | The left type |
| `==`, `!=` | One primitive type | The identical type | `bool` |
| `<`, `<=`, `>`, `>=` | One numeric type | The identical type | `bool` |
| `&&`, `||` | `bool` | `bool` | `bool` |

Here, numeric means `i64`, `u64`, `u8`, or `f64`; integer means `i64`, `u64`,
or `u8`. Boolean values have logical operations and equality but no ordering,
arithmetic, shifts, or bitwise operations. `unit`, optionals, arrays, class
values, object views, and shared-owner handles receive no new operator.

#### Exact selection and explicit casts

Operator selection never performs an implicit cast, promotion, narrowing,
signedness change, boolean conversion, truthiness conversion, or expected-type
reinterpretation. Except for shifts, both operands must have the identical
static type. A shift has an integer left operand and exactly `u64` on the
right.

An explicit cast completes according to its own contract and, on success,
produces an ordinary value with exactly its target type before a surrounding
operator is selected or executed. Operator selection sees only resulting
static types and cannot observe whether a value came from a literal, binding,
call, cast, or another expression. An operator never inserts or requests a
cast.

Adding an explicit source/target cast pair may make more operand expressions
constructible, but does not add a mixed-type operator case or change an
existing result or operation:

```ska
1 + 1u                   // invalid: i64 and u64
(i64) 1u8 + 2            // valid i64 addition
(i64) (255u8 + 1u8)      // valid: u8 addition wraps, then converts to i64
1u8 << 3u                // valid; result is u8
1u8 << 3u8               // invalid; count must be u64
```

The [complete explicit primitive cast matrix](#frozen-complete-explicit-primitive-cast-matrix)
is frozen independently of this operator design. Its implementation does not
revise this boundary. Implicit conversion, mixed-type operator resolution, or
contextual literal typing would revise the boundary and requires a separate
design.

An unsupported operator/type combination is a compile-time error. Diagnostics
identify the operator and incompatible operand types, but the language does
not freeze a diagnostic code, exact wording, follow-on count, or ordering
between independent errors.

#### Integer arithmetic and overflow

`i64`, `u64`, and `u8` addition, subtraction, and multiplication wrap modulo
their width. Unary `i64` negation uses the same rule, so negating the minimum
`i64` value returns that value unchanged. An `i64` result retains the low
64 bits and interprets them as two's-complement; `u64` retains the low 64 bits;
and `u8` retains the low 8 bits and remains canonical in `0..=255`.

Overflow does not panic, produce an invalid value, depend on build mode, or
expose a target overflow flag. Compile-time evaluation and runtime execution
must agree.

Unsigned integer division and remainder have the ordinary nonnegative
quotient and remainder. Signed division rounds the mathematical quotient
toward negative infinity. Signed remainder satisfies
`remainder = dividend - quotient * divisor` under the type's wrapping
arithmetic and is zero or has the divisor's sign:

| Expression | Result |
|---|---|
| `7 / 3` | `2` |
| `7 % 3` | `1` |
| `-7 / 3` | `-3` |
| `-7 % 3` | `2` |
| `7 / -3` | `-3` |
| `7 % -3` | `-2` |
| `-7 / -3` | `2` |
| `-7 % -3` | `-1` |
| `-9223372036854775808 / -1` | `-9223372036854775808` |
| `-9223372036854775808 % -1` | `0` |

Integer `/` and `%` panic when the divisor is zero. The signed-minimum pair is
handled before any target instruction that could fault; raw hardware faults
do not implement Skald semantics.

#### Bitwise operations and shifts

`&`, `|`, `^`, and `~` operate on the exact fixed-width representation of
their integer operand. Left shift inserts zero low bits and discards high bits.
Right shift is arithmetic for `i64` and logical for `u64` and `u8`.

Counts from `0u` through `63u` are valid for `i64` and `u64`; counts from `0u`
through `7u` are valid for `u8`. A count at or above the left operand's width
panics. Skald never masks an excessive count to target instruction count bits.
Every `u8` result is canonicalized.

#### Floating-point operations and comparisons

`f64` unary negation and `+`, `-`, `*`, and `/` follow IEEE-754 binary64 in the
existing round-to-nearest, ties-to-even environment. Division by floating zero
does not panic; it produces the applicable signed infinity or NaN. Overflow,
underflow, signed zero, subnormal, infinity, and NaN results follow the
corresponding binary64 operation.

Floating equality and ordering are unordered when either operand is NaN:

| Operator | Result when either operand is NaN |
|---|---|
| `==` | `false` |
| `!=` | `true` |
| `<`, `<=`, `>`, `>=` | `false` |

Positive and negative zero compare equal. Infinities use ordinary numeric
ordering. These operators do not define a total order. Skald does not promise
a particular NaN sign or payload, preservation of signaling NaN state, or
source-visible floating exception flags.

Floating `%` is not part of this profile.

#### Boolean operations

`bool` supports prefix `!`, exact equality and inequality, and mandatory
short-circuit `&&` and `||`. There is no truthiness conversion from numeric,
optional, owner, array, class, interface, `Obj`, or `unit` values.

Logical evaluation, source order, skipped effects, and temporary lifetime are
defined by
[Functions and Control Flow](FUNCTIONS_AND_CONTROL_FLOW.md#short-circuit-logical-expressions).
Integer zero-divisor and excessive-shift failure are defined by
[Errors and Exceptional Control Flow](ERRORS.md#implemented-operator-failures).

#### Deferred operator and conversion work

This profile does not define:

- power or exponentiation;
- floating remainder;
- operators on `Str`, other class values, interfaces, `Obj`, shared owners,
  optionals, arrays, or future value families;
- user-defined operator declarations or overload resolution;
- implicit numeric promotion or mixed-type operators;
- total floating-point ordering or NaN payload facilities;
- checked, saturating, arbitrary-precision, or selectable overflow modes;
- rotations or integer bit utilities;
- compound assignment, increment, decrement, or assignment expressions;
- `is not`, identity equality, or object value equality; or
- coalescing, conditional, pipeline, range, SIMD, atomic, volatile, or
  concurrency operators.

Each area requires a separate design. No deferred syntax is reserved.

## Implemented integer bitwise and shift operators

Prefix `~` accepts exactly one `i64`, `u64`, or `u8` operand, complements every
bit within that type's fixed width, and returns the same type. Binary `&`, `|`,
and `^` accept exactly two operands of the same integer type and return that
type. `bool`, `f64`, `unit`, optionals, arrays, class values, object views, and
shared owners are not bitwise operands.

`<<` and `>>` accept an `i64`, `u64`, or `u8` left operand and exactly `u64`
on the right, and return the left type. Left shift inserts zero low bits and
discards high bits. Right shift is arithmetic for `i64` and logical for `u64`
and `u8`. Counts at or above the left width terminate with
`shift count out of range`; constants use the same runtime path as dynamic
counts and are never masked to a target instruction's count width.

No bitwise operation inserts an implicit cast, promotion, narrowing,
signedness change, expected-type reinterpretation, or truthiness conversion.
An explicit integer cast completes first and supplies its exact result type to
surrounding operation selection. Every `u8` result is canonical in `0..=255`.

A unary operand evaluates exactly once. Binary operands evaluate exactly once
from left to right, including calls, fields, checked array accesses, and
optional unwrap. Both shift operands complete before the count check. Pure
bitwise operations introduce no failure of their own,
runtime call, allocation, cleanup rule, or control-flow edge; an operand's
existing effects and failures remain unchanged. Successful shifts retain the
ordinary full-expression cleanup boundary; their non-returning failure path
uses the language's existing non-unwinding panic contract.

Postfix operations bind before prefix `~`, and prefix operators associate
right to left. Additive expressions bind before the left-associative shift
tier, followed by the separate left-associative `&`, `^`, and `|` tiers.
Those tiers bind before comparisons, contextual `is`,
and short-circuit `&&` and `||`. The exact accepted ladder is in the
[implemented grammar](GRAMMAR.md#expressions).

## Implemented integer division and remainder

Binary `/` and `%` accept exactly two operands of the same type among `i64`,
`u64`, and `u8`, and return that identical type. They do not insert an
implicit cast, promotion, narrowing, signedness change, expected-type
reinterpretation, or truthiness conversion. Floating division and remainder
are specified separately: floating division is implemented below, while
floating remainder remains unavailable.

Unsigned division and remainder use the ordinary nonnegative quotient and
remainder. Signed `i64` division rounds toward negative infinity; its
remainder is zero or has the divisor's sign and satisfies
`remainder = dividend - quotient * divisor` under wrapping arithmetic. The
defined `i64::MIN / -1` and `i64::MIN % -1` results are `i64::MIN` and zero,
not failures.

Operands evaluate exactly once from left to right. Both complete before the
divisor check, so an earlier operand failure occurs first. A zero divisor
terminates through the operation-specific `integer division by zero` or
`integer remainder by zero` panic reason; a literal zero follows the same
runtime path as a dynamic zero. Successful temporaries retain the ordinary
full-expression cleanup boundary.

`*`, `/`, and `%` form one left-associative multiplicative tier above `+` and
`-`. The operations compose with arbitrary valid operands and all ordinary
expression consumers. Their typed representation, explicit checked MIR
diamond, and x86-64 realization are described by the compiler phase and
backend contracts.

## Implemented floating division

Binary `/` accepts exactly two `f64` operands and returns `f64`. It inserts no
implicit conversion or promotion and does not weaken the exact integer
division rules above. Operands evaluate exactly once from left to right, and
their temporaries retain the ordinary full-expression cleanup boundary.

Division follows IEEE-754 binary64 behavior in the default round-to-nearest,
ties-to-even environment. Overflow, gradual underflow, subnormal results,
signed zero, infinity, and NaN are ordinary results. A positive or negative
zero divisor does not panic: a nonzero numerator produces the appropriately
signed infinity, while zero divided by zero produces NaN. No particular NaN
payload, sign, or signaling state is promised.

Floating division shares the left-associative multiplicative tier with `*`
and `%` and composes with every ordinary expression consumer. Its typed HIR,
verified non-failing MIR operation, and x86-64 realization introduce no
runtime call, failure edge, or ABI change.

## Implemented primitive comparisons, boolean negation, and integer casts

This section defines the implemented source-visible comparison, eager boolean,
and integer-cast profile. Syntax, exact-type checking, typed HIR, and verified
target-independent MIR are implemented for these operations, and they execute
through the x86-64 target. The
[status matrix](STATUS.md#implemented-language) records availability
separately from the language contract.

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

The two operands evaluate exactly once from left to right. In the implemented
grammar all six operators share one non-associative level above contextual
`is`; the implemented operator grammar places both forms in one non-associative
comparison tier. Both reject an ungrouped chain such as `a < b < c`. The
[grammar](GRAMMAR.md#expressions) defines the accepted source shape.

### Floating comparisons

The same six comparison operators accept exactly two `f64` operands and
produce `bool`. They use IEEE-754 unordered comparison semantics: if either
operand is NaN, `==` is false, `!=` is true, and every ordering predicate is
false. Positive and negative zero compare equal, and infinities follow
ordinary numeric ordering. These predicates do not define a total order.

Operands evaluate exactly once from left to right. Comparisons introduce no
failure edge or runtime call, produce canonical booleans, and compose with the
ordinary boolean and control-flow consumers, including short-circuit `&&` and
`||`. Mixed floating/integer comparisons remain errors; Skald inserts no
promotion or conversion.

### Boolean negation and equality

Prefix `!` accepts exactly one `bool` operand and produces its logical
negation. Boolean `==` and `!=` accept two `bool` operands and compare their
values. Boolean ordering is invalid, and none of these operations introduces
truthiness or an implicit conversion.

A unary operand evaluates exactly once. Equality operands evaluate exactly
once from left to right. Postfix optional unwrap binds first, so
`!optional_flag!` negates the extracted boolean and retains the unwrap's
existing checked failure behavior. These eager operations produce canonical
`bool` values and add no runtime ABI.

`&&` and `||` accept exactly two `bool` operands and produce a canonical
`bool`. They use the implemented short-circuit evaluation and selected-path
cleanup rules in
[Functions and Control Flow](FUNCTIONS_AND_CONTROL_FLOW.md#short-circuit-logical-expressions);
they are never approximated with eager scalar evaluation.

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
external ABI. Integer arithmetic overflow is defined independently by the
[implemented primitive operator profile](#implemented-primitive-operator-profile).

## Frozen complete explicit primitive cast matrix

The complete primitive cast design is implemented. Source programs, typed
HIR, verified MIR, and x86-64 execution support all twenty-five cells. The
three checked `f64`-to-integer cells use explicit verified success/failure
control flow; the other twenty-two cells are pure value operations.
An explicit primitive cast retains unary syntax `(T) source`, where `T` and
the source type are each exactly one of `i64`, `u64`, `u8`, `f64`, and `bool`.
All twenty-five source/target pairs are valid, including every same-type
identity cast. `unit` is not a value type and never participates.

The complete matrix is:

| Source | `i64` target | `u64` target | `u8` target | `f64` target | `bool` target |
|---|---|---|---|---|---|
| `i64` | identity | preserve all 64 bits | retain the low 8 bits | signed numeric conversion | false only for zero |
| `u64` | preserve all 64 bits and interpret the sign bit | identity | retain the low 8 bits | unsigned numeric conversion | false only for zero |
| `u8` | zero-extend | zero-extend | identity | exact numeric conversion | false only for zero |
| `f64` | checked truncation toward zero | checked truncation toward zero | checked truncation toward zero | identity | false only for positive or negative zero |
| `bool` | false/true become 0/1 | false/true become 0/1 | false/true become 0/1 | false/true become 0.0/1.0 | identity |

The already implemented integer-to-integer cells retain their exact
[two's-complement and modulo contract](#explicit-integer-casts). They remain
total and cannot fail.

Integer-to-`f64` conversion uses the source integer's signedness and produces
the correctly rounded nearest IEEE-754 binary64 value, with ties to an even
significand. Precision loss is allowed. Every `u8` value is represented
exactly; larger `i64` and `u64` values need not be. Boolean-to-`f64` conversion
is exact.

Conversion to `bool` is a value comparison, not implicit truthiness. Integer
zero becomes `false` and every nonzero integer becomes `true`. Positive and
negative floating zero become `false`; every other binary64 value becomes
`true`, including subnormals, infinities, and every NaN. These rules do not
allow a numeric, optional, owner, array, class, interface, `Obj`, or `unit`
expression directly as a condition: the explicit cast must first produce a
`bool` value.

An `f64`-to-integer cast first requires a finite source, then truncates its
mathematical value toward zero, and finally checks that truncated integer
against the target range. The accepted result ranges are `-2^63..=2^63-1`
for `i64`, `0..=2^64-1` for `u64`, and `0..=255` for `u8`. A finite negative
fraction greater than `-1.0` therefore truncates to zero and is valid for an
unsigned target. NaN, either infinity, or a truncated value outside the target
range terminates through the common unrecoverable-failure boundary with the
exact catalog message `floating-point cast out of range`; it never produces a
value or resumes Skald execution.

A floating literal must first be valid under its ordinary literal contract.
After that, a known failing cast remains the same source-reachable runtime
failure as a cast of a dynamically produced value; it is not a new literal
range diagnostic. Representative results are:

```ska
(u64) -0.5  // 0u
(u8) 255.9  // 255u8
(i64) -7.9  // -7
(bool) -0.0 // false
```

Identity casts return the unchanged value. In particular, an `f64` identity
cast preserves its complete binary64 datum, including signed zero, infinity,
and NaN payload and sign. All primitive casts evaluate their source exactly
once before conversion. Non-failing casts are pure value operations. A
potentially failing `f64`-to-integer cast is control-affecting even when its
source is otherwise pure; its failure is non-catchable and guarantees no
remaining source-level cleanup after reporting begins.

Primitive casts remain explicit at initialization, assignment, argument,
return, arithmetic, comparison, condition, and every other typed boundary. A
cast completes before a surrounding operator is selected, and the operator
observes only the cast's exact result type. No cast allocates, changes
ownership, or creates a runtime-managed value.

## Deferred conversion work

The complete primitive matrix does not define:

- implicit numeric conversion, promotion, or contextual literal typing;
- saturating, wrapping floating-to-integer, optional-result, or otherwise
  recoverable primitive conversion;
- user-defined conversion declarations;
- object, optional, array, `Obj`, shared-owner, or `unit` conversion through
  primitive casts; or
- conversion syntax or semantics for future primitive types.

Those areas require separate designs. The checked failure built into the three
`f64`-to-integer cells does not imply a general checked-cast syntax or a
recoverable conversion family.

## Other conversions and future value families

The current compiler executes primitive integer division and remainder,
floating division, bitwise and shift operations, comparisons, all twenty-five
primitive casts, and boolean negation and equality through the x86-64 backend.
The twenty-two non-failing cast cells use verified pure MIR; the three checked
`f64`-to-integer cells use verified success/failure control flow. The compiler
performs no user-defined conversions. Object casts are defined separately in
[Object Casts](OBJECT_CASTS.md): implemented plain casts select checked object
places, while shared casts preserve existing allocations. Neither form
reinterprets bytes.

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
