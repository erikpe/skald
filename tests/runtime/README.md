# Runtime Tests

Runtime tests are small C harnesses linked directly with the runtime implementation. They verify the C ABI independently of code generation. The first vertical slice requires only an ABI/build smoke test; shared ownership and object-layout tests arrive with those runtime features.

Run the current runtime suite from the repository root with:

```text
make runtime-test
```

`test_runtime_abi.c` verifies that a consumer compiled against the public header observes the same ABI version from the linked runtime archive.
