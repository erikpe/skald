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

Runtime ABI version 3 also exposes `ska_rt_println_bool(bool)`. The harness
checks exact lowercase `false` and `true` records and consecutive mixed calls.
The public header supplies the standard C `bool` type; runtime implementation
details such as `FILE *` remain private.

The harness additionally runs each output function in a child process whose
stdout descriptor is closed. Each child must terminate unsuccessfully, proving
that a detected write or flush failure cannot be reported as a successful
Skald runtime operation. Tests write nothing to stdout or stderr when they pass
and compile under C11 with `-Wall -Wextra -Werror`.
