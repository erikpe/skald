# Runtime Tests

Runtime tests are small C harnesses linked directly with the runtime archive.
They verify the public C ABI independently of compiler code generation.
Shared ownership and object-layout tests arrive with those runtime features.

Run the current runtime suite from the repository root with:

```text
make runtime-test
```

`test_runtime_abi.c` verifies that a consumer compiled against the public
header observes the same ABI version from the linked runtime archive. It also
captures stdout and compares the exact bytes produced by
`ska_rt_println_i64(int64_t)` for zero, positive and negative values,
`INT64_MIN`, `INT64_MAX`, and consecutive calls. The expected representation
uses ASCII decimal digits followed by exactly one LF per call.

Runtime ABI version 3 introduced `ska_rt_println_bool(bool)`. The harness
checks exact lowercase `false` and `true` records and consecutive mixed calls.
The public header supplies the standard C `bool` type; runtime implementation
details such as `FILE *` remain private.

Runtime ABI version 4 adds `ska_rt_println_u64(uint64_t)`,
`ska_rt_println_u8(uint8_t)`, and `ska_rt_println_f64_bits(double)`. The harness
checks unsigned zero, one, representative values and maxima; positive and
negative binary64 zero; an exact fraction; minimum subnormal and maximum finite
values; infinity; and a retained quiet-NaN payload. It also checks consecutive
records mixed with the older output operations. C11 compile-time checks require
IEC 60559 semantics, eight-bit bytes, and the binary radix, significand width,
exponent range, and storage size needed for IEEE-754 binary64.

The harness additionally runs each output function in a child process whose
stdout descriptor is closed. Every child must terminate unsuccessfully, proving
that a detected write or flush failure cannot be reported as a successful
Skald runtime operation. Tests write nothing to stdout or stderr when they pass
and compile under C11 with `-Wall -Wextra -Werror`.
