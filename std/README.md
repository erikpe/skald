# Standard Library

This directory contains the installed Skald standard-library source. The
canonical `std::str::Str` module provides the frozen byte-string descriptor,
safe byte-copying construction, checked observation and slicing, independent
array conversion, byte equality against generic objects, concatenation, and
canonical formatting plus optional parsing for every primitive type. Its
`to_f64` facade recognizes special values with ordinary string equality and
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
and runtime ABI version 8 provides independently tested host byte operations.
All nine public functions are ordinary Skald library code over those private
`u8[]` intrinsics and the canonical primitive `Str` conversions. Reads own
geometric buffering, EOF loops, exact-length trimming, normal file close, and
the existing final `Str.from_bytes` copy. The exact conversion surface and text
contract are documented in
[Skald Strings](../docs/language/STRINGS.md#frozen-primitive-textual-conversions)
and its completed rollout is recorded in the archived
[primitive string conversions roadmap](../docs/archive/PRIMITIVE_STRING_CONVERSIONS_ROADMAP.md).

The `std::error` module declares the compiler-known
`panic(message: std::str::Str) -> unit` intrinsic and imports `std::str`
explicitly. Call statements execute through the compiler's non-returning panic
terminator and the length-delimited runtime reporter. It is not an external
function or an exception API.

`std::str` selectively imports that panic declaration for invalid byte and
slice bounds, forming an ordinary two-module cycle with `std::error`. It also
imports `std::str::parse_f64` and `std::str::format_f64` in one direction. The
parser depends on primitives, arrays, and `std::str::bigunsigned_helper`. The
formatter imports `std::f64` for exact bit decomposition and `Str` only to read
the immortal compact-table literal during lazy initialization. That reciprocal
`std::str` import is an ordinary source cycle, not a backing-storage escape.
The error and formatter cycles have no eager initialization order: the
formatter publishes its private cached-power array only on first use.

A module and descendant module may coexist: the `std/str.ska` source is
`std::str`, while files below `std/str/` are distinct descendant modules. The
parser's public `parse(ref storage: u8[], start: i64, length: u64) -> f64?`
validates its range and is available for direct decimal parsing. `Str.to_f64`
borrows its private backing only for that call; the call-scoped alias cannot
expose the backing to callers. The formatter's public `format(value: f64) ->
shared u8[]` requires a finite value because the `Str` facade owns special
spellings. It uses fixed-width Ryū arithmetic, lazily decodes one 832-byte
encoding held in five immortal literal sections into a reusable private static
table, and allocates only the returned exact-length array per value. The public
`BigUnsigned` helper remains
a narrow parser implementation entry point, not part of the supported `Str`
conversion surface. Third-party-derived standard-library code is listed in
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
