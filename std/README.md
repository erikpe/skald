# Standard Library

This directory contains the installed Skald standard-library source. The
canonical `std::str::Str` module provides the frozen byte-string descriptor,
safe byte-copying construction, checked observation and slicing, independent
array conversion, read-only structural byte indexing and omitted-bound
descriptor slicing, byte equality against generic objects, concatenation, and
canonical formatting plus optional parsing for every primitive type. Its
integer methods delegate to type-named helpers in
`std::str::format_integer` and `std::str::parse_integer`; the descriptor keeps
no decimal integer algorithm of its own. Its `to_f64` facade recognizes
special values with ordinary string equality and
delegates finite conversion to `std::str::parse_f64`; `from_f64` similarly
keeps special spellings in the facade and delegates finite formatting to
`std::str::format_f64`. The exact standard-stream and primitive-line-output
APIs are implemented in Skald, with no scalar runtime observation surface.

The `std::f64` module provides exact `to_bits(f64) -> u64` and
`from_bits(u64) -> f64` value reinterpretation. Its public functions are
ordinary Skald wrappers over two private compiler intrinsics. Typed HIR,
verified MIR, and x86-64 lowering preserve every binary64 bit inline; the
module adds no allocation, foreign call, or runtime ABI surface.

The `std::io` module has an implemented
[whole-stream source contract](../docs/language/IO.md) and a separate
[compiler/runtime contract](../docs/compiler/IO.md). Its five private
byte-array declarations, compiler HIR/MIR, and x86-64 lowering are implemented,
and runtime ABI version 9 provides independently tested host byte operations.
All nine public functions are ordinary Skald library code over those private
`u8[]` intrinsics and the canonical primitive `Str` conversions. Reads own
geometric buffering, EOF loops, exact-length trimming, normal file close, and
the existing final `Str.from_bytes` copy. The exact conversion surface and text
contract are documented in
[Skald Strings](../docs/language/STRINGS.md#frozen-primitive-textual-conversions)
and its completed rollout is recorded in the archived
[primitive string conversions roadmap](../docs/archive/PRIMITIVE_STRING_CONVERSIONS_ROADMAP.md).

The `std::process` module has an implemented
[process-argument contract](../docs/language/PROCESS.md). Its `args()` function
composes existing `std::io`, raw-byte strings, arrays, loops, and modules to
read the Linux invocation vector. It introduces no compiler intrinsic,
entry-function parameter, static cache, or runtime ABI addition.

The dependency-free `std::lang` module provides the foundational `Equatable`
and `Hashable` interfaces. Equality accepts a read-only `Obj` view so an
implementation can return `false` for values outside its equality domain;
hashing returns one `u64`. Both declarations are ordinary explicitly imported
library interfaces without implicit prelude behavior or compiler machinery.

The `std::vec` module provides the implemented generic
[`Vec<T>` vector](../docs/language/VECTORS.md). It owns independent `T?[]`
capacity storage and infers its element lifecycle requirements from ordinary
method operations. Heterogeneous shared-object code uses `Vec<shared Obj>`.
The vector provides capacity, geometric growth, checked positive and negative
indexing through compatibility methods and structural brackets, independent
logical-length slices, equal-length snapshot slice replacement, push, pop,
last, replacement, and clear without compiler or runtime machinery.

The `std::error` module declares the compiler-known
`panic(message: std::str::Str) -> unit` intrinsic and imports `std::str`
explicitly. Call statements execute through the compiler's non-returning panic
terminator and the length-delimited runtime reporter. It is not an external
function or an exception API.

The `std::test` module provides ordinary source-level assertions for golden
and application tests. It exports exact equality helpers for `i64`, `u64`,
`u8`, `f64`, and `Str`, boolean truth helpers, and an unconditional `fail`
helper. Assertion failures format values through the canonical `Str`
conversions and terminate through `std::error::panic`; the module adds no
compiler intrinsic or runtime ABI surface. Floating-point equality uses
ordinary exact `f64` equality, so signed zeroes compare equal and NaN values
do not.

`std::str` selectively imports that panic declaration for invalid byte and
slice bounds, forming an ordinary two-module cycle with `std::error`. It also
imports the integer and binary64 formatting and parsing descendant modules in
one direction. The integer helpers depend only on primitive values and arrays.
The binary64 parser and formatter depend only on primitive values and arrays,
and independently import `std::f64` for bit conversion. Their private cached
powers are initialized directly as static `u64[]` fields before execution.

A module and descendant module may coexist: the `std/str.ska` source is
`std::str`, while files below `std/str/` are distinct descendant modules.
`std::str::format_integer` provides `format_i64`, `format_u64`, and
`format_u8`, each returning a fresh exact-length `shared u8[]`.
`std::str::parse_integer` provides the corresponding `parse_i64`, `parse_u64`,
and `parse_u8` functions over validated backing-array ranges. `Str` lends its
private backing only for each parse call; direct users can pass only arrays
they already possess.

The binary64 parser's public
`parse(ref storage: u8[], start: i64, length: u64) -> f64?`
validates its range and is available for direct decimal parsing. `Str.to_f64`
borrows its private backing only for that call; the call-scoped alias cannot
expose the backing to callers. The formatter's public `format(value: f64) ->
shared u8[]` requires a finite value because the `Str` facade owns special
spellings. It uses fixed-width Ryū arithmetic over a reusable private static
cached-power table set and allocates only the returned exact-length array per
value. The public
`BigUnsigned` class now lives in `std::str::parse_f64` beside its only consumer
and remains a narrow parser implementation entry point, not part of the
supported `Str` conversion surface. The parser keeps the existing exact small
path, uses fixed-width Eisel-Lemire conversion for ordinary decimals, and
rescans only ambiguous inputs into a 768-digit exact fallback. Its static
powers and arithmetic helpers are independent from the formatter. Third-party-
derived standard-library code is listed in
[Third-party notices](THIRD_PARTY.md).

Import and call the panic intrinsic as a standalone statement:

```ska
from std::error import panic;

fn main() -> i64 {
    panic("configuration is missing");
}
```

The reporter writes `panic: `, the exact string bytes, and a line feed to
standard error, then terminates unsuccessfully. It does not unwind or run
remaining cleanup.

Named private standard-library members begin with `_` by convention, including
private fields and private instance or static methods. Declarations without a
member name, such as `private init(...)`, are unchanged. Parameters and local
variables use ordinary descriptive names unless they independently require an
underscore.

The public string contract is documented in
[Skald Strings](../docs/language/STRINGS.md). Feature maturity and the
remaining broader library scope are tracked in the
[status matrix](../docs/language/STATUS.md#not-implemented).
