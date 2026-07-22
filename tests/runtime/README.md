# Runtime Tests

Runtime tests are small C harnesses linked directly with the runtime archive.
They verify the [public runtime ABI](../../docs/compiler/RUNTIME_ABI.md)
independently of compiler code generation.

Run the current runtime suite from the repository root with:

```text
make runtime-test
```

The suite contains three focused executables:

- `test_runtime_contract.c` checks link/version compatibility and platform
  requirements;
- `test_runtime_output.c` captures stdout and compares successful output
  records byte for byte;
- `test_runtime_output_failure.c` uses child processes to verify that every
  output operation terminates unsuccessfully after a write or flush failure.

The two output harnesses share only the small error-reporting and exact-f64-bit
construction helpers in `runtime_test_support.c`. The Makefile builds and runs
the contract, successful-output, and failure harnesses in that order so a
failure identifies the responsible boundary directly.

The successful-output cases include integer boundaries, boolean values,
representative exact binary64 patterns, and mixed consecutive records. The
failure harness closes each child process's stdout descriptor before invoking
one output function. Passing tests write nothing to stdout or stderr and
compile under C11 with `-Wall -Wextra -Werror`.

Exact symbols, signatures, output bytes, versioning rules, and responsibility
boundaries belong only in the [runtime ABI](../../docs/compiler/RUNTIME_ABI.md).
