# Runtime Tests

Runtime tests are small C harnesses linked directly with the runtime archive.
They verify the public C ABI independently of compiler code generation.
Shared ownership and object-layout tests arrive with those runtime features.

Run the current runtime suite from the repository root with:

```text
make runtime-test
```

The suite contains three focused executables:

- `test_runtime_contract.c` links and calls the version-4 marker, verifies the
  inspectable ABI version, and checks the C platform properties required by
  Skald's primitive representations;
- `test_runtime_output.c` captures stdout and compares successful output
  records byte for byte;
- `test_runtime_output_failure.c` uses child processes to verify that every
  output operation terminates unsuccessfully after a write or flush failure.

The two output harnesses share only the small error-reporting and exact-f64-bit
construction helpers in `runtime_test_support.c`. The Makefile builds and runs
the contract, successful-output, and failure harnesses in that order so a
failure identifies the responsible boundary directly.

The successful-output harness compares the exact bytes produced by
`ska_rt_println_i64(int64_t)` for zero, positive and negative values,
`INT64_MIN`, `INT64_MAX`, and consecutive calls. The expected representation
uses ASCII decimal digits followed by exactly one LF per call.

The current runtime ABI includes `ska_rt_println_bool(bool)`. The output harness
checks exact lowercase `false` and `true` records and consecutive mixed calls.
The public header supplies the standard C `bool` type; runtime implementation
details such as `FILE *` remain private.

Runtime ABI version 4 also includes `ska_rt_println_u64(uint64_t)`,
`ska_rt_println_u8(uint8_t)`, and `ska_rt_println_f64_bits(double)`. The output
harness checks unsigned zero, one, representative values and maxima; positive
and negative binary64 zero; an exact fraction; minimum subnormal and maximum
finite values; infinity; and a retained quiet-NaN payload. It also checks
consecutive records mixed with the older output operations. C11 compile-time
checks require IEC 60559 semantics, eight-bit bytes, and the binary radix,
significand width, exponent range, and storage size needed for IEEE-754
binary64.

The public header names the version-specific no-op marker `ska_rt_abi_v4`
through `SKALD_RUNTIME_ABI_MARKER`. Compiler-generated entry wrappers call it
before Skald code. A stale archive therefore leaves a normal undefined symbol
and fails during linking. An incompatible ABI change must rename the marker;
the numeric `ska_rt_abi_version()` query remains useful for inspection but is
not the link guard.

Native compiler goldens reuse these directly tested symbols to cover locals,
internal calls, mixed integer/SSE register and stack placement, and consecutive
cross-type records.
This keeps representation testing in the runtime harness and code-generation
testing in the golden suite without duplicating implementation paths.

The failure harness runs each output function in a child process whose
stdout descriptor is closed. Every child must terminate unsuccessfully, proving
that a detected write or flush failure cannot be reported as a successful
Skald runtime operation. Tests write nothing to stdout or stderr when they pass
and compile under C11 with `-Wall -Wextra -Werror`.
