# Standard Library

This directory contains the installed Skald standard-library source. The
canonical `std::str::Str` module provides the frozen byte-string descriptor,
safe byte-copying construction, checked observation and slicing, independent
array conversion, and concatenation. The exact standard-stream output API is
implemented in Skald; bootstrap C runtime print functions remain available
for observability.

The `std::io` module has a frozen
[whole-stream source contract](../docs/language/IO.md) and a separate
[compiler/runtime contract](../docs/compiler/IO.md). Its five private
byte-array declarations, compiler HIR/MIR, and x86-64 lowering are implemented,
and runtime ABI version 7 provides independently tested host byte operations.
`write_stdout` and `write_stderr` are ordinary Skald library loops over those
private `u8[]` intrinsics; `read_stdin` and `read_file` remain planned. This
module neither replaces the observability helpers nor adds formatting,
parsing, or new string conversions.

The `std::error` module declares the compiler-known
`panic(message: std::str::Str) -> unit` intrinsic and imports `std::str`
explicitly. Call statements execute through the compiler's non-returning panic
terminator and the length-delimited runtime reporter. It is not an external
function or an exception API.

`std::str` selectively imports that panic declaration for invalid byte and
slice bounds, forming an ordinary two-module import cycle with `std::error`.
The cycle has no initialization-order consequences because modules contain no
executable top-level state.

Import and call it as a standalone statement:

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
