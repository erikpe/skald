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

The class-typed inline-field cases extend that coverage through acyclic nested
storage, forward layout dependencies, padding, empty subobjects, direct field
construction, deep local/receiver/alias access, method calls, deliberate alias
forwarding, and observable constructor evaluation order. Their failure
snapshots cover unknown and non-class field types, direct and indirect
containment cycles, wrong and grouped constructors, premature/duplicate/missing
initialization, read-only nested mutation, alias access/type mismatches,
object-value escape, and whole-object replacement.

The alias corpus covers read-only and mutable access, forwarding, grouped
places, deliberate overlap, method `self`, initializer aliases, nested calls,
conditionals, and signatures that independently exhaust integer and SSE
registers before passing aliases and scalar values on the stack. Failure cases
snapshot malformed modifiers, excluded local and primitive aliases,
object-value misuse, exact nominal mismatches, read-only mutation, invalid
mutable forwarding, external aliases, and wrong arity.

The deterministic-destruction corpus observes user-body-before-field cleanup,
reverse field and local order, nested storage and scopes, conditional
fallthrough, early return, return-expression evaluation before cleanup,
non-owning aliases, empty and padded classes, and absent user bodies. Failure
snapshots cover malformed and duplicate declarations, invalid value returns,
explicit calls of the special member, reconstruction of live fields, excluded
object values, and body type mismatches. The runner's existing process
isolation proves exact assembly and diagnostic determinism; native sidecars
additionally require exact stdout, status, and empty stderr.

The object-value corpus spans `object_value_copy`, `object_value_parameters`,
`object_value_results`, and `object_value_temporaries`. Together they cover
user and synthesized lifecycle operations, self-assignment, nested and padded
layout, empty classes, caller-owned parameter copies, mixed register/stack
arguments, function and method results, conditional and recursive returns,
explicit return storage, alias sources, constructor- and result-produced
sources, full-expression cleanup, and both permitted elision cases. Lifecycle
traces distinguish grouped materialization from direct local/return
construction and require every owning destination to be cleaned exactly once.
Failure snapshots cover malformed lifecycle declarations, illegal bodies and
contexts, exact-class mismatches, missing returns, alias-rooted replacement,
and external object signatures. Capability-unavailable states are not
source-reachable with the current acyclic primitive/inline field kinds; focused
HIR/MIR tests cover their deterministic diagnostic and verifier paths.

Run it from the repository root with:

```text
make golden-test
```
