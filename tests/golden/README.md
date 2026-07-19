# Golden Tests

Golden tests compile complete `.ska` programs and assert one or more externally visible results: successful compilation, diagnostics, emitted assembly properties, link success, and process exit status.

Keep each case focused. Architecture-independent cases should be reusable across backends; target-specific assembly assertions should be clearly separated from semantic expectations.

The runner discovers two case families:

- `run/**/*.ska` has a required same-named `.exit` sidecar containing the expected Linux process status in `0..=255` and may have a same-named `.stdout` sidecar containing the exact expected stdout bytes;
- `compile_fail/**/*.ska` has a same-named `.stderr` sidecar containing the exact expected diagnostic output.

If a run case has no `.stdout` sidecar, its expected stdout is empty. Stdout comparison is byte-for-byte: whitespace, line endings, trailing line feeds, and non-UTF-8 bytes are not normalized. Runtime stderr must remain empty.

The runner builds the runtime, invokes the public `skac` executable, and reports all cases. Every successful source is compiled to assembly twice and compared byte-for-byte before native execution. Every failing source is compiled twice and must produce the same exact stderr snapshot, no stdout, and compiler exit status 1.

The failure corpus covers every diagnostic family reachable from implemented
source. O3 adds exact cases for invalid unit/value returns, using a unit call as
an `i64` value, and discarding an `i64` call statement. O5 and O6 add
external-entry, restricted-signature, duplicate-name, and malformed external
declaration cases. C2 adds focused boolean type/entry failures and replaces the
old unsupported-`bool` case with a genuinely unsupported type. The O6 `println_i64` run case covers the full supported
source-to-runtime output path, ordered consecutive writes, representative
computed values, both `i64` extrema, and a process status independent of its
exact stdout expectation. The C2 `println_bool` case likewise covers boolean
literals, locals, parameters, function returns, and ordinary runtime linkage.
C5 adds exact conditional parser, scope, condition-type, and definite-return
failures. Its native conditional case uses observable condition functions to
prove left-to-right evaluation, skipped later arms, `else` selection, and
fallthrough when no arm is selected.
C6 adds omitted-condition and omitted-closing-parenthesis snapshots plus a
nested native case whose value-returning function relies entirely on exhaustive
conditional arms. Together with the non-exhaustive failure case, this covers
both sides of branch-aware return analysis.

Run it from the repository root with:

```text
make golden-test
```
