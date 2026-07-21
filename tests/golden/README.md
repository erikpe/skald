# Golden Tests

Golden tests compile complete `.ska` programs and assert one or more externally visible results: successful compilation, diagnostics, emitted assembly properties, link success, and process exit status.

Keep each case focused. Architecture-independent cases should be reusable across backends; target-specific assembly assertions should be clearly separated from semantic expectations.

The runner discovers two case families:

- `run/**/*.ska` has a required same-named `.exit` sidecar containing the expected Linux process status in `0..=255` and may have a same-named `.stdout` sidecar containing the exact expected stdout bytes;
- `compile_fail/**/*.ska` has a same-named `.stderr` sidecar containing the exact expected diagnostic output.

If a run case has no `.stdout` sidecar, its expected stdout is empty. Stdout comparison is byte-for-byte: whitespace, line endings, trailing line feeds, and non-UTF-8 bytes are not normalized. Runtime stderr must remain empty.

The runner builds the runtime, invokes the public `skac` executable, and reports all cases. Every successful source is compiled to assembly twice and compared byte-for-byte before native execution. Every failing source is compiled twice and must produce the same exact stderr snapshot, no stdout, and compiler exit status 1.

The failure corpus covers every diagnostic family reachable from implemented
source: invalid unit/value returns, value-context and discarded calls,
external-entry and restricted-signature rules, duplicate declarations,
malformed syntax, condition types and scopes, and definite-return behavior.
The primitive output run cases cover ordered runtime output,
representative values, primitive extrema, locals, parameters, function
results, and process status independently of exact stdout.

Conditional cases use observable condition functions to prove left-to-right
evaluation, skipped later arms, `else` selection, fallthrough, nested control
flow, and both exhaustive and non-exhaustive return analysis.

The primitive corpus covers all implemented scalar types. Unsigned run cases
observe zero, one, maxima, wrapping arithmetic, locals, parameters, internal
results, and external output calls. The floating case observes positive and
negative zero, exact fractions, subnormal and maximum finite values, underflow,
arithmetic, call/return flow, conditional arms, and both register-only and
independently exhausted mixed integer/SSE signatures. Its calls interleave `u64`, `u8`, and
raw-bit `f64` output. Compile-failure cases snapshot every numeric overflow
family, malformed suffix and float forms, implicit conversions, mixed
arithmetic, invalid unsigned negation, and the restricted external profile.
Because the runner compiles every case twice, this coverage also proves
assembly and diagnostic determinism across independent compiler processes.

The inline-object corpus covers direct construction, all primitive field
types, field reads and writes, read-only and mutable methods, method results,
conditionals, multiple object locals, observable argument order, padded class
layout, and mixed integer/SSE receiver calls through register and stack
boundaries. Compile-failure cases snapshot construction and value exclusions,
field initialization state, initializer-body restrictions, declaration
restrictions, and read-only receiver violations.

Run it from the repository root with:

```text
make golden-test
```
