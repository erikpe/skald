# Golden Tests

Golden tests compile complete `.ska` programs and assert one or more externally visible results: successful compilation, diagnostics, emitted assembly properties, link success, and process exit status.

Keep each case focused. Architecture-independent cases should be reusable across backends; target-specific assembly assertions should be clearly separated from semantic expectations.

The runner discovers two case families:

- `run/**/*.ska` has a same-named `.exit` sidecar containing the expected Linux process status in `0..=255`;
- `compile_fail/**/*.ska` has a same-named `.stderr` sidecar containing the exact expected diagnostic output.

It builds the runtime, invokes the public `skac` executable, and reports all cases. Every successful source is compiled to assembly twice and compared byte-for-byte before native execution. Every failing source is compiled twice and must produce the same exact stderr snapshot, no stdout, and compiler exit status 1.

The failure corpus covers every diagnostic family reachable from first-slice source. The generic type-mismatch diagnostic becomes source-reachable only after a second semantic type is added and must gain a golden case in that feature slice.

Run it from the repository root with:

```text
make golden-test
```
