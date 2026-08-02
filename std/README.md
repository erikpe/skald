# Standard Library

This directory contains the installed Skald standard-library source. The
canonical `std::str::Str` module provides the frozen byte-string descriptor,
safe byte-copying construction, checked observation and slicing, independent
array conversion, byte equality against generic objects, concatenation,
canonical boolean and integer formatting, and exact optional boolean and
integer parsing. Its `to_f64` facade recognizes special values with ordinary
string equality and delegates decimal conversion to the substantial correctly
rounded binary64 parser in the companion `std::str::parse_f64` module. The
exact standard-stream output API is implemented in Skald; bootstrap C runtime
print functions remain available for observability.

The `std::io` module has an implemented
[whole-stream source contract](../docs/language/IO.md) and a separate
[compiler/runtime contract](../docs/compiler/IO.md). Its five private
byte-array declarations, compiler HIR/MIR, and x86-64 lowering are implemented,
and runtime ABI version 7 provides independently tested host byte operations.
All four public functions are ordinary Skald library code over those private
`u8[]` intrinsics. Reads own geometric buffering, EOF loops, exact-length
trimming, normal file close, and the existing final `Str.from_bytes` copy.
This module does not yet replace the observability helpers. Boolean and integer
formatting and optional parsing plus binary64 parsing are implemented, while
binary64 formatting remains frozen. Their exact surface and text contract are
documented in
[Skald Strings](../docs/language/STRINGS.md#frozen-primitive-textual-conversions)
and scheduled by the active
[primitive string conversions roadmap](../docs/roadmaps/PRIMITIVE_STRING_CONVERSIONS_ROADMAP.md).

The `std::error` module declares the compiler-known
`panic(message: std::str::Str) -> unit` intrinsic and imports `std::str`
explicitly. Call statements execute through the compiler's non-returning panic
terminator and the length-delimited runtime reporter. It is not an external
function or an exception API.

`std::str` selectively imports that panic declaration for invalid byte and
slice bounds, forming an ordinary two-module cycle with `std::error`. It also
imports `std::str::parse_f64` in one direction. The parser companion depends
only on primitive values and a borrowed `u8[]` range; it does not import `Str`.
The error cycle has no initialization-order consequences because modules
contain no executable top-level state.

A module and descendant module may coexist: the `std/str.ska` source is
`std::str`, while `std/str/parse_f64.ska` is the distinct
`std::str::parse_f64` module. The companion's public `parse(ref storage: u8[],
start: i64, length: u64) -> f64?` validates its range and is available for
direct decimal parsing. `Str.to_f64` borrows its private backing only for that
call; the call-scoped alias cannot expose the backing to callers. All stateless
parser support is private module code, and the stateful bounded unsigned
arithmetic type remains private to the companion.

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
