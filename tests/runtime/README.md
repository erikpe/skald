# Runtime Tests

Runtime tests are small C harnesses linked directly with the runtime archive.
They verify the [public runtime ABI](../../docs/compiler/RUNTIME_ABI.md)
independently of compiler code generation. The
[testing guide](../../docs/development/TESTING.md) owns general placement and
selection policy.

Run the current runtime suite from the repository root with:

```text
make runtime-test
```

The suite contains eight focused executables:

- `test_runtime_contract.c` checks link/version compatibility and platform
  requirements;
- `test_runtime_allocation.c` checks successful non-null, suitably aligned,
  writable allocations and exact-base deallocation;
- `test_runtime_allocation_failure.c` uses child processes to verify private
  hard failure without stderr for invalid sizes and the exact panic record for
  valid-request host exhaustion;
- `test_runtime_io.c` uses temporary files and pipes to verify standard
  handles, read-only open, close-on-exec, exact binary transfers, partial
  progress, EOF, host failures, normal close, and post-close behavior;
- `test_runtime_io_defects.c` uses child processes to verify hard failure for
  invalid selectors, modes, handles, and pointer/length pairs;
- `test_runtime_output.c` captures stdout and compares successful output
  records byte for byte;
- `test_runtime_output_failure.c` uses child processes to verify that every
  output operation terminates unsuccessfully after a write or flush failure;
  and
- `test_runtime_panic.c` verifies exact length-delimited panic records,
  reporter failure, and invalid-input hard failure.

The allocation-failure, I/O, output, and panic harnesses share only the small
error-reporting and exact-f64-bit construction helpers in
`runtime_test_support.c`. The Makefile builds and runs the runtime archive
before entering the runtime test Makefile, which retains the archive dependency
for direct use. It runs the harnesses in responsibility order so a failure
identifies the affected boundary directly.

The successful-output cases include integer boundaries, boolean values,
representative exact binary64 patterns, and mixed consecutive records. The
output-failure harness closes each child process's stdout descriptor before
invoking one output function. Passing tests write nothing to stdout or stderr
and compile under C11 with `-Wall -Wextra -Werror`.

Exact symbols, signatures, output bytes, versioning rules, and responsibility
boundaries belong only in the [runtime ABI](../../docs/compiler/RUNTIME_ABI.md).
