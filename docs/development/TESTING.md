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

Primitive binding reassignment is covered at each owning boundary: syntax tests
retain direct and grouped assignment shapes; resolver tests pin `LocalId`
and `ParameterId` selection, shadowing, and source-diagnostic recovery;
type-check and HIR tests cover all five exact primitive types and deterministic
identity-only dumps for locals and value parameters; MIR tests establish
binding-storage selection, source-before-store ordering, and post-store
temporary cleanup; backend and native goldens exercise canonical integer,
byte, boolean, and floating storage plus exact rendered failures.

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

Multi-file golden directories contain one `case.args` manifest plus their
entry and supporting trees. The manifest records one exact command argument
per line, including entry mode, module roots, and standard-library selection;
its directory is the compiler working directory. Discovery treats the whole
directory as one case and never promotes supporting `.ska` files into
independent fixtures.

Exact dump and diagnostic expectations should remain readable and intentional.
When an expectation changes, inspect the semantic difference before updating
it. Do not introduce a second renderer solely to make a test convenient.

Cyclic-module tests are split by owner. Graph tests prove reachable-closure
loading and canonical module identities without recursion. Resolver and
type-check tests prove declaration-first lookup across cycles and retain
inheritance, containment, interface, external-ABI, privacy, and direct-import
diagnostics at their existing semantic phases. The cross-process determinism
suite permutes source discovery, provider spelling, and import order while
comparing graph, resolved, HIR, MIR, diagnostic, and assembly products.
Native and compile-failure goldens remain the source-visible end-to-end
observations.

### Module-system coverage map

