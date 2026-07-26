# Testing

Status: authoritative for test ownership, placement, and selection. Language,
compiler, backend, driver, and runtime contracts remain in their focused
documents; test guides describe how those contracts are verified, not what
the contracts mean.

## Test layers

Choose the narrowest layer that observes the behavior at its owning boundary.

Optional-value coverage spans type/capability/containment tests, HIR and MIR
shape and verifier tests, target layout tests, and native lifecycle tests.
Exact-class optional native tests use side-effect-visible destructors to catch
extra temporaries, missed conditional cleanup, and incorrect argument/result
ownership. Checked-view tests additionally cover bounded consumers, nested
guards, invalidating later arguments, shared-root anchor order, and failure
traps.
Optional shared-owner coverage additionally checks the one-word zero niche,
copy/adopt/move and conditional release, field and callable ownership,
self-assignment, target lifting and casts after unwrap, secured-anchor lifetime,
ABI register/stack pressure, absent-access failure, and exactly-once
last-owner finalization.

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

Shared-ownership coverage is intentionally distributed by responsibility.
Type-check and MIR tests cover owners in local, field, call, result, cast,
receiver, alias, ordinary-allocation, and copy-allocation positions. Backend
tests cover the header, count updates, hidden anchors, dynamic finalization,
cascading field release, cycles, and malformed-input rejection. Native goldens
observe destruction order and failure status; direct C runtime tests own
allocator success and fatal allocation failure. The process-determinism suite
compares all phase products for a representative shared copy-allocation
program.

Explicit shared-dereference coverage must keep owner operations and pointee
operations separate. Positive cases use `*owner` or `owner->member`; exact
compile-failure goldens cover raw-handle member access, alias arguments,
checked casts, type tests, inline copies, non-shared `*`, and unsupported
whole-pointee assignment. Diagnostics for member selection recommend `->`;
general object-place diagnostics recommend `*`.

## Determinism and process isolation

Phase dump tests call the same renderer repeatedly and compare exact text.
`pipeline_determinism` compares tokens, AST, resolved, HIR, MIR, and assembly
products for representative object-lifetime, polymorphism, and shared-ownership
programs from two independent test processes. The golden runner invokes `skac`
twice for every successful assembly and every compile failure, comparing
assembly or diagnostic bytes. It also executes every native case twice and
compares status, stdout, and stderr before evaluating the checked-in
expectations.

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

## Optional-value coverage

The frozen
[optional-values compiler contract](../compiler/OPTIONAL_VALUES.md#test-obligations)
requires coverage at every owning layer. Current lexer and parser tests own
tokens, contextual words, spans, precedence, bounded nesting, reserved forms,
and recovery. Current resolution tests own flat target identities and
source-shaped expression nodes. Inline-optional type-check and HIR tests own
expected-type-directed `none`, exact injection, initializer ranking, fields,
calls/results, exact signatures, copy, assignment, presence, unwrap,
truthiness rejection, external rejection, checked class payload consumers,
inline optional-container alias access/forwarding, and reserved-form
boundaries. MIR tests own initialized places, explicit
operations, CFG joins, aggregate calls, synthesized field lifecycle, checked
view/anchor order, and exact failure edges; verifier mutations break one
invariant at a time, including missing, mismatched, leaked, and reordered
guards. Backend tests own layout, instruction selection, guard counts, hidden
destinations, register/stack pressure, traps, and native execution.

Optional-owner coverage includes shared class/interface/`Obj` up-views,
zero-niche realization, secured-owner unwrap, and virtual/interface dispatch
while preserving the inline optional and checked-view matrix.
