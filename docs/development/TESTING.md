# Testing

Status: authoritative for test ownership, placement, and selection. Language,
compiler, backend, driver, and runtime contracts remain in their focused
documents; test guides describe how those contracts are verified, not what
the contracts mean.

## Test layers

Choose the narrowest layer that observes the behavior at its owning boundary.

| Layer | Location | Use it for |
|---|---|---|
| Compiler unit tests | `crates/skald-compiler/src/` beside the owner | Private algorithms, diagnostics, exact phase dumps, MIR verification, target legality, and lowering |
| Compiler integration tests | `crates/skald-compiler/tests/` | Public paths, cross-phase composition, cross-process determinism, and frontend robustness |
| Binary integration tests | `crates/skac/tests/` | The real `skac` entry point and process-visible CLI behavior |
| Golden tests | `tests/golden/` | Complete source-to-diagnostic or source-to-native-observation behavior |
| Runtime tests | `tests/runtime/` | The C runtime contract independently of compiler code generation |
| Documentation tests | `crates/skald-docs-check/` | Repository-local Markdown links, anchors, and required indexes |

Reusable non-Rust compiler corpus data belongs under `tests/compiler/`.
Production crates must not depend on the top-level test tree at runtime.

## Selecting coverage

Add a colocated unit test when a private owner can state the invariant directly.
Use a compiler integration test only when the public repository-internal API or
multiple phase facades are the subject. Use a binary integration test for CLI
argument, stream, status, or artifact behavior that must pass through the real
executable.

Add a golden case when the required observation crosses the complete compiler
boundary: exact diagnostics, deterministic assembly, linking, runtime output,
or process status. Add a direct C runtime case when compiler output is
irrelevant to the ABI behavior. A change may need more than one layer, but do
not repeat the same assertion at every layer.

For a new accepted source form, normally cover its smallest phase-specific
contract and its source-visible result. For a rejection, assert the diagnostic
at the phase that owns it and add an exact compile-failure golden only when the
complete rendered diagnostic is part of the regression. Backend tests should
prefer assembly shape or legality assertions; native goldens are for behavior
that assembly text alone does not establish.

## Focused commands

`make help` is the complete command inventory. Useful focused forms include:

```text
cargo test --locked -p skald-compiler lexer::tests
cargo test --locked -p skald-compiler mir::verify
cargo test --locked -p skald-compiler --test public_api
make cli-test
make golden-test
make runtime-test
make compiler-test
```

Rust test-name filters match substrings and may select more than one test; use
`--exact` only after obtaining the complete test path from `cargo test -- --list`.
Before handoff, run the full validation described in the
[development workflow](README.md#change-validation).

## Fixtures and expectations

Keep small source fixtures in the test module that consumes them. Shared Rust
fixtures belong in a responsibility-named test module, not a general bag of
defaults. Test-only compiler pipelines stop at the boundary named by the
helper and assert only that earlier phases succeeded.

MIR verifier tests may use the crate-visible constructors and mutation
accessors under `cfg(test)`. These deliberately preserve explicit identities,
types, ownership modes, and spans; they are not production API. Start from the
smallest valid MIR, mutate one invariant, and assert the structured verifier
failure and, where relevant, backend rejection.

Top-level corpus and sidecar formats are documented locally:

- [compiler and robustness corpus](../../tests/compiler/README.md);
- [golden discovery and sidecars](../../tests/golden/README.md); and
- [runtime harnesses](../../tests/runtime/README.md).

Exact dump and diagnostic expectations should remain readable and intentional.
When an expectation changes, inspect the semantic difference before updating
it. Do not introduce a second renderer solely to make a test convenient.

Explicit-copy tests should distinguish `T(copy source)` from ordinary
`T(copy)` and `T(copy, other)`, cover static and runtime target selection,
assert one source evaluation and one selected copy, and verify that explicit
copy is not recorded as constructor elision. Corrupt the lowered copy target
or operation in a verifier test rather than relying only on successful native
execution.

Ordinary-constructor coverage should compose value, `ref`, and `mut ref`
binding with exact, ancestor, interface, and `Obj` relations. Exercise selected
initializer identities in local, field, argument, result, temporary, and
direct-base contexts. Verifier mutations should independently cover table
density, declaration/definition agreement, selected target and signature,
source lifetime, and undeclared call targets.

## Determinism and process isolation

Phase dump tests call the same renderer repeatedly and compare exact text.
`pipeline_determinism` compares tokens, AST, resolved, HIR, MIR, and assembly
products for representative object-lifetime and polymorphism programs from
two independent test processes. The golden runner invokes `skac` twice for
every successful assembly and every compile failure, comparing assembly or
diagnostic bytes. It also executes every native case twice and compares status,
stdout, and stderr before evaluating the checked-in expectations.

Preserve this process isolation for behavior affected by identity allocation,
table traversal, filesystem paths, labels, diagnostics, or formatting. A
single-process equality check is useful but does not replace it.

## Robustness

`make compiler-test` includes the fixed-seed bounded hostile frontend inputs
and structured MIR mutations with the rest of the compiler suite.
`make robustness-long` reruns the generated frontend cases with a larger
`SKALD_ROBUSTNESS_CASES` value. It is intended for less frequent external,
scheduled, or pre-release validation and remains reproducible.

When robustness testing finds a defect, retain the smallest focused regression
at the owning layer. Add corpus data only when the bytes or source are clearer
and more reusable than constructing the case in Rust.
