# Standard Library

This directory contains the installed Skald standard-library source. The
canonical `std::str::Str` module provides the frozen byte-string descriptor,
safe byte-copying construction, checked observation and slicing, independent
array conversion, and concatenation. Current output operations remain
bootstrap C runtime functions rather than the final I/O API.

The public string contract is documented in
[Skald Strings](../docs/language/STRINGS.md). Broader library scope remains an
[open design area](../docs/language/STATUS.md#not-implemented).
