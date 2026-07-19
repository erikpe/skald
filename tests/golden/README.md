# Golden Tests

Golden tests compile complete `.ska` programs and assert one or more externally visible results: successful compilation, diagnostics, emitted assembly properties, link success, and process exit status.

Keep each case focused. Architecture-independent cases should be reusable across backends; target-specific assembly assertions should be clearly separated from semantic expectations.

The M7 native runner discovers `run/**/*.ska`. Each source has a same-named `.exit` sidecar containing the expected Linux process status in `0..=255`. It builds the runtime, compiles through the public `skac` executable, runs the generated program, and rejects unexpected stdout or stderr.

Run it from the repository root with:

```text
make golden-test
```
