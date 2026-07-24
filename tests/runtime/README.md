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

The suite contains five focused executables:

- `test_runtime_contract.c` checks link/version compatibility and platform
  requirements;
- `test_runtime_allocation.c` checks successful non-null, suitably aligned,
  writable allocations and exact-base deallocation;
- `test_runtime_allocation_failure.c` uses child processes to verify fatal
  zero-size, host-unrepresentable-size when applicable, and allocator-failure
  paths;
- `test_runtime_output.c` captures stdout and compares successful output
  records byte for byte;
- `test_runtime_output_failure.c` uses child processes to verify that every
  output operation terminates unsuccessfully after a write or flush failure.

The allocation-failure and two output harnesses share only the small
error-reporting and exact-f64-bit construction helpers in
`runtime_test_support.c`. The Makefile builds and runs the runtime archive
before entering the runtime test Makefile, which retains the archive dependency
for direct use. It then builds and runs the contract, successful-allocation,
allocation-failure, successful-output, and output-failure harnesses in that
order so a failure identifies the responsible boundary directly.

The successful-output cases include integer boundaries, boolean values,
representative exact binary64 patterns, and mixed consecutive records. The
output-failure harness closes each child process's stdout descriptor before
invoking one output function. Passing tests write nothing to stdout or stderr
and compile under C11 with `-Wall -Wextra -Werror`.

Exact symbols, signatures, output bytes, versioning rules, and responsibility
boundaries belong only in the [runtime ABI](../../docs/compiler/RUNTIME_ABI.md).