The required diagnostics in the
[module compiler contract](../compiler/MODULE_SYSTEM.md#required-diagnostic-coverage)
have explicit owners:

| Contract area | Owning evidence |
|---|---|
| Entry selectors, logical spelling, root/standard-library options, suffixes, output defaults, and process status | driver CLI unit tests and `crates/skac/tests/cli.rs` through the real binary |
| Root equivalence, provider ambiguity independent of contents or physical target, symlink traversal and failure, exact case, unreadable/non-regular candidates, and positional containment | `module::provider::tests` and `module::graph::tests` filesystem matrices |
| Missing and ambiguous imports, import-source aliases, direct self-imports, malformed reached sources, binding conflicts, privacy, direct-import enforcement, unknown/wrong-kind declarations, selected `main`, and incompatible external ABI declarations | exact single- and multi-file snapshots under `tests/golden/compile_fail/`, plus structured graph and resolver tests |
| Two-module, longer, selected-entry, synthetic string, and deep cyclic dependency graphs | successful native goldens plus structured `module::graph::tests` coverage |
| Qualified/selective lookup, opposite-side declarations, non-module semantic cycles, and access diagnostics inside cyclic graphs | `resolve::tests::cyclic_imports`, the `modules_cycle` native golden, and the `modules_cycle_diagnostics` exact compile-failure golden |
| Multi-segment aliases, wildcard imports, and trailing selective-import commas | exact parser goldens and syntax recovery tests |
| Ordering and independent-process stability, including semantic cycles and alternate selected entries with one closure | `pipeline_determinism`, `resolve::tests::cyclic_imports`, graph/provider permutation tests, and the two-process golden runner |

Host-dependent filesystem wording stays asserted structurally at the provider
boundary; portable source diagnostics use byte-exact golden snapshots. This
keeps every required failure owned without freezing operating-system error
text.

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
observe destruction order and exact failure stderr; direct C runtime tests own
allocator success, invalid-input defects, and reported host exhaustion. The
process-determinism suite
compares all phase products for a representative shared copy-allocation
program.

Explicit shared-dereference coverage must keep owner operations and pointee
operations separate. Positive cases use `*owner` or `owner->member`; exact
compile-failure goldens cover raw-handle member access, alias arguments,
checked casts, type tests, inline copies, non-shared `*`, and unsupported
whole-pointee assignment. Diagnostics for member selection recommend `->`;
general object-place diagnostics recommend `*`.

Array coverage is distributed by the operation owner. Syntax, resolution, and
type-check tests cover recursive grouping, exact identity, capabilities,
access, diagnostics, and deferred forms. MIR mutation tests break structural,
storage, ownership, projection, slice-order, and anchor invariants one at a
time. Backend tests own checked byte arithmetic, layout, helper labels,
internal ABI pressure, every terminating failure family, and native lifecycle.
Golden cases own complete successful and unsuccessful process observations.
Because array failures promise only non-return, `.exit` sidecars use
`failure`; tests must not depend on a particular signal or numeric status.

While-loop coverage follows the phase boundary that owns each invariant.
Lexer tests keep `while`, `break`, and `continue` reserved without reserving
identifier prefixes. Syntax tests own mandatory loop punctuation and
loop-exit recovery and spans. Resolution and type-check tests own
source-ordered `LoopId`s, nearest-loop exit selection, outside-loop rejection,
enclosing-condition and child body scopes, exact-`bool` conditions,
conservative fallthrough, distinct targeted effects, and structured HIR
dumps. Source-to-MIR tests prove generic cyclic graphs and cleanup-to-exit or
latch edges, while the internal-HIR lifecycle matrix and verifier mutations
cover every current storage family without duplicating those cases at the
parser boundary. Native goldens own zero, one, and repeated iterations,
immediate and conditional exits, nested blocks and loops, enclosing mutation,
condition/body/break/continue cleanup, ownership-heavy exits, mixed
fallthrough/return/panic behavior, and return from a body. The golden runner's
repeated assembly comparison remains the source-to-target determinism check.

The complete source-observation matrix has these owners:

| Loop lifecycle area | Source-to-observation evidence |
|---|---|
| Condition false and condition-owned compiler temporaries | `while_loops.ska` and `loop_lifecycle_matrix.ska` |
| Primitive mutation, normal body fallthrough, and fresh body epochs | `while_loops.ska` |
| Inline objects, shared owners, primitive/class/shared optionals, arrays, checked views, aliases, and shared-backed anchors | `loop_lifecycle_matrix.ska` |
| Immediate, conditional, nested-scope, ownership-heavy, and nested-loop `break` | `break_loops.ska` and `loop_lifecycle_matrix.ska` |
| Immediate, conditional, nested-scope, ownership-heavy, nearest-loop, and condition-effect `continue` | `continue_loops.ska` and `loop_lifecycle_matrix.ska` |
| Return cleanup and mixed fallthrough, break, continue, return, and panic effects | `while_loops.ska`, `continue_loops.ska`, and `loop_lifecycle_matrix.ska` |

Colocated MIR hardening tests separately prove that condition-owned storage is
dead before body or exit, body-owned storage is dead before latch, header, or
exit, and redirected edges cannot skip cleanup. Bounded generators cover both
source loops through the complete pipeline and target-independent cyclic CFGs
through deterministic verification. Equivalent split and renumbered loop CFGs
must survive the pass boundary and x86-64 lowering without canonical-layout or
source-loop recognition.

Primitive integer operation coverage keeps the closed matrices explicit.
Type-check and MIR tests enumerate all eighteen same-type comparisons and all
nine casts, while rejection tests enumerate every ordered mixed integer pair,
predicate, noninteger operand family, and cast target. Backend tests own
condition signedness, canonical scalar results, instruction shape, and
malformed-MIR rejection. Golden cases own exact diagnostics, extrema and
modulo observations, and range-check-before-cast composition with array
positions.

## Private initializer coverage

Private ordinary initializer tests follow the owning phase instead of relying
on one end-to-end factory case:

- syntax and resolution tests own contextual modifier parsing, recovery,
  declaration spans, per-overload visibility, and stable initializer
  identities;
- type-check tests own unique-most-specific selection followed by access,
  including the no-public-fallback rule, exact-class body categories,
  inaccessible derived `super(...)`, direct/shared construction, and inline
  and shared default-length arrays;
- HIR tests prove that successful selection retains only the authorized
  `InitializerId` and erases visibility, including independently mutated
  resolved declarations;
- MIR verifier tests mutate selected initializer identities and array default
  plans independently because lower phases deliberately have no visibility
  metadata;
- determinism tests compare mixed public/private selection, rendered `TYP040`
  diagnostics, HIR, MIR, and assembly across independent processes; and
- native factory goldens prove exact-class private construction while compile
  failures cover foreign and derived callers.

When adding a construction form, test its authorization at the type-check
consumer and its selected identity at the nearest lower-phase verifier. Do not
make a global array capability table caller-dependent: authorize its stable
default plan where the source array expression is checked.

## String coverage

String coverage follows the
[language](../language/STRINGS.md) and
[compiler](../compiler/STRINGS.md#diagnostics-and-test-obligations) contracts:

- lexer and syntax tests own decoded bytes, every escape and malformed-literal
  category, spans, recovery, and nesting limits;
- module/provider tests own synthetic `std::str` reachability, explicit-edge
  coalescing, missing/ambiguous/exact-case lookup, malformed and non-UTF-8
  providers, cyclic dependencies, replacement roots, and disabled
  standard-library lookup;
- resolver and type-check tests own exact language-item identity, the complete
  descriptor/privacy/lifecycle rejection matrix, produced-value contexts,
  canonical private descriptor construction, the canonical
  `std::str`/`std::error` cycle and panic statements, and the rule that
  ordinary initializer and method names have no compiler meaning;
- MIR tests mutate each string declaration, literal-data, static-owner,
  descriptor-publication, and ownership invariant independently;
- backend tests own immutable bytes, pooling, alignment, relocations,
  sentinel-aware retain/release, dynamic reclamation, malformed-input
  rejection, and the unchanged runtime ABI;
- native goldens own copying, assignment, arguments, results, temporaries,
  signed and negative byte positions, half-open and negative slice bounds,
  checked failures, factory isolation, conversion, concatenation, embedded
  zero/high bytes, and repeated execution; and
- `pipeline_determinism` compares canonical graph, diagnostics, resolved HIR,
  verified MIR, and assembly across independent processes and provider/source
  permutations.

Focused resolver, MIR, and backend tests use one shared test-only helper that
loads the canonical `std::str` and `std::error` sources as a complete
dependency closure. Cross-process determinism and driver provider tests copy
both real sources into their filesystem fixtures. This keeps the public
surface, panic dependency, and lifecycle behavior from drifting independently
of compiler coverage.

## Determinism and process isolation

Phase dump tests call the same renderer repeatedly and compare exact text.
`pipeline_determinism` compares tokens, AST, resolved, HIR, MIR, and assembly
products for representative object-lifetime, polymorphism, shared-ownership,
optional-value, array, primitive-integer-operation, and string programs from
two independent test processes. Its module cases additionally permute root
option order, equivalent root spellings, source creation order, import
declaration order, and logical versus positional selection of the same rooted
entry, then compare canonical graph, resolved, HIR, MIR, assembly, and
diagnostic products. The golden runner invokes `skac` twice for every
successful assembly and every compile failure, comparing assembly or
diagnostic bytes. It also executes every native case twice and compares
status, stdout, and stderr before evaluating the checked-in expectations.
Native `.stdout` and `.stderr` sidecars are exact byte expectations; a missing
sidecar requires its stream to be empty. The focused
`make golden-expectations-test` suite owns sidecar loading and escaped
byte-mismatch rendering independently of compiler execution.

Panic goldens cover every failure that a compact source program can trigger,
including explicit dynamic messages, cast and optional failures, array and
string bounds, invalid allocation requests, and valid host-allocation
exhaustion. Counter saturation cannot be reached by a tractable source fixture:
backend native tests inject the optional-guard and ownership-count boundary
states into otherwise compiler-generated assembly, use the same exact stderr
expectations, and separately prove that invalid ownership states remain silent
hard traps. The exhaustive termination-selector test covers every
`MirTerminationReason`, requires one reporter call and no `ud2`, and therefore
keeps new compiler-known failures inside the common reporting boundary.

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

The implemented
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
Native golden `.exit` sidecars use `failure` when the contract requires only
unsuccessful termination; exact trap signals and shell-normalized statuses are
not portable language observations. Optional full-phase determinism and the
MIR mutation corpus cover dumps, verification, and backend rejection, while
the extended robustness suite mutates optional punctuation deterministically.
